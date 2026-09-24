//! `qualifier init` — interactive, idempotent project bootstrap.
//!
//! Configures VCS merge behavior and inserts a directive into agent
//! instruction files that points coding agents at `qualifier agents`.
//! See `docs/superpowers/specs/2026-05-13-qualifier-init-design.md`.

use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
pub struct Args {
    /// Auto-accept every step at its default.
    #[arg(short, long)]
    pub yes: bool,

    /// Report what would change without writing or prompting.
    #[arg(long, conflicts_with = "yes")]
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Interactive,
    Yes,
    DryRun,
}

fn mode_from_args(args: &Args) -> Mode {
    if args.dry_run {
        Mode::DryRun
    } else if args.yes {
        Mode::Yes
    } else {
        Mode::Interactive
    }
}

#[cfg(target_os = "emscripten")]
pub fn run(_args: Args) -> crate::Result<()> {
    Err(crate::Error::Validation(
        "init is not available in the browser".into(),
    ))
}

#[cfg(not(target_os = "emscripten"))]
pub fn run(args: Args) -> crate::Result<()> {
    use std::io::IsTerminal;

    let mode = mode_from_args(&args);
    if mode == Mode::Interactive && !io::stdin().is_terminal() {
        return Err(crate::Error::Validation(
            "init: refusing to prompt without a TTY; pass --yes or --dry-run".into(),
        ));
    }

    let cwd = std::env::current_dir()?;
    let root = crate::qual_file::find_project_root(&cwd).unwrap_or(cwd);
    run_steps(
        &root,
        mode,
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
    )
}

/// Run every init step against `root`, reading answers from `input` in
/// interactive mode and writing the transcript to `out`.
///
/// Changes are written as each step is accepted, so an error (including
/// end of input at a prompt) leaves earlier accepted changes in place and
/// makes no further ones.
fn run_steps<R: BufRead, W: Write>(
    root: &Path,
    mode: Mode,
    input: &mut R,
    out: &mut W,
) -> crate::Result<()> {
    let mut session = Session {
        mode,
        input,
        out,
        applied: 0,
        skipped: 0,
    };
    step_vcs_merge_config(&mut session, root)?;
    step_agent_directive(&mut session, root)?;
    session.finish()
}

/// Prompting, transcript output, and applied/skipped bookkeeping shared by
/// all steps.
struct Session<'a, R, W> {
    mode: Mode,
    input: &'a mut R,
    out: &'a mut W,
    /// Changes written (or, in dry-run mode, that would be written).
    applied: usize,
    /// Steps not applied: already configured, declined, or not applicable.
    skipped: usize,
}

impl<R: BufRead, W: Write> Session<'_, R, W> {
    /// Show a proposed change and decide whether to write it. Returns true
    /// only when the caller should write; in dry-run mode the change is
    /// counted as applied but never written.
    fn offer(&mut self, target: &str, additions: &[&str], prompt: &str) -> crate::Result<bool> {
        writeln!(self.out, "{target}")?;
        for line in additions {
            writeln!(self.out, "  + {line}")?;
        }
        let accepted = match self.mode {
            Mode::Yes => true,
            Mode::DryRun => {
                self.applied += 1;
                return Ok(false);
            }
            Mode::Interactive => self.ask(prompt)?,
        };
        if accepted {
            self.applied += 1;
        } else {
            self.skipped += 1;
        }
        Ok(accepted)
    }

    /// Prompt with a default of yes until the answer is recognizable.
    /// End of input aborts rather than taking the default.
    fn ask(&mut self, prompt: &str) -> crate::Result<bool> {
        loop {
            write!(self.out, "{prompt} [Y/n] ")?;
            self.out.flush()?;
            let mut line = String::new();
            if self.input.read_line(&mut line)? == 0 {
                writeln!(self.out)?;
                return Err(crate::Error::Validation(
                    "init: aborted at end of input; no further changes made".into(),
                ));
            }
            match line.trim().to_ascii_lowercase().as_str() {
                "" | "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => writeln!(self.out, "please answer y or n")?,
            }
        }
    }

    /// Record a step that needs no change or cannot be automated.
    fn skip(&mut self, message: &str) -> crate::Result<()> {
        writeln!(self.out, "{message}")?;
        self.skipped += 1;
        Ok(())
    }

    fn finish(self) -> crate::Result<()> {
        let (applied, skipped) = (self.applied, self.skipped);
        if self.mode == Mode::DryRun {
            writeln!(
                self.out,
                "init (dry run): would apply {applied}, skipped {skipped}."
            )?;
        } else {
            writeln!(self.out, "init: applied {applied}, skipped {skipped}.")?;
        }
        Ok(())
    }
}

fn step_vcs_merge_config<R: BufRead, W: Write>(
    session: &mut Session<'_, R, W>,
    root: &Path,
) -> crate::Result<()> {
    match crate::qual_file::detect_vcs(root) {
        Some("git") => step_git_gitattributes(session, root),
        Some("hg") => session.skip(
            "hg detected: to union-merge .qual files, add to .hg/hgrc:\n  \
             [merge-patterns]\n  \
             **.qual = :union",
        ),
        Some("jj") => session.skip(
            "jj detected: jj has no per-path merge configuration; when .qual \
             files conflict, keep both sides' lines (see SPEC §8.2)",
        ),
        Some(vcs) => session.skip(&format!(
            "{vcs} detected: configure union merge for *.qual if {vcs} supports it \
             (see SPEC §8.2)"
        )),
        None => session.skip("no VCS detected: skipping merge configuration"),
    }
}

fn step_git_gitattributes<R: BufRead, W: Write>(
    session: &mut Session<'_, R, W>,
    root: &Path,
) -> crate::Result<()> {
    let path = root.join(".gitattributes");
    let existing = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io_error(&path, "read", e)),
    };

    if has_gitattributes_rule(&existing) {
        return session.skip(".gitattributes: *.qual merge=union already configured");
    }
    if session.offer(".gitattributes", &[GITATTRIBUTES_RULE], "Apply?")? {
        write_file(&path, &append_gitattributes_rule(&existing))?;
    }
    Ok(())
}

/// Where an agent-instruction convention lives relative to the project
/// root.
enum AgentSource {
    /// A single file.
    File(&'static str),
    /// Files directly inside a directory (not recursive) whose names end
    /// with one of the given suffixes.
    Dir(&'static str, &'static [&'static str]),
}

/// Agent-instruction conventions, in the order files are reported.
const AGENT_SOURCES: &[AgentSource] = &[
    AgentSource::File("AGENTS.md"),
    AgentSource::File("CLAUDE.md"),
    AgentSource::File("GEMINI.md"),
    AgentSource::File("CONVENTIONS.md"),
    AgentSource::File(".cursorrules"),
    AgentSource::File(".windsurfrules"),
    AgentSource::File(".clinerules"),
    AgentSource::Dir(".clinerules", &[".md"]),
    AgentSource::File(".github/copilot-instructions.md"),
    AgentSource::Dir(".github/instructions", &[".instructions.md"]),
    AgentSource::File(".junie/guidelines.md"),
    AgentSource::Dir(".cursor/rules", &[".md", ".mdc"]),
];

/// Resolve [`AGENT_SOURCES`] against `root` into the agent-instruction
/// files that exist. Directory entries are sorted by name. Paths that
/// resolve to the same file (e.g. `CLAUDE.md` symlinked to `AGENTS.md`)
/// are listed once, under the first name found.
fn discover_agent_files(root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for source in AGENT_SOURCES {
        match source {
            AgentSource::File(rel) => {
                let full = root.join(rel);
                if full.is_file() {
                    candidates.push(full);
                }
            }
            AgentSource::Dir(rel, suffixes) => {
                let Ok(entries) = fs::read_dir(root.join(rel)) else {
                    continue;
                };
                let mut children: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_file())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| suffixes.iter().any(|s| n.ends_with(s)))
                    })
                    .collect();
                children.sort();
                candidates.extend(children);
            }
        }
    }

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())))
        .collect()
}

fn step_agent_directive<R: BufRead, W: Write>(
    session: &mut Session<'_, R, W>,
    root: &Path,
) -> crate::Result<()> {
    let files = discover_agent_files(root);
    if files.is_empty() {
        if session.offer("AGENTS.md (new file)", &[DIRECTIVE], "Create AGENTS.md?")? {
            write_file(&root.join("AGENTS.md"), &append_agent_directive(""))?;
        }
        return Ok(());
    }

    for path in files {
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
        let existing = fs::read_to_string(&path).map_err(|e| io_error(&path, "read", e))?;
        if already_has_directive(&existing) {
            session.skip(&format!("{rel}: already references `qualifier agents`"))?;
        } else if session.offer(&rel, &[DIRECTIVE], "Apply?")? {
            write_file(&path, &append_agent_directive(&existing))?;
        }
    }
    Ok(())
}

/// Attach the failing path to an I/O error, keeping its kind.
fn io_error(path: &Path, action: &str, err: io::Error) -> crate::Error {
    crate::Error::Io(io::Error::new(
        err.kind(),
        format!("failed to {action} {}: {err}", path.display()),
    ))
}

fn write_file(path: &Path, content: &str) -> crate::Result<()> {
    fs::write(path, content).map_err(|e| io_error(path, "write", e))
}

const GITATTRIBUTES_RULE: &str = "*.qual merge=union";

/// True if the `merge` attribute of `*.qual` files ends up as `union`
/// according to `content` (the body of a root `.gitattributes`).
///
/// Follows git's rules for the lines that can affect `*.qual`: lines whose
/// pattern is `*.qual`, `**/*.qual`, or `*`; the last line that sets
/// `merge` wins; `-merge`, `!merge`, a bare `merge`, a different
/// `merge=<driver>`, or the `binary` macro all mean "not union". `#` starts
/// a comment only at the beginning of a line, and `[attr]` macro
/// definitions are not patterns.
fn has_gitattributes_rule(content: &str) -> bool {
    let mut union = false;
    for line in content.lines() {
        let mut tokens = line.split_whitespace();
        let Some(pattern) = tokens.next() else {
            continue;
        };
        if !matches!(pattern, "*.qual" | "**/*.qual" | "*") {
            continue;
        }
        for attr in tokens {
            match attr {
                "merge=union" => union = true,
                "-merge" | "!merge" | "merge" | "binary" => union = false,
                a if a.starts_with("merge=") => union = false,
                _ => {}
            }
        }
    }
    union
}

/// The line ending `content` already uses: CRLF if any line has one,
/// otherwise LF.
fn line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Build the new `.gitattributes` content with the qualifier rule
/// appended on its own line, in the file's existing line ending, and
/// ending with a line ending.
fn append_gitattributes_rule(existing: &str) -> String {
    let eol = line_ending(existing);
    let mut out = String::with_capacity(existing.len() + GITATTRIBUTES_RULE.len() + 4);
    out.push_str(existing);
    if !existing.is_empty() && !existing.ends_with('\n') {
        out.push_str(eol);
    }
    out.push_str(GITATTRIBUTES_RULE);
    out.push_str(eol);
    out
}

/// True if `content` already mentions `qualifier agents`
/// (case-insensitive substring). Used to avoid duplicate directive
/// insertion on re-runs and to respect existing documentation that
/// already points readers at the subcommand.
fn already_has_directive(content: &str) -> bool {
    content.to_ascii_lowercase().contains("qualifier agents")
}

const DIRECTIVE: &str = "If qualifier might be relevant, run `qualifier agents` first.";

/// Build the new agent-file content with the directive appended.
/// Empty input → just the directive and a line ending. Non-empty input →
/// ensure a trailing line ending, then a blank line, then the directive,
/// all in the file's existing line ending.
fn append_agent_directive(existing: &str) -> String {
    let eol = line_ending(existing);
    let mut out = String::with_capacity(existing.len() + DIRECTIVE.len() + 6);
    if !existing.is_empty() {
        out.push_str(existing);
        if !existing.ends_with('\n') {
            out.push_str(eol);
        }
        out.push_str(eol);
    }
    out.push_str(DIRECTIVE);
    out.push_str(eol);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gitattributes_present_exact() {
        assert!(has_gitattributes_rule("*.qual merge=union\n"));
    }

    #[test]
    fn gitattributes_present_no_trailing_newline() {
        assert!(has_gitattributes_rule("*.qual merge=union"));
    }

    #[test]
    fn gitattributes_present_leading_whitespace() {
        assert!(has_gitattributes_rule("  *.qual merge=union\n"));
    }

    #[test]
    fn gitattributes_present_with_trailing_comment() {
        assert!(has_gitattributes_rule("*.qual merge=union # qualifier\n"));
    }

    #[test]
    fn gitattributes_present_among_other_rules() {
        let content = "*.png binary\n*.qual merge=union\n*.lock -diff\n";
        assert!(has_gitattributes_rule(content));
    }

    #[test]
    fn gitattributes_absent_empty() {
        assert!(!has_gitattributes_rule(""));
    }

    #[test]
    fn gitattributes_absent_other_rules_only() {
        assert!(!has_gitattributes_rule("*.png binary\n*.lock -diff\n"));
    }

    #[test]
    fn gitattributes_absent_different_attr() {
        assert!(!has_gitattributes_rule("*.qual text\n"));
    }

    #[test]
    fn gitattributes_absent_different_pattern() {
        assert!(!has_gitattributes_rule("*.qualifier merge=union\n"));
    }

    #[test]
    fn append_gitattributes_to_empty_file() {
        assert_eq!(append_gitattributes_rule(""), "*.qual merge=union\n");
    }

    #[test]
    fn append_gitattributes_to_newline_terminated_content() {
        assert_eq!(
            append_gitattributes_rule("*.png binary\n"),
            "*.png binary\n*.qual merge=union\n"
        );
    }

    #[test]
    fn append_gitattributes_to_non_newline_terminated_content() {
        assert_eq!(
            append_gitattributes_rule("*.png binary"),
            "*.png binary\n*.qual merge=union\n"
        );
    }

    #[test]
    fn directive_present_exact() {
        assert!(already_has_directive("Run `qualifier agents` first.\n"));
    }

    #[test]
    fn directive_present_mixed_case() {
        assert!(already_has_directive("See Qualifier Agents for context.\n"));
    }

    #[test]
    fn directive_present_inside_prose() {
        let content = "# AGENTS.md\n\nWhen working on this repo, consult qualifier agents.\n";
        assert!(already_has_directive(content));
    }

    #[test]
    fn directive_absent_hyphenated_form() {
        // Hyphen is not a space — should NOT match.
        assert!(!already_has_directive(
            "Run qualifier-agents to learn more.\n"
        ));
    }

    #[test]
    fn directive_absent_empty() {
        assert!(!already_has_directive(""));
    }

    #[test]
    fn directive_absent_unrelated() {
        assert!(!already_has_directive(
            "# AGENTS.md\n\nBe nice. Test things.\n"
        ));
    }

    #[test]
    fn append_directive_to_empty_file() {
        assert_eq!(
            append_agent_directive(""),
            "If qualifier might be relevant, run `qualifier agents` first.\n"
        );
    }

    #[test]
    fn append_directive_to_newline_terminated_content() {
        let existing = "# AGENTS.md\n\nBe nice.\n";
        let expected = "# AGENTS.md\n\nBe nice.\n\nIf qualifier might be relevant, run `qualifier agents` first.\n";
        assert_eq!(append_agent_directive(existing), expected);
    }

    #[test]
    fn append_directive_to_non_newline_terminated_content() {
        let existing = "# AGENTS.md\n\nBe nice.";
        let expected = "# AGENTS.md\n\nBe nice.\n\nIf qualifier might be relevant, run `qualifier agents` first.\n";
        assert_eq!(append_agent_directive(existing), expected);
    }

    #[test]
    fn gitattributes_trailing_hash_is_an_attribute_not_a_comment() {
        // Git only treats `#` as a comment at the start of a line, so the
        // trailing tokens are extra attributes; `merge=union` still holds.
        assert!(has_gitattributes_rule("*.qual merge=union # qualifier\n"));
    }

    #[test]
    fn gitattributes_commented_out_line_is_ignored() {
        assert!(!has_gitattributes_rule("# *.qual merge=union\n"));
    }

    #[test]
    fn gitattributes_present_with_extra_attributes() {
        assert!(has_gitattributes_rule("*.qual merge=union -diff\n"));
    }

    #[test]
    fn gitattributes_present_with_double_star_pattern() {
        assert!(has_gitattributes_rule("**/*.qual merge=union\n"));
    }

    #[test]
    fn gitattributes_present_with_catch_all_pattern() {
        assert!(has_gitattributes_rule("* merge=union\n"));
    }

    #[test]
    fn gitattributes_later_unset_wins() {
        assert!(!has_gitattributes_rule(
            "*.qual merge=union\n*.qual -merge\n"
        ));
    }

    #[test]
    fn gitattributes_later_binary_macro_wins() {
        assert!(!has_gitattributes_rule(
            "*.qual merge=union\n*.qual binary\n"
        ));
    }

    #[test]
    fn gitattributes_later_union_overrides_earlier_driver() {
        assert!(has_gitattributes_rule(
            "*.qual merge=ours\n*.qual merge=union\n"
        ));
    }

    #[test]
    fn gitattributes_macro_definition_is_not_a_pattern() {
        assert!(!has_gitattributes_rule("[attr]qual merge=union\n"));
    }

    #[test]
    fn gitattributes_crlf_content_is_parsed() {
        assert!(has_gitattributes_rule(
            "*.png binary\r\n*.qual merge=union\r\n"
        ));
    }

    #[test]
    fn append_gitattributes_preserves_crlf() {
        assert_eq!(
            append_gitattributes_rule("*.png binary\r\n"),
            "*.png binary\r\n*.qual merge=union\r\n"
        );
    }

    #[test]
    fn append_directive_preserves_crlf() {
        assert_eq!(
            append_agent_directive("# Rules\r\n"),
            format!("# Rules\r\n\r\n{DIRECTIVE}\r\n")
        );
    }

    /// Run the init steps against `root` with scripted stdin; return the
    /// result and everything written to stdout.
    fn run_scripted(root: &Path, mode: Mode, input: &str) -> (crate::Result<()>, String) {
        let mut reader = io::Cursor::new(input.as_bytes().to_vec());
        let mut out = Vec::new();
        let result = run_steps(root, mode, &mut reader, &mut out);
        (result, String::from_utf8(out).unwrap())
    }

    fn git_root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        dir
    }

    #[test]
    fn interactive_eof_aborts_without_writing() {
        let dir = git_root();
        let (result, _) = run_scripted(dir.path(), Mode::Interactive, "");
        let err = result.expect_err("EOF at a prompt must abort");
        assert!(err.to_string().contains("end of input"), "{err}");
        assert!(!dir.path().join(".gitattributes").exists());
        assert!(!dir.path().join("AGENTS.md").exists());
    }

    #[test]
    fn interactive_eof_after_first_answer_keeps_earlier_change_only() {
        let dir = git_root();
        let (result, _) = run_scripted(dir.path(), Mode::Interactive, "y\n");
        assert!(result.is_err());
        assert!(dir.path().join(".gitattributes").exists());
        assert!(!dir.path().join("AGENTS.md").exists());
    }

    #[test]
    fn interactive_answers_apply_and_decline_per_step() {
        let dir = git_root();
        let (result, out) = run_scripted(dir.path(), Mode::Interactive, "n\ny\n");
        result.unwrap();
        assert!(!dir.path().join(".gitattributes").exists());
        assert!(dir.path().join("AGENTS.md").exists());
        assert!(out.contains("init: applied 1, skipped 1."), "{out}");
    }

    #[test]
    fn interactive_empty_answer_takes_default_yes() {
        let dir = git_root();
        let (result, out) = run_scripted(dir.path(), Mode::Interactive, "\n\n");
        result.unwrap();
        assert!(dir.path().join(".gitattributes").exists());
        assert!(dir.path().join("AGENTS.md").exists());
        assert!(out.contains("init: applied 2, skipped 0."), "{out}");
    }

    #[test]
    fn interactive_reprompts_on_unrecognized_answer() {
        let dir = git_root();
        let (result, out) = run_scripted(dir.path(), Mode::Interactive, "maybe\nyes\nNO\n");
        result.unwrap();
        assert!(out.contains("please answer y or n"), "{out}");
        assert!(dir.path().join(".gitattributes").exists());
        assert!(!dir.path().join("AGENTS.md").exists());
    }

    #[test]
    fn yes_mode_prints_changes_without_prompts() {
        let dir = git_root();
        let (result, out) = run_scripted(dir.path(), Mode::Yes, "");
        result.unwrap();
        assert!(out.contains("+ *.qual merge=union"), "{out}");
        assert!(!out.contains("Apply?"), "{out}");
        assert!(out.contains("init: applied 2, skipped 0."), "{out}");
    }

    #[test]
    fn already_configured_steps_print_once() {
        let dir = git_root();
        fs::write(dir.path().join(".gitattributes"), "*.qual merge=union\n").unwrap();
        let (result, out) = run_scripted(dir.path(), Mode::Yes, "");
        result.unwrap();
        assert_eq!(out.matches(".gitattributes").count(), 1, "{out}");
    }

    #[test]
    fn dry_run_summary_reports_would_apply() {
        let dir = git_root();
        let (result, out) = run_scripted(dir.path(), Mode::DryRun, "");
        result.unwrap();
        assert!(
            out.contains("init (dry run): would apply 2, skipped 0."),
            "{out}"
        );
        assert!(!out.contains("applied"), "{out}");
        assert!(!dir.path().join(".gitattributes").exists());
    }

    #[test]
    fn hg_hint_gives_hgrc_snippet() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".hg")).unwrap();
        let (result, out) = run_scripted(dir.path(), Mode::Yes, "");
        result.unwrap();
        assert!(out.contains(".hg/hgrc"), "{out}");
        assert!(out.contains("[merge-patterns]"), "{out}");
        assert!(out.contains("**.qual = :union"), "{out}");
    }

    #[test]
    fn jj_hint_does_not_promise_union_merge() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join(".jj")).unwrap();
        let (result, out) = run_scripted(dir.path(), Mode::Yes, "");
        result.unwrap();
        assert!(out.contains("jj"), "{out}");
        assert!(out.contains("keep both sides"), "{out}");
    }

    #[cfg(unix)]
    #[test]
    fn read_failure_is_io_error_naming_the_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("CLAUDE.md");
        fs::write(&path, "# rules\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read_to_string(&path).is_ok() {
            return; // running as root; permissions are not enforced
        }
        let (result, _) = run_scripted(dir.path(), Mode::Yes, "");
        let err = result.expect_err("unreadable agent file must fail");
        assert!(matches!(err, crate::Error::Io(_)), "{err:?}");
        assert!(err.to_string().contains("CLAUDE.md"), "{err}");
    }

    fn rel_paths(root: &Path) -> Vec<String> {
        discover_agent_files(root)
            .into_iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn discovers_additional_agent_conventions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("CONVENTIONS.md"), "x\n").unwrap();
        fs::write(root.join(".clinerules"), "x\n").unwrap();
        fs::create_dir_all(root.join(".github/instructions")).unwrap();
        fs::write(
            root.join(".github/instructions/rust.instructions.md"),
            "x\n",
        )
        .unwrap();
        fs::write(root.join(".github/instructions/notes.md"), "x\n").unwrap();
        fs::create_dir_all(root.join(".junie")).unwrap();
        fs::write(root.join(".junie/guidelines.md"), "x\n").unwrap();

        assert_eq!(
            rel_paths(root),
            vec![
                "CONVENTIONS.md",
                ".clinerules",
                ".github/instructions/rust.instructions.md",
                ".junie/guidelines.md",
            ]
        );
    }

    #[test]
    fn discovers_clinerules_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".clinerules")).unwrap();
        fs::write(dir.path().join(".clinerules/01-style.md"), "x\n").unwrap();
        fs::write(dir.path().join(".clinerules/notes.txt"), "x\n").unwrap();
        assert_eq!(rel_paths(dir.path()), vec![".clinerules/01-style.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_agent_file_is_listed_once() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# rules\n").unwrap();
        std::os::unix::fs::symlink("AGENTS.md", dir.path().join("CLAUDE.md")).unwrap();
        assert_eq!(rel_paths(dir.path()), vec!["AGENTS.md"]);

        let (result, out) = run_scripted(dir.path(), Mode::DryRun, "");
        result.unwrap();
        assert!(!out.contains("CLAUDE.md"), "{out}");
    }
}
