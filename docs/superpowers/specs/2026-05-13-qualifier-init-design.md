# `qualifier init` — design

Date: 2026-05-13
Status: design approved in conversation; pending user review of this document

## Motivation

Adopting qualifier in a new project requires two small but easy-to-forget
configuration steps:

1. Telling the VCS to union-merge `*.qual` files so concurrent appends don't
   collide.
2. Telling AI coding agents working in the repo that `qualifier agents`
   exists and is the entry point they should consult before recording
   annotations.

Both are documented in `SPEC.md` (§8.2 and §9 respectively) but neither is
automated today. A previous `qualifier init` existed and was yanked in
commit `0a376e6` because its main job (scaffolding `qualifier.graph.jsonl`)
went away with the graph engine. SPEC §12 lists init as a future
consideration "reintroduced when there's a clear win." Agent integration is
that clear win.

## Scope

`qualifier init` is an interactive, idempotent bootstrap command. It walks
through a small fixed list of steps; each step inspects the repository,
proposes a concrete change, and asks the user before applying it.

Out of scope:

- Creating a starter `.qual` file (the format is append-only and
  content-addressed; no placeholder is needed).
- Generating a `.qualifier.toml` config file (config loading already
  tolerates the absence of one).
- Scaffolding `.qualignore` (most projects don't need one; explicit creation
  is a single `touch` away when they do).
- Editing CI configuration, pre-commit hooks, or anything outside the
  project root.

## User-visible behavior

### Steps (in order)

**Step 1 — VCS merge config.**

- If `.git` is detected at the project root: read `.gitattributes` (treat
  missing as empty), check for `*.qual merge=union`, and if absent propose
  appending it. If present, skip silently.
- If `.hg`, `.jj`, `.pijul`, `_FOSSIL_`, or `.svn` is detected: print a
  one-line setup hint referring the user to SPEC §8.2. No prompt, no edit.
- If no VCS marker is found: print one line, `no VCS detected — skipping
  merge configuration`.

**Step 2 — Agent-instruction directive.**

Discovery table (fixed):

| Path                                    | Kind |
|-----------------------------------------|------|
| `AGENTS.md`                             | file |
| `CLAUDE.md`                             | file |
| `GEMINI.md`                             | file |
| `.cursorrules`                          | file |
| `.windsurfrules`                        | file |
| `.github/copilot-instructions.md`       | file |
| `.cursor/rules/` (one level deep, `*.md` and `*.mdc`) | dir |

For each existing file:

1. Read it and check (case-insensitive substring) for `qualifier agents`. If
   found, skip silently.
2. Otherwise propose appending one line at the end:

   ```
   If qualifier might be relevant, run `qualifier agents` first.
   ```

   If the file is non-empty and doesn't end with `\n`, write `\n` first.
   Then write a blank line and the directive line, each terminated by `\n`.
   If the file is empty, write only the directive line plus a single `\n`.

If discovery produces **zero** existing files, prompt once: "Create
`AGENTS.md` with the qualifier directive? [Y/n]" — default yes. On accept,
write a new `AGENTS.md` containing only the directive plus a trailing
newline.

### Prompt format

Per-step, the command prints a small describe-then-confirm block:

```
.gitattributes
  + *.qual merge=union
Apply? [Y/n]
```

- Default yes (uppercase `Y`); a bare Enter accepts.
- The describe block names the target path and lists the lines that would
  be added with a `+` prefix.
- Steps that detect already-applied state print nothing and silently skip.

### Flags

- `--yes` / `-y`: auto-accept every prompt at its default. The transcript
  still shows each describe block followed by `[applying]` so the run is
  auditable.
- `--dry-run`: report each step's intent without writing anything and
  without prompting. The describe block is followed by `[dry-run]`.
- `--yes` and `--dry-run` are mutually exclusive (clap `conflicts_with`).

### TTY guard

If interactive mode is in effect (neither `--yes` nor `--dry-run`) and
stdin is not a TTY, the command exits non-zero with:

```
init: refusing to prompt without a TTY; pass --yes or --dry-run
```

Detection: `std::io::IsTerminal::is_terminal(&std::io::stdin())`.

### Exit and summary

- Final line on success: `init: applied N step(s), skipped M.` where N and
  M count individual sub-steps (each agent file is its own sub-step).
- I/O errors return `crate::Error::Io` and the CLI runner exits 1.
- Clap validation errors (e.g., `--yes --dry-run`) exit 2 per clap's
  default.

## Architecture

### File layout

- New: `src/cli/commands/init.rs`.
- Modified: `src/cli/commands/mod.rs` (re-export), `src/cli/mod.rs`
  (`Commands::Init(init::Args)` variant, `HELP_TEMPLATE` entry, dispatch
  arm).

`HELP_TEMPLATE` gets a new section above `Record observations:`:

```
Initialize:
  init       Bootstrap a project: VCS merge config and agent directives
```

### Types

```rust
#[derive(clap::Parser, Debug)]
pub struct Args {
    /// Auto-accept every step at its default.
    #[arg(short, long)]
    pub yes: bool,

    /// Report what would change without writing or prompting.
    #[arg(long, conflicts_with = "yes")]
    pub dry_run: bool,
}

enum Mode { Interactive, Yes, DryRun }

enum StepOutcome {
    Applied(String),  // human-readable summary line
    Skipped(String),  // already-configured / declined / nothing to do
}
```

`run(args: Args) -> crate::Result<()>` resolves the project root via
`qual_file::find_project_root` (falling back to CWD), computes `Mode`,
calls each step in order, and prints the summary.

### `confirm` helper

```rust
fn confirm(prompt: &str, default_yes: bool, mode: Mode) -> io::Result<bool>
```

- `Mode::Interactive`: prints `prompt`, reads a line from stdin, returns
  `default_yes` on empty input, `true` on `y`/`yes`, `false` on `n`/`no`,
  re-prompts on anything else.
- `Mode::Yes`: prints `prompt` then `[applying]` (or `[skipping]` if
  `default_yes` is false), returns `default_yes`.
- `Mode::DryRun`: prints `prompt` then `[dry-run]`, returns `false`.

A `describe(path, additions: &[&str])` helper prints the path + `+` lines
before the prompt.

### Step 1 — `step_vcs_merge_config`

Reuses `qual_file::detect_vcs` (already exists). Branches:

- `Some("git")`: read `.gitattributes` (NotFound → empty); if
  `has_gitattributes_rule(&content)` returns true, emit
  `Skipped(".gitattributes: already configured")` with no prompt;
  otherwise build new content (`existing` + optional `\n` separator +
  `"*.qual merge=union\n"`), describe, prompt, write.
- `Some("hg" | "jj" | "pijul" | "fossil" | "svn")`: emit `Skipped` with a
  VCS-appropriate hint message; no prompt.
- `None`: emit `Skipped("no VCS detected — skipping merge configuration")`.

`has_gitattributes_rule` is line-anchored and tolerant of leading
whitespace and trailing whitespace/comments. It accepts any line whose
non-comment, non-whitespace tokens are exactly `*.qual` and `merge=union`
(in that order). Exact-pattern guarantees we don't double-insert.

### Step 2 — `step_agent_directive`

```rust
const AGENT_FILES: &[(&str, AgentKind)] = &[
    ("AGENTS.md", AgentKind::File),
    ("CLAUDE.md", AgentKind::File),
    ("GEMINI.md", AgentKind::File),
    (".cursorrules", AgentKind::File),
    (".windsurfrules", AgentKind::File),
    (".github/copilot-instructions.md", AgentKind::File),
    (".cursor/rules", AgentKind::Dir),
];
```

For each entry: resolve relative to project root. `File` entries collect
the path if it exists. `Dir` entries, if the directory exists, do a single
non-recursive read and collect `*.md` and `*.mdc` children sorted by name.

For each collected file path:

1. Read content. If `already_has_directive(&content)` returns true, emit
   `Skipped("<path>: already configured")`.
2. Otherwise compute the appended content, describe (`<path>` + `+`
   directive line), prompt (`[Y/n]`, default yes), write on accept.

`already_has_directive`: case-insensitive search for the literal substring
`"qualifier agents"`. (Loose enough to catch reorderings, tight enough that
documentation referring to the agents subcommand counts as already
configured — which is the right call: don't insert when the concept is
already documented.)

If the collected list is empty, prompt once for creating `AGENTS.md`
(default yes). On accept, write the directive line + `\n`.

### Wasm gate

```rust
#[cfg(target_os = "emscripten")]
pub fn run(_args: Args) -> crate::Result<()> {
    Err(crate::Error::Validation(
        "init is not available in the browser".into(),
    ))
}
```

Mirrors the wasm gate the old `init.rs` used.

### Path safety

- All paths are resolved against the discovered project root.
- The discovery table is fixed — no user-supplied globbing, no symlink
  following beyond what the OS does for `read_to_string`.
- `.cursor/rules` is the only directory walked, and it's walked one level
  deep with no recursion.

## Testing

### Unit tests in `src/cli/commands/init.rs`

Pure string-in/string-out functions:

- `gitattributes_already_present` — exact line; leading whitespace;
  trailing comment; present among other rules; present without trailing
  newline. All match.
- `gitattributes_append_format` — input ending in `\n` and input not
  ending in `\n` both produce a result ending in `*.qual merge=union\n`
  with the correct separator.
- `already_has_directive_cases` — `"qualifier agents"` matched; mixed
  case matched; substring inside a code fence matched; `qualifier-agents`
  (no space) not matched.
- `append_directive_to_empty_file` and
  `append_directive_to_non_newline_terminated_file`.

### CLI integration tests in `tests/cli_integration.rs`

Each test sets up a temp dir, optionally seeds it (`.git`, files), and
invokes the built `qualifier` binary.

1. `init_git_fresh_yes` — git repo, no `.gitattributes`, no agent files.
   `qualifier init --yes`. Assert: `.gitattributes` contains the rule;
   `AGENTS.md` exists with the directive; summary reports 2 applied.
2. `init_git_idempotent` — pre-create `.gitattributes` with the rule and
   `AGENTS.md` with the directive. `--yes`. Assert: files byte-for-byte
   unchanged; summary reports 0 applied / 2 skipped.
3. `init_dry_run_writes_nothing` — fresh repo. `--dry-run`. Assert: no
   files created or modified; stdout shows `[dry-run]`; exit 0.
4. `init_yes_and_dry_run_conflict` — clap rejects with exit 2.
5. `init_non_git_hg` — create `.hg/` marker. `--yes`. Assert:
   `.gitattributes` not created; stdout contains hg hint; agent step runs.
6. `init_no_vcs` — bare dir. `--yes`. Assert: `no VCS detected` line;
   agent step runs and creates `AGENTS.md`.
7. `init_patches_multiple_agent_files` — pre-create `AGENTS.md` and
   `CLAUDE.md` (no directive). `--yes`. Assert: both files have the
   directive appended exactly once.
8. `init_creates_agents_md_when_none_exist` — fresh repo. `--yes`.
   Assert: `AGENTS.md` is created; no other agent file is.
9. `init_cursor_rules_directory` — create `.cursor/rules/main.md`.
   `--yes`. Assert: that file is patched; nested files (`.cursor/rules/sub/x.md`)
   are not.
10. `init_non_tty_without_flag` — invoke with piped stdin and no flags.
    Assert: non-zero exit; stderr contains the TTY-required message.

### Out of scope

- No pty-based test of the interactive prompt loop. The `confirm()` helper
  is exercised through `Mode::Yes` and `Mode::DryRun` paths, which cover
  all branching except the actual stdin read.

## Documentation

In the same commit set:

- `SPEC.md` §12: remove the `qualifier init` bullet from Future
  Considerations.
- `SPEC.md`: add a short §8.5 or update §8.2 noting that `qualifier init`
  is the convenience path for the manual setup steps already documented.
- `README.md`: add `qualifier init` to the CLI Commands listing.
- `src/cli/mod.rs` `HELP_TEMPLATE`: add the `Initialize:` section.
- `src/cli/commands/agents/pages/_overview.md` (and any pitfalls page): if
  it mentions setup, point at `qualifier init`.
- `Cargo.toml`: bump minor version (new user-visible command).
- `CHANGELOG.md`: entry under the new version.

## Open questions

None at design time. Behavior, flags, file layout, and test plan are all
settled.
