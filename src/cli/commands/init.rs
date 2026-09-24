//! `qualifier init` — interactive, idempotent project bootstrap.
//!
//! Configures VCS merge behavior and inserts a directive into agent
//! instruction files that points coding agents at `qualifier agents`.
//! See `docs/superpowers/specs/2026-05-13-qualifier-init-design.md`.

use clap::Parser;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

#[derive(Parser, Debug)]
pub struct Args {
    /// Auto-accept every step at its default.
    #[arg(short, long)]
    pub yes: bool,

    /// Report what would change without writing or prompting.
    #[arg(long, conflicts_with = "yes")]
    pub dry_run: bool,
}

enum StepOutcome {
    Applied(String),
    Skipped(String),
}

fn step_vcs_merge_config<W: Write>(
    out: &mut W,
    root: &Path,
    mode: Mode,
) -> crate::Result<StepOutcome> {
    match crate::qual_file::detect_vcs(root) {
        Some("git") => step_git_gitattributes(out, root, mode),
        Some(vcs) => Ok(StepOutcome::Skipped(format!(
            "{vcs} detected — configure union merge for *.qual (see SPEC §8.2)"
        ))),
        None => Ok(StepOutcome::Skipped(
            "no VCS detected — skipping merge configuration".into(),
        )),
    }
}

fn step_git_gitattributes<W: Write>(
    out: &mut W,
    root: &Path,
    mode: Mode,
) -> crate::Result<StepOutcome> {
    let path = root.join(".gitattributes");
    let existing = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(read_error(&path, e)),
    };

    if has_gitattributes_rule(&existing) {
        return Ok(StepOutcome::Skipped(
            ".gitattributes: *.qual merge=union already configured".into(),
        ));
    }

    describe_to(out, ".gitattributes", &["*.qual merge=union"])?;
    if confirm_to(out, "Apply?", true, mode)? {
        let new_content = append_gitattributes_rule(&existing);
        write_file(&path, &new_content)?;
        Ok(StepOutcome::Applied(
            ".gitattributes: added *.qual merge=union".into(),
        ))
    } else {
        Ok(StepOutcome::Skipped(format!(
            ".gitattributes: {}",
            decline_label(mode)
        )))
    }
}

#[derive(Clone, Copy)]
enum AgentKind {
    File,
    Dir,
}

const AGENT_FILES: &[(&str, AgentKind)] = &[
    ("AGENTS.md", AgentKind::File),
    ("CLAUDE.md", AgentKind::File),
    ("GEMINI.md", AgentKind::File),
    (".cursorrules", AgentKind::File),
    (".windsurfrules", AgentKind::File),
    (".github/copilot-instructions.md", AgentKind::File),
    (".cursor/rules", AgentKind::Dir),
];

/// Resolve the discovery table against `root` into a list of existing
/// agent-file paths. For `AgentKind::Dir` entries, walks the directory
/// one level deep and collects `*.md` and `*.mdc` children sorted by
/// name.
fn discover_agent_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for (rel, kind) in AGENT_FILES {
        let full = root.join(rel);
        match kind {
            AgentKind::File => {
                if full.is_file() {
                    found.push(full);
                }
            }
            AgentKind::Dir => {
                if let Ok(entries) = fs::read_dir(&full) {
                    let mut children: Vec<std::path::PathBuf> = entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.is_file())
                        .filter(|p| {
                            matches!(
                                p.extension().and_then(|s| s.to_str()),
                                Some("md") | Some("mdc")
                            )
                        })
                        .collect();
                    children.sort();
                    found.extend(children);
                }
            }
        }
    }
    found
}

fn step_agent_directive<W: Write>(
    out: &mut W,
    root: &Path,
    mode: Mode,
) -> crate::Result<Vec<StepOutcome>> {
    let files = discover_agent_files(root);
    if files.is_empty() {
        let path = root.join("AGENTS.md");
        describe_to(out, "AGENTS.md (new file)", &[DIRECTIVE])?;
        if confirm_to(out, "Create AGENTS.md?", true, mode)? {
            write_file(&path, &append_agent_directive(""))?;
            return Ok(vec![StepOutcome::Applied("AGENTS.md: created".into())]);
        } else {
            return Ok(vec![StepOutcome::Skipped(format!(
                "AGENTS.md: {} creation",
                decline_label(mode)
            ))]);
        }
    }

    let mut outcomes = Vec::with_capacity(files.len());
    for path in files {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        let existing = fs::read_to_string(&path).map_err(|e| read_error(&path, e))?;
        if already_has_directive(&existing) {
            outcomes.push(StepOutcome::Skipped(format!(
                "{rel}: already references `qualifier agents`"
            )));
            continue;
        }
        describe_to(out, &rel, &[DIRECTIVE])?;
        if confirm_to(out, "Apply?", true, mode)? {
            let new_content = append_agent_directive(&existing);
            write_file(&path, &new_content)?;
            outcomes.push(StepOutcome::Applied(format!("{rel}: directive appended")));
        } else {
            outcomes.push(StepOutcome::Skipped(format!(
                "{rel}: {}",
                decline_label(mode)
            )));
        }
    }
    Ok(outcomes)
}

/// Word used in skip messages when the user says no (or `--dry-run`
/// short-circuits the prompt). Keeping these distinct in the summary
/// makes a dry-run transcript readable.
fn decline_label(mode: Mode) -> &'static str {
    match mode {
        Mode::DryRun => "previewed (dry-run)",
        _ => "declined",
    }
}

/// Wrap an I/O read error with the path that failed so the user can
/// tell which agent file (out of several discovered) was unreadable.
fn read_error(path: &Path, err: io::Error) -> crate::Error {
    crate::Error::Validation(format!("init: failed to read {}: {err}", path.display()))
}

/// Wrap an I/O write error with the path that failed.
fn write_file(path: &Path, content: &str) -> crate::Result<()> {
    fs::write(path, content).map_err(|e| {
        crate::Error::Validation(format!("init: failed to write {}: {e}", path.display()))
    })
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
    if mode == Mode::Interactive && !std::io::stdin().is_terminal() {
        return Err(crate::Error::Validation(
            "init: refusing to prompt without a TTY; pass --yes or --dry-run".into(),
        ));
    }

    let cwd = std::env::current_dir()?;
    let root = crate::qual_file::find_project_root(&cwd).unwrap_or(cwd);

    let mut stdout = io::stdout().lock();
    let mut outcomes = Vec::new();
    outcomes.push(step_vcs_merge_config(&mut stdout, &root, mode)?);
    outcomes.extend(step_agent_directive(&mut stdout, &root, mode)?);

    let mut applied = 0usize;
    let mut skipped = 0usize;
    for o in &outcomes {
        match o {
            StepOutcome::Applied(msg) => {
                applied += 1;
                writeln!(stdout, "{msg}")?;
            }
            StepOutcome::Skipped(msg) => {
                skipped += 1;
                writeln!(stdout, "{msg}")?;
            }
        }
    }
    writeln!(stdout, "init: applied {applied}, skipped {skipped}.")?;
    Ok(())
}

/// True if `content` (the body of `.gitattributes`) already contains a
/// line whose non-whitespace, non-comment tokens are exactly
/// `*.qual` followed by `merge=union`.
fn has_gitattributes_rule(content: &str) -> bool {
    content.lines().any(|line| {
        // Strip trailing comment.
        let no_comment = line.split('#').next().unwrap_or("");
        let mut toks = no_comment.split_whitespace();
        toks.next() == Some("*.qual") && toks.next() == Some("merge=union") && toks.next().is_none()
    })
}

/// Build the new `.gitattributes` content with the qualifier rule
/// appended. Guarantees the prior content is separated by exactly one
/// `\n` and the file ends with a `\n`.
fn append_gitattributes_rule(existing: &str) -> String {
    if existing.is_empty() {
        return "*.qual merge=union\n".to_string();
    }
    let needs_sep = !existing.ends_with('\n');
    let mut out = String::with_capacity(existing.len() + 32);
    out.push_str(existing);
    if needs_sep {
        out.push('\n');
    }
    out.push_str("*.qual merge=union\n");
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
/// Empty input → just the directive plus `\n`.
/// Non-empty input → ensure a trailing newline, then a blank line,
/// then the directive plus `\n`.
fn append_agent_directive(existing: &str) -> String {
    if existing.is_empty() {
        return format!("{DIRECTIVE}\n");
    }
    let mut out = String::with_capacity(existing.len() + DIRECTIVE.len() + 4);
    out.push_str(existing);
    if !existing.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(DIRECTIVE);
    out.push('\n');
    out
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

/// Print a path + the lines that would be appended.
fn describe_to<W: Write>(out: &mut W, path: &str, additions: &[&str]) -> io::Result<()> {
    writeln!(out, "{path}")?;
    for line in additions {
        writeln!(out, "  + {line}")?;
    }
    Ok(())
}

/// Prompt for confirmation, writing to `out` and reading from stdin in
/// interactive mode. Non-interactive modes never read stdin; they just
/// print a single transcript line indicating what would happen.
fn confirm_to<W: Write>(
    out: &mut W,
    prompt: &str,
    default_yes: bool,
    mode: Mode,
) -> io::Result<bool> {
    let suffix = if default_yes { "[Y/n]" } else { "[y/N]" };
    match mode {
        Mode::Interactive => loop {
            write!(out, "{prompt} {suffix} ")?;
            out.flush()?;
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            let answer = line.trim().to_ascii_lowercase();
            match answer.as_str() {
                "" => return Ok(default_yes),
                "y" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => {
                    writeln!(out, "please answer y or n")?;
                }
            }
        },
        Mode::Yes => {
            let tag = if default_yes {
                "[applying]"
            } else {
                "[skipping]"
            };
            writeln!(out, "{prompt} {suffix} {tag}")?;
            Ok(default_yes)
        }
        Mode::DryRun => {
            writeln!(out, "{prompt} {suffix} [dry-run]")?;
            Ok(false)
        }
    }
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
    fn confirm_yes_mode_returns_default_true() {
        let mut out = Vec::new();
        let result = confirm_to(&mut out, "Apply?", true, Mode::Yes).unwrap();
        assert!(result);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("Apply?"));
        assert!(s.contains("[applying]"));
    }

    #[test]
    fn confirm_yes_mode_with_default_false_returns_false() {
        let mut out = Vec::new();
        let result = confirm_to(&mut out, "Apply?", false, Mode::Yes).unwrap();
        assert!(!result);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("[skipping]"));
    }

    #[test]
    fn confirm_dry_run_mode_returns_false() {
        let mut out = Vec::new();
        let result = confirm_to(&mut out, "Apply?", true, Mode::DryRun).unwrap();
        assert!(!result);
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("[dry-run]"));
    }

    #[test]
    fn describe_lists_path_and_additions() {
        let mut out = Vec::new();
        describe_to(&mut out, ".gitattributes", &["*.qual merge=union"]).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains(".gitattributes"));
        assert!(s.contains("+ *.qual merge=union"));
    }
}
