# `qualifier init` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `qualifier init` subcommand that interactively bootstraps a project with VCS merge config and an agent-instruction directive, with `--yes` / `--dry-run` flags and idempotent re-runs.

**Architecture:** Single new file `src/cli/commands/init.rs` containing pure helpers (string-in / string-out), a `confirm` / `describe` UI layer keyed on a `Mode` enum (Interactive / Yes / DryRun), and two step functions wired into a new `Commands::Init` variant in `src/cli/mod.rs`. All file system interaction is scoped to the project root resolved by the existing `qual_file::find_project_root`. Pure helpers get unit tests in-file; CLI behavior gets integration tests in `tests/cli_integration.rs` driven by the existing `run_qualifier(dir, args)` harness.

**Tech Stack:** Rust 2024, clap 4 (derive), `std::io::IsTerminal`, existing `qual_file::detect_vcs` for VCS detection, `tempfile` for tests.

**Spec:** `docs/superpowers/specs/2026-05-13-qualifier-init-design.md`

---

## File Structure

**Create:**
- `src/cli/commands/init.rs` — Args, Mode, helpers, steps, `run()`.

**Modify:**
- `src/cli/commands/mod.rs` — register `pub mod init;`.
- `src/cli/mod.rs` — add `Init(commands::init::Args)` variant; add dispatch arm; add `Initialize:` section to `HELP_TEMPLATE`.
- `tests/cli_integration.rs` — append init integration tests at end of file.
- `SPEC.md` — remove `qualifier init` from §12 Future Considerations; add brief mention in §8.2 that init automates the merge config step.
- `README.md` — add `init` to CLI Commands table/listing.
- `CHANGELOG.md` — entry under new version.
- `Cargo.toml` — bump `version` from `0.6.1` to `0.7.0`.
- `Cargo.lock` — rebuild via `cargo build`.

---

### Task 1: Scaffold the `init` subcommand (no behavior)

Wire the command into clap and the help template so `qualifier init` and `qualifier init --help` succeed. Behavior comes in later tasks.

**Files:**
- Create: `src/cli/commands/init.rs`
- Modify: `src/cli/commands/mod.rs`
- Modify: `src/cli/mod.rs`
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing integration test**

Append at the bottom of `tests/cli_integration.rs`:

```rust
// --- qualifier init: scaffolding ---

#[test]
fn test_init_help_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_qualifier(dir.path(), &["init", "--help"]);
    assert_eq!(code, 0, "init --help should succeed");
    assert!(stdout.contains("--yes"), "help should mention --yes flag");
    assert!(stdout.contains("--dry-run"), "help should mention --dry-run flag");
}

#[test]
fn test_init_appears_in_top_level_help() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_qualifier(dir.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Initialize:"), "top-level help should have Initialize section");
    assert!(stdout.contains("init"), "top-level help should list init command");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_integration test_init -- --nocapture`
Expected: FAIL — `init` subcommand doesn't exist yet.

- [ ] **Step 3: Create the init module with scaffold**

Create `src/cli/commands/init.rs`:

```rust
//! `qualifier init` — interactive, idempotent project bootstrap.
//!
//! Configures VCS merge behavior and inserts a directive into agent
//! instruction files that points coding agents at `qualifier agents`.
//! See `docs/superpowers/specs/2026-05-13-qualifier-init-design.md`.

use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Auto-accept every step at its default.
    #[arg(short, long)]
    pub yes: bool,

    /// Report what would change without writing or prompting.
    #[arg(long, conflicts_with = "yes")]
    pub dry_run: bool,
}

#[cfg(target_os = "emscripten")]
pub fn run(_args: Args) -> crate::Result<()> {
    Err(crate::Error::Validation(
        "init is not available in the browser".into(),
    ))
}

#[cfg(not(target_os = "emscripten"))]
pub fn run(_args: Args) -> crate::Result<()> {
    // Behavior added in subsequent tasks.
    Ok(())
}
```

- [ ] **Step 4: Register the module**

Edit `src/cli/commands/mod.rs`. Add `pub mod init;` in alphabetical order:

```rust
pub mod agents;
pub mod compact;
pub mod diff;
pub mod emit;
pub mod freshness;
pub mod haiku;
pub mod init;
pub mod ls;
pub mod praise;
pub mod record;
pub mod reply;
pub mod resolve;
pub mod show;
```

- [ ] **Step 5: Wire the variant and dispatch in cli/mod.rs**

In `src/cli/mod.rs`, edit `HELP_TEMPLATE` — add a new `Initialize:` section above `For AI agents:`:

```rust
const HELP_TEMPLATE: &str = "\
{about-with-newline}
{usage-heading} {usage}

If you are an AI coding agent, run `qualifier agents` first — it covers
the conventions and pitfalls you need before recording any annotation.

Initialize:
  init       Bootstrap a project: VCS merge config and agent directives

For AI agents:
  agents     Read this before recording annotations. Self-contained agent guide.

Record observations:
  record     Record an annotation: `qualifier record <kind> <location> [message]`
  reply      Reply to an existing record (id-prefix or location)
  resolve    Resolve (close) an existing record (id-prefix or location)
  emit       Emit a raw record of any type

Inspect annotations:
  show       Show annotations for an artifact
  ls         List artifacts by kind
  praise     Show who annotated an artifact and why (alias: blame)
  review     Check freshness of annotations against current code
  diff       Show records added, resolved, or drifted since a git ref

Maintain:
  compact    Compact a .qual file

Other:
  haiku      Print a random qualifier haiku
  help       Print this message or the help of the given subcommand(s)

Run `qualifier <COMMAND> --help` for command-specific options.

Options:
{options}
";
```

In the `Commands` enum, add the `Init` variant just before `Agents`:

```rust
#[derive(Subcommand)]
pub enum Commands {
    /// Bootstrap a project: VCS merge config and agent directives
    Init(commands::init::Args),

    /// Read this before recording annotations. Self-contained agent guide.
    Agents(commands::agents::Args),
    // ... rest unchanged
```

In the `match cli.command` block in `run()`, add the dispatch arm right before the `Agents` arm:

```rust
    let result: crate::Result<()> = match cli.command {
        Commands::Init(args) => commands::init::run(args),
        Commands::Agents(args) => commands::agents::run(args),
        // ... rest unchanged
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --test cli_integration test_init -- --nocapture`
Expected: PASS — both `test_init_help_runs` and `test_init_appears_in_top_level_help`.

- [ ] **Step 7: Verify the whole suite still passes**

Run: `cargo test --all-features`
Expected: all tests pass.

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 8: Commit**

```bash
git add src/cli/commands/init.rs src/cli/commands/mod.rs src/cli/mod.rs tests/cli_integration.rs
git commit -m "feat(init): scaffold \`qualifier init\` subcommand"
```

---

### Task 2: Confirm `--yes` / `--dry-run` conflict

Verify clap rejects mutually exclusive flags as designed.

**Files:**
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing test**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_yes_and_dry_run_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let (_, stderr, code) = run_qualifier(dir.path(), &["init", "--yes", "--dry-run"]);
    assert_ne!(code, 0, "conflicting flags should fail");
    assert!(
        stderr.contains("--dry-run") || stderr.contains("--yes") || stderr.contains("conflict"),
        "stderr should explain the conflict: {stderr}"
    );
}
```

- [ ] **Step 2: Run test to verify it passes already**

Run: `cargo test --test cli_integration test_init_yes_and_dry_run_conflict -- --nocapture`
Expected: PASS — clap's `conflicts_with = "yes"` is already in place from Task 1.

- [ ] **Step 3: Commit**

```bash
git add tests/cli_integration.rs
git commit -m "test(init): assert --yes and --dry-run are mutually exclusive"
```

---

### Task 3: Pure helper — `has_gitattributes_rule`

Detect whether `.gitattributes` already contains the `*.qual merge=union` rule. Line-anchored, tolerant of leading whitespace and trailing comments.

**Files:**
- Modify: `src/cli/commands/init.rs`

- [ ] **Step 1: Write the failing unit tests**

Append to `src/cli/commands/init.rs`:

```rust
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib init::tests -- --nocapture`
Expected: FAIL — `has_gitattributes_rule` not defined.

- [ ] **Step 3: Implement the helper**

Add to `src/cli/commands/init.rs` above the `#[cfg(test)]` block:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib init::tests -- --nocapture`
Expected: PASS — all 9 cases.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs
git commit -m "feat(init): add has_gitattributes_rule helper"
```

---

### Task 4: Pure helper — `append_gitattributes_rule`

Given existing `.gitattributes` content, produce the new content with `*.qual merge=union` appended and exactly one newline separating it from prior content.

**Files:**
- Modify: `src/cli/commands/init.rs`

- [ ] **Step 1: Write the failing unit tests**

Add inside the `mod tests` block in `src/cli/commands/init.rs`:

```rust
    #[test]
    fn append_gitattributes_to_empty_file() {
        assert_eq!(
            append_gitattributes_rule(""),
            "*.qual merge=union\n"
        );
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib init::tests::append_gitattributes -- --nocapture`
Expected: FAIL — `append_gitattributes_rule` not defined.

- [ ] **Step 3: Implement the helper**

Add to `src/cli/commands/init.rs` just below `has_gitattributes_rule`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib init::tests -- --nocapture`
Expected: PASS — all gitattributes tests.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs
git commit -m "feat(init): add append_gitattributes_rule helper"
```

---

### Task 5: Pure helper — `already_has_directive`

Detect whether an agent-instruction file already references `qualifier agents`. Case-insensitive substring search.

**Files:**
- Modify: `src/cli/commands/init.rs`

- [ ] **Step 1: Write the failing unit tests**

Add inside the `mod tests` block in `src/cli/commands/init.rs`:

```rust
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
        assert!(!already_has_directive("Run qualifier-agents to learn more.\n"));
    }

    #[test]
    fn directive_absent_empty() {
        assert!(!already_has_directive(""));
    }

    #[test]
    fn directive_absent_unrelated() {
        assert!(!already_has_directive("# AGENTS.md\n\nBe nice. Test things.\n"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib init::tests::directive -- --nocapture`
Expected: FAIL — `already_has_directive` not defined.

- [ ] **Step 3: Implement the helper**

Add to `src/cli/commands/init.rs`:

```rust
/// True if `content` already mentions `qualifier agents`
/// (case-insensitive substring). Used to avoid duplicate directive
/// insertion on re-runs and to respect existing documentation that
/// already points readers at the subcommand.
fn already_has_directive(content: &str) -> bool {
    content.to_ascii_lowercase().contains("qualifier agents")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib init::tests -- --nocapture`
Expected: PASS — all directive tests.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs
git commit -m "feat(init): add already_has_directive helper"
```

---

### Task 6: Pure helper — `append_agent_directive`

Given existing agent-file content, produce the new content with the directive line appended. Empty file → just the directive; non-empty → blank line separator + directive.

**Files:**
- Modify: `src/cli/commands/init.rs`

- [ ] **Step 1: Write the failing unit tests**

Add inside the `mod tests` block in `src/cli/commands/init.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib init::tests::append_directive -- --nocapture`
Expected: FAIL — `append_agent_directive` and `DIRECTIVE` not defined.

- [ ] **Step 3: Implement the helper**

Add to `src/cli/commands/init.rs`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib init::tests -- --nocapture`
Expected: PASS — all append_directive tests.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs
git commit -m "feat(init): add append_agent_directive helper"
```

---

### Task 7: `Mode` enum + `confirm` / `describe` helpers

Wire up the interactive/auto/dry-run prompt machinery. Unit test the non-interactive paths.

**Files:**
- Modify: `src/cli/commands/init.rs`

- [ ] **Step 1: Write the failing unit tests**

Add inside the `mod tests` block in `src/cli/commands/init.rs`:

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib init::tests::confirm -- --nocapture`
Expected: FAIL — `Mode`, `confirm_to`, `describe_to` not defined.

- [ ] **Step 3: Implement the types and helpers**

Add to `src/cli/commands/init.rs` (above the `#[cfg(test)]` block):

```rust
use std::io::{self, BufRead, Write};

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
        Mode::Interactive => {
            loop {
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
            }
        }
        Mode::Yes => {
            let tag = if default_yes { "[applying]" } else { "[skipping]" };
            writeln!(out, "{prompt} {suffix} {tag}")?;
            Ok(default_yes)
        }
        Mode::DryRun => {
            writeln!(out, "{prompt} {suffix} [dry-run]")?;
            Ok(false)
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib init::tests -- --nocapture`
Expected: PASS — all confirm/describe tests.

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs
git commit -m "feat(init): add Mode, confirm, and describe helpers"
```

---

### Task 8: TTY guard at top of `run()`

When neither `--yes` nor `--dry-run` is passed and stdin is not a TTY, refuse to prompt with a clear error.

**Files:**
- Modify: `src/cli/commands/init.rs`
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing integration test**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_non_tty_without_flag_refuses() {
    let dir = tempfile::tempdir().unwrap();
    // run_qualifier already pipes stdin (not a TTY).
    let (_, stderr, code) = run_qualifier(dir.path(), &["init"]);
    assert_ne!(code, 0, "should refuse to prompt without a TTY");
    assert!(
        stderr.contains("TTY") || stderr.contains("tty"),
        "stderr should mention TTY: {stderr}"
    );
    assert!(
        stderr.contains("--yes") || stderr.contains("--dry-run"),
        "stderr should suggest --yes or --dry-run: {stderr}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_integration test_init_non_tty -- --nocapture`
Expected: FAIL — `run()` currently exits 0.

- [ ] **Step 3: Implement the guard**

Replace the non-emscripten `run()` body in `src/cli/commands/init.rs`:

```rust
#[cfg(not(target_os = "emscripten"))]
pub fn run(args: Args) -> crate::Result<()> {
    use std::io::IsTerminal;

    let mode = mode_from_args(&args);
    if mode == Mode::Interactive && !std::io::stdin().is_terminal() {
        return Err(crate::Error::Validation(
            "init: refusing to prompt without a TTY; pass --yes or --dry-run".into(),
        ));
    }
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli_integration test_init_non_tty -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs tests/cli_integration.rs
git commit -m "feat(init): refuse interactive prompts without a TTY"
```

---

### Task 9: `step_vcs_merge_config` — git path

Implement the first step. Detect git, read/write `.gitattributes`, return `StepOutcome`. Integration tests cover fresh git repo and idempotency.

**Files:**
- Modify: `src/cli/commands/init.rs`
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing integration tests**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_git_creates_gitattributes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0, "init --yes should succeed: {stdout}");

    let gitattr = std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert!(
        gitattr.contains("*.qual merge=union"),
        ".gitattributes should contain the rule: {gitattr:?}"
    );
}

#[test]
fn test_init_git_idempotent_gitattributes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join(".gitattributes"), "*.qual merge=union\n").unwrap();

    let (_, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);

    let gitattr = std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap();
    assert_eq!(
        gitattr, "*.qual merge=union\n",
        ".gitattributes should be unchanged byte-for-byte"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_integration test_init_git -- --nocapture`
Expected: FAIL — `run()` doesn't write `.gitattributes` yet.

- [ ] **Step 3: Implement the step and outcome plumbing**

In `src/cli/commands/init.rs`, add above `run()`:

```rust
use std::fs;
use std::path::Path;

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
        Err(e) => return Err(crate::Error::Io(e)),
    };

    if has_gitattributes_rule(&existing) {
        return Ok(StepOutcome::Skipped(
            ".gitattributes: *.qual merge=union already configured".into(),
        ));
    }

    describe_to(out, ".gitattributes", &["*.qual merge=union"])?;
    if confirm_to(out, "Apply?", true, mode)? {
        let new_content = append_gitattributes_rule(&existing);
        fs::write(&path, new_content)?;
        Ok(StepOutcome::Applied(
            ".gitattributes: added *.qual merge=union".into(),
        ))
    } else {
        Ok(StepOutcome::Skipped(".gitattributes: declined".into()))
    }
}
```

Update `run()` to call the step:

```rust
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
    let outcome = step_vcs_merge_config(&mut stdout, &root, mode)?;
    match outcome {
        StepOutcome::Applied(msg) | StepOutcome::Skipped(msg) => {
            writeln!(stdout, "{msg}")?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test cli_integration test_init_git -- --nocapture`
Expected: PASS — both `test_init_git_creates_gitattributes` and `test_init_git_idempotent_gitattributes`.

Run: `cargo test --all-features`
Expected: full suite passes.

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs tests/cli_integration.rs
git commit -m "feat(init): wire VCS merge-config step (git path)"
```

---

### Task 10: `step_vcs_merge_config` — non-git VCS + no-VCS coverage

Add integration tests for the hg/jj branches and the no-VCS path. The implementation already covers them (it returns `Skipped` with a hint); we just verify the behavior.

**Files:**
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_hg_prints_hint_no_gitattributes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".hg")).unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("hg"),
        "stdout should mention hg: {stdout}"
    );
    assert!(
        !dir.path().join(".gitattributes").exists(),
        ".gitattributes should not be created for hg repos"
    );
}

#[test]
fn test_init_no_vcs_prints_note() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("no VCS detected"),
        "stdout should explain no-VCS skip: {stdout}"
    );
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --test cli_integration test_init_hg test_init_no_vcs -- --nocapture`
Expected: PASS — Task 9's implementation already handles these branches.

(If these tests fail, audit `step_vcs_merge_config` against Task 9 — they should not.)

- [ ] **Step 3: Commit**

```bash
git add tests/cli_integration.rs
git commit -m "test(init): cover hg and no-VCS skip paths"
```

---

### Task 11: Agent file discovery + `step_agent_directive` for existing files

Walk the fixed discovery table; for each existing agent file, append the directive if not already present. Skip silently if already present. This task does **not** cover the fallback "create AGENTS.md" case (Task 12) or `.cursor/rules` directory walk (Task 13).

**Files:**
- Modify: `src/cli/commands/init.rs`
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing integration tests**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_patches_existing_agents_md() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("AGENTS.md"),
        "# AGENTS.md\n\nBe nice.\n",
    )
    .unwrap();

    let (_, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);

    let content = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(
        content.contains("qualifier agents"),
        "AGENTS.md should contain the directive: {content:?}"
    );
    assert!(
        content.starts_with("# AGENTS.md\n\nBe nice.\n"),
        "prior content should be preserved"
    );
}

#[test]
fn test_init_patches_multiple_agent_files() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "# AGENTS\n").unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "# Claude\n").unwrap();

    let (_, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);

    for name in ["AGENTS.md", "CLAUDE.md"] {
        let content = std::fs::read_to_string(dir.path().join(name)).unwrap();
        assert!(
            content.contains("qualifier agents"),
            "{name} should contain directive: {content:?}"
        );
        // Exactly one occurrence — re-runs must not duplicate.
        assert_eq!(
            content.matches("qualifier agents").count(),
            1,
            "{name} should have directive exactly once"
        );
    }
}

#[test]
fn test_init_skips_agent_file_with_directive() {
    let dir = tempfile::tempdir().unwrap();
    let original = "# AGENTS.md\n\nSee qualifier agents.\n";
    std::fs::write(dir.path().join("AGENTS.md"), original).unwrap();

    let (_, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);

    let content = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert_eq!(content, original, "AGENTS.md should be unchanged");
}

#[test]
fn test_init_patches_copilot_instructions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".github")).unwrap();
    std::fs::write(
        dir.path().join(".github/copilot-instructions.md"),
        "# Copilot\n",
    )
    .unwrap();

    let (_, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);

    let content =
        std::fs::read_to_string(dir.path().join(".github/copilot-instructions.md")).unwrap();
    assert!(content.contains("qualifier agents"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_integration test_init_patches test_init_skips_agent -- --nocapture`
Expected: FAIL — step_agent_directive doesn't exist yet.

- [ ] **Step 3: Implement discovery + the agent step**

In `src/cli/commands/init.rs`, add above `run()`:

```rust
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
/// name. (The `.cursor/rules` walk is implemented in a later task; for
/// now Dir entries return nothing.)
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
                // Implemented in Task 13.
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
        // Fallback (create AGENTS.md) implemented in Task 12.
        return Ok(vec![StepOutcome::Skipped(
            "no agent-instruction files found".into(),
        )]);
    }

    let mut outcomes = Vec::with_capacity(files.len());
    for path in files {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        let existing = fs::read_to_string(&path)?;
        if already_has_directive(&existing) {
            outcomes.push(StepOutcome::Skipped(format!(
                "{rel}: already references `qualifier agents`"
            )));
            continue;
        }
        describe_to(out, &rel, &[DIRECTIVE])?;
        if confirm_to(out, "Apply?", true, mode)? {
            let new_content = append_agent_directive(&existing);
            fs::write(&path, new_content)?;
            outcomes.push(StepOutcome::Applied(format!("{rel}: directive appended")));
        } else {
            outcomes.push(StepOutcome::Skipped(format!("{rel}: declined")));
        }
    }
    Ok(outcomes)
}
```

Update `run()` to call the new step after the VCS step:

```rust
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

    for o in &outcomes {
        match o {
            StepOutcome::Applied(msg) | StepOutcome::Skipped(msg) => {
                writeln!(stdout, "{msg}")?;
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test cli_integration test_init -- --nocapture`
Expected: PASS — every `test_init_*` test added so far.

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs tests/cli_integration.rs
git commit -m "feat(init): patch existing agent-instruction files"
```

---

### Task 12: Fallback — offer to create `AGENTS.md` when none exist

When discovery finds zero agent files, prompt to create `AGENTS.md`.

**Files:**
- Modify: `src/cli/commands/init.rs`
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing integration test**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_creates_agents_md_when_none_exist() {
    let dir = tempfile::tempdir().unwrap();
    let (_, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);

    let path = dir.path().join("AGENTS.md");
    assert!(path.exists(), "AGENTS.md should have been created");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("qualifier agents"));

    // Other supported files should NOT have been created.
    assert!(!dir.path().join("CLAUDE.md").exists());
    assert!(!dir.path().join("GEMINI.md").exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_integration test_init_creates_agents_md -- --nocapture`
Expected: FAIL — the fallback isn't implemented; `AGENTS.md` isn't created.

- [ ] **Step 3: Implement the fallback**

Replace the early-return branch in `step_agent_directive` in `src/cli/commands/init.rs`:

```rust
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
            fs::write(&path, append_agent_directive(""))?;
            return Ok(vec![StepOutcome::Applied("AGENTS.md: created".into())]);
        } else {
            return Ok(vec![StepOutcome::Skipped(
                "AGENTS.md: declined creation".into(),
            )]);
        }
    }

    let mut outcomes = Vec::with_capacity(files.len());
    for path in files {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_string_lossy().into_owned());
        let existing = fs::read_to_string(&path)?;
        if already_has_directive(&existing) {
            outcomes.push(StepOutcome::Skipped(format!(
                "{rel}: already references `qualifier agents`"
            )));
            continue;
        }
        describe_to(out, &rel, &[DIRECTIVE])?;
        if confirm_to(out, "Apply?", true, mode)? {
            let new_content = append_agent_directive(&existing);
            fs::write(&path, new_content)?;
            outcomes.push(StepOutcome::Applied(format!("{rel}: directive appended")));
        } else {
            outcomes.push(StepOutcome::Skipped(format!("{rel}: declined")));
        }
    }
    Ok(outcomes)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli_integration test_init_creates_agents_md -- --nocapture`
Expected: PASS.

Run: `cargo test --all-features`
Expected: full suite passes.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs tests/cli_integration.rs
git commit -m "feat(init): offer to create AGENTS.md when none exist"
```

---

### Task 13: `.cursor/rules` directory walk (one level deep)

Discover `*.md` and `*.mdc` children of `.cursor/rules/` (no recursion) and patch each.

**Files:**
- Modify: `src/cli/commands/init.rs`
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing integration test**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_walks_cursor_rules_one_level() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".cursor/rules")).unwrap();
    std::fs::create_dir_all(dir.path().join(".cursor/rules/nested")).unwrap();
    std::fs::write(dir.path().join(".cursor/rules/main.md"), "# main\n").unwrap();
    std::fs::write(dir.path().join(".cursor/rules/style.mdc"), "# style\n").unwrap();
    std::fs::write(dir.path().join(".cursor/rules/README.txt"), "# unrelated\n").unwrap();
    std::fs::write(
        dir.path().join(".cursor/rules/nested/deep.md"),
        "# deep\n",
    )
    .unwrap();

    let (_, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);

    // Patched: .md and .mdc at the top level.
    let main = std::fs::read_to_string(dir.path().join(".cursor/rules/main.md")).unwrap();
    let style = std::fs::read_to_string(dir.path().join(".cursor/rules/style.mdc")).unwrap();
    assert!(main.contains("qualifier agents"));
    assert!(style.contains("qualifier agents"));

    // Not patched: unrelated extension and nested file.
    let readme = std::fs::read_to_string(dir.path().join(".cursor/rules/README.txt")).unwrap();
    let deep = std::fs::read_to_string(dir.path().join(".cursor/rules/nested/deep.md")).unwrap();
    assert!(!readme.contains("qualifier agents"), "README.txt must not be touched");
    assert!(!deep.contains("qualifier agents"), "nested file must not be touched");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_integration test_init_walks_cursor_rules -- --nocapture`
Expected: FAIL — `discover_agent_files` ignores Dir entries today.

- [ ] **Step 3: Implement the directory walk**

Replace `discover_agent_files` in `src/cli/commands/init.rs`:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test cli_integration test_init_walks_cursor_rules -- --nocapture`
Expected: PASS.

Run: `cargo test --all-features`
Expected: full suite passes.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs tests/cli_integration.rs
git commit -m "feat(init): walk .cursor/rules one level deep"
```

---

### Task 14: Final summary line + applied/skipped counts

Print a one-line summary at the end of `run()`: `init: applied N step(s), skipped M.`

**Files:**
- Modify: `src/cli/commands/init.rs`
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_summary_counts_fresh_git() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);
    // 1 gitattributes step + 1 fallback AGENTS.md creation = 2 applied.
    assert!(
        stdout.contains("applied 2") && stdout.contains("skipped 0"),
        "summary should report 2 applied / 0 skipped: {stdout}"
    );
}

#[test]
fn test_init_summary_counts_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();
    std::fs::write(dir.path().join(".gitattributes"), "*.qual merge=union\n").unwrap();
    std::fs::write(
        dir.path().join("AGENTS.md"),
        "# AGENTS\n\nSee qualifier agents.\n",
    )
    .unwrap();

    let (stdout, _, code) = run_qualifier(dir.path(), &["init", "--yes"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("applied 0") && stdout.contains("skipped 2"),
        "summary should report 0 applied / 2 skipped: {stdout}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_integration test_init_summary -- --nocapture`
Expected: FAIL — no summary line emitted.

- [ ] **Step 3: Implement the summary**

Replace `run()` in `src/cli/commands/init.rs`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test cli_integration test_init_summary -- --nocapture`
Expected: PASS — both summary tests.

Run: `cargo test --all-features`
Expected: full suite passes.

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/init.rs tests/cli_integration.rs
git commit -m "feat(init): print applied/skipped summary"
```

---

### Task 15: `--dry-run` writes nothing

Verify dry-run doesn't touch the filesystem and prints `[dry-run]` markers.

**Files:**
- Test: `tests/cli_integration.rs` (append)

- [ ] **Step 1: Write the failing test**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_init_dry_run_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".git")).unwrap();

    let before_listing: std::collections::BTreeSet<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();

    let (stdout, _, code) = run_qualifier(dir.path(), &["init", "--dry-run"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("[dry-run]"), "stdout should mark dry-run lines: {stdout}");

    let after_listing: std::collections::BTreeSet<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before_listing, after_listing, "dry-run must not create files");

    assert!(
        !dir.path().join(".gitattributes").exists(),
        "dry-run must not create .gitattributes"
    );
    assert!(
        !dir.path().join("AGENTS.md").exists(),
        "dry-run must not create AGENTS.md"
    );
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test --test cli_integration test_init_dry_run -- --nocapture`
Expected: PASS — `Mode::DryRun` causes `confirm_to` to return `false`, so no writes happen and the `[dry-run]` marker is printed.

(If this test fails, audit `confirm_to` against Task 7.)

- [ ] **Step 3: Commit**

```bash
git add tests/cli_integration.rs
git commit -m "test(init): verify --dry-run writes nothing"
```

---

### Task 16: Documentation + version bump

Update spec, README, changelog, and the crate version.

**Files:**
- Modify: `SPEC.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock` (regenerated)

- [ ] **Step 1: Remove `qualifier init` from SPEC §12 future considerations**

Edit `SPEC.md`. Find the bullet starting `- **Project bootstrap (\`qualifier init\`):**` (around line 1415) and delete the whole bullet (typically 3 lines).

- [ ] **Step 2: Mention init in SPEC §8.2**

Edit `SPEC.md` §8.2 (around line 1273). After the table, add a single line:

```markdown
Run `qualifier init` to apply the git configuration interactively.
```

- [ ] **Step 3: Add init to README CLI listing**

Edit `README.md`. Locate the CLI commands section (search for `qualifier record` or the commands table). Add an entry for `qualifier init` that matches the existing entry style. Example phrasing if the section is a table:

```markdown
| `qualifier init`    | Bootstrap a project: VCS merge config and agent directives |
```

If the section is a bullet list, mirror that format instead. Place `init` near the top, before recording commands, to match the help-template grouping.

- [ ] **Step 4: Add changelog entry**

Edit `CHANGELOG.md`. Add a new section at the top:

```markdown
## [0.7.0] — unreleased

### Added

- **`qualifier init`** — interactive, idempotent bootstrap command.
  Configures `*.qual merge=union` in `.gitattributes` for git repos
  (hints for hg/jj/pijul/fossil/svn), and appends a one-line directive
  pointing AI coding agents at `qualifier agents` to any discovered
  agent-instruction file (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`,
  `.cursorrules`, `.windsurfrules`, `.github/copilot-instructions.md`,
  or `*.md`/`*.mdc` files directly under `.cursor/rules/`). If no agent
  file exists, offers to create `AGENTS.md`. Supports `--yes` for
  non-interactive use and `--dry-run` to preview changes. Already-
  configured steps are silently skipped.
```

- [ ] **Step 5: Bump crate version**

Edit `Cargo.toml`, change:

```toml
version = "0.6.1"
```

to:

```toml
version = "0.7.0"
```

- [ ] **Step 6: Rebuild Cargo.lock and verify everything compiles**

Run: `cargo build --all-features`
Expected: rebuilds, `Cargo.lock` updates the `qualifier` entry to 0.7.0.

Run: `cargo test --all-features`
Expected: full suite passes.

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings.

Run: `cargo fmt --check`
Expected: no diff. (If it complains, run `cargo fmt` and re-stage.)

- [ ] **Step 7: Commit**

```bash
git add SPEC.md README.md CHANGELOG.md Cargo.toml Cargo.lock
git commit -m "chore: bump to 0.7.0 and document \`qualifier init\`"
```

---

## Self-Review

**Spec coverage check** (against `docs/superpowers/specs/2026-05-13-qualifier-init-design.md`):

- §"Step 1 — VCS merge config" (git path) — Task 9.
- §"Step 1 — VCS merge config" (hg/jj/pijul/fossil/svn hints) — Task 10 (impl from Task 9's `step_vcs_merge_config` already covers it; Task 10 adds the test).
- §"Step 1 — VCS merge config" (no VCS) — Task 10.
- §"Step 2 — Agent-instruction directive" (discovery + patch) — Task 11.
- §"Step 2 — Agent-instruction directive" (fallback to create AGENTS.md) — Task 12.
- §"Step 2 — Agent-instruction directive" (`.cursor/rules` walk one level) — Task 13.
- §"Prompt format" — Task 7 (`describe` + `confirm` helpers).
- §"Flags" (`--yes`, `--dry-run`, mutually exclusive) — Task 1 (clap defs), Task 2 (conflict test), Task 15 (dry-run behavior test).
- §"TTY guard" — Task 8.
- §"Exit and summary" — Task 14.
- §"Wasm gate" — Task 1 (`#[cfg(target_os = "emscripten")]` stub).
- §"Path safety" — Tasks 9/11/13 (paths derived from `find_project_root`; bounded `.cursor/rules` walk).
- §"Documentation" — Task 16.

**Placeholder scan:** None. Every code step shows the full code that should land. The phrase "implemented in a later task" appears in Task 11 inside an explanatory comment in `discover_agent_files`, then is removed in Task 13 when the walk is implemented — this is incremental, not a placeholder.

**Type consistency:** `Mode`, `Args`, `StepOutcome`, `AgentKind` are introduced once and referred to consistently. `describe_to` / `confirm_to` keep their signatures across tasks. `step_vcs_merge_config` returns `StepOutcome`; `step_agent_directive` returns `Vec<StepOutcome>` (sub-steps); `run()` flattens them — consistent across Tasks 9, 11, 12, 14.

**Test-name consistency:** All integration tests are prefixed `test_init_*` so `cargo test --test cli_integration test_init` filters cleanly. Unit tests live under `init::tests`.
