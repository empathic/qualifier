# `qualifier agents` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `qualifier agents` subcommand that prints a self-contained guide so an AI coding agent can bootstrap into productive use of qualifier without a separately-installed skill.

**Architecture:** New `src/cli/commands/agents/` module with a registry of pages (each loaded via `include_str!()` from `pages/*.md`). Single positional arg: bare → orientation page with `{{TOPICS}}` index substitution; named → that page's body verbatim; unknown → exit 2. New "For AI agents:" group at the top of the existing `HELP_TEMPLATE`.

**Tech Stack:** Rust 2024, clap 4, existing CLI integration test harness (`tests/cli_integration.rs`) using `std::process::Command`.

**Spec:** [docs/superpowers/specs/2026-05-06-qualifier-agents-command-design.md](../specs/2026-05-06-qualifier-agents-command-design.md)

---

## File Structure

**Created:**
- `src/cli/commands/agents/mod.rs` — clap `Args`, `run()`, registry, dispatch, overview rendering
- `src/cli/commands/agents/pages/_overview.md` — orientation (printed by bare `qualifier agents`)
- `src/cli/commands/agents/pages/concepts.md` — annotation model, kinds, supersession, IDs, layout
- `src/cli/commands/agents/pages/workflows.md` — five worked recipes
- `src/cli/commands/agents/pages/pitfalls.md` — common mistakes (stub-friendly)
- `src/cli/commands/agents/pages/{record,reply,resolve,emit,show,ls,praise,review,compact}.md` — per-subcommand pages (9 files)

**Modified:**
- `src/cli/commands/mod.rs` — add `pub mod agents;`
- `src/cli/mod.rs` — add `Agents` variant, dispatch arm, update `HELP_TEMPLATE`
- `tests/cli_integration.rs` — new tests (see Task 9)
- `Cargo.toml` — bump version (e.g., 0.4.0 → 0.5.0; pick whichever next-minor matches crates.io state)
- `CHANGELOG.md` — note new subcommand under Added
- `README.md` — add `agents` to the CLI Commands tables under a new "For AI agents" sub-table

**Boundaries:** the agents module is self-contained. Its only outward dependency is being wired into the `Commands` enum and `HELP_TEMPLATE` in `src/cli/mod.rs`. It does not touch `annotation.rs`, `qual_file.rs`, or any other domain code.

---

## Task 1: Scaffold the agents module

**Files:**
- Create: `src/cli/commands/agents/mod.rs`
- Create: `src/cli/commands/agents/pages/_overview.md` (placeholder)
- Modify: `src/cli/commands/mod.rs`
- Modify: `src/cli/mod.rs`
- Test: `tests/cli_integration.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/cli_integration.rs`:

```rust
// --- qualifier agents ---

#[test]
fn test_agents_bare_invocation_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, stderr, code) = run_qualifier(dir.path(), &["agents"]);
    assert_eq!(code, 0, "agents should succeed: stderr={stderr}");
    assert!(!stdout.is_empty(), "agents should print something");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_integration test_agents_bare_invocation_succeeds`
Expected: FAIL with clap error like "unrecognized subcommand 'agents'".

- [ ] **Step 3: Create the placeholder overview page**

Create `src/cli/commands/agents/pages/_overview.md` with one line so `include_str!` succeeds:

```markdown
qualifier agents — orientation page (placeholder)
```

- [ ] **Step 4: Create the agents module**

Create `src/cli/commands/agents/mod.rs`:

```rust
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Topic name (e.g. `record`, `concepts`, `workflows`). Omit to print the orientation page.
    pub topic: Option<String>,
}

const OVERVIEW: &str = include_str!("pages/_overview.md");

pub fn run(args: Args) -> crate::Result<()> {
    match args.topic.as_deref() {
        None => {
            print!("{OVERVIEW}");
            Ok(())
        }
        Some(name) => {
            eprintln!("qualifier agents: no such topic '{name}'. Available: (none yet)");
            std::process::exit(2);
        }
    }
}
```

- [ ] **Step 5: Register the module**

Modify `src/cli/commands/mod.rs`. Add the line in alphabetical order:

```rust
pub mod agents;
pub mod compact;
pub mod emit;
// ... existing entries
```

- [ ] **Step 6: Wire the subcommand into `Commands`**

Modify `src/cli/mod.rs`. Add the variant inside `enum Commands`:

```rust
    /// Self-contained guide for AI coding agents (start here)
    Agents(commands::agents::Args),
```

Place it as the first variant (above `Record`) so it sits logically with the new "For AI agents" group.

Then add the dispatch arm inside the `match cli.command` block in `pub fn run()`:

```rust
        Commands::Agents(args) => commands::agents::run(args),
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test --test cli_integration test_agents_bare_invocation_succeeds`
Expected: PASS.

- [ ] **Step 8: Run the full test suite to ensure nothing broke**

Run: `cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 9: Commit**

```bash
git add src/cli/commands/agents src/cli/commands/mod.rs src/cli/mod.rs tests/cli_integration.rs
git commit -m "feat(agents): scaffold qualifier agents subcommand"
```

---

## Task 2: Add the unknown-topic error path

**Files:**
- Test: `tests/cli_integration.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_agents_unknown_topic_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let (_stdout, stderr, code) = run_qualifier(dir.path(), &["agents", "bogus-topic"]);
    assert_eq!(code, 2, "unknown topic should exit 2: stderr={stderr}");
    assert!(
        stderr.contains("no such topic"),
        "stderr should explain: {stderr}"
    );
    assert!(
        stderr.contains("bogus-topic"),
        "stderr should name the bad topic: {stderr}"
    );
}
```

- [ ] **Step 2: Run test to verify it passes already**

Run: `cargo test --test cli_integration test_agents_unknown_topic_exits_2`
Expected: PASS (Task 1's `run()` already prints the error and exits 2).

If it fails, re-check the `eprintln!` wording in `agents/mod.rs` — it must contain the literal string `no such topic`.

- [ ] **Step 3: Commit**

```bash
git add tests/cli_integration.rs
git commit -m "test(agents): cover unknown-topic exit path"
```

---

## Task 3: Introduce the page registry and per-topic dispatch

**Files:**
- Modify: `src/cli/commands/agents/mod.rs`
- Create: `src/cli/commands/agents/pages/concepts.md` (placeholder)
- Create: `src/cli/commands/agents/pages/workflows.md` (placeholder)
- Create: `src/cli/commands/agents/pages/pitfalls.md` (placeholder)
- Create: `src/cli/commands/agents/pages/{record,reply,resolve,emit,show,ls,praise,review,compact}.md` (placeholders, 9 files)
- Test: `tests/cli_integration.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_agents_concepts_topic_prints_body() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, stderr, code) = run_qualifier(dir.path(), &["agents", "concepts"]);
    assert_eq!(code, 0, "agents concepts should succeed: stderr={stderr}");
    assert!(!stdout.is_empty(), "should print body");
}

#[test]
fn test_agents_all_registered_topics_render() {
    // Each topic in the registry must produce non-empty stdout with exit 0.
    // If you add a topic, add it here too.
    let topics = [
        "concepts", "workflows", "pitfalls",
        "record", "reply", "resolve", "emit",
        "show", "ls", "praise", "review", "compact",
    ];
    let dir = tempfile::tempdir().unwrap();
    for topic in topics {
        let (stdout, stderr, code) = run_qualifier(dir.path(), &["agents", topic]);
        assert_eq!(code, 0, "agents {topic} should succeed: stderr={stderr}");
        assert!(!stdout.is_empty(), "agents {topic} should print body");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_integration test_agents_concepts_topic_prints_body test_agents_all_registered_topics_render`
Expected: FAIL — both topics are unknown.

- [ ] **Step 3: Create placeholder markdown files**

Create each file with a single placeholder line so `include_str!` resolves and the smoke test passes. You can use the same one-liner for all of them:

```markdown
qualifier agents — <TOPIC> page (placeholder; replace in Task 7/8)
```

Replace `<TOPIC>` with the topic name. Files to create:

- `src/cli/commands/agents/pages/concepts.md`
- `src/cli/commands/agents/pages/workflows.md`
- `src/cli/commands/agents/pages/pitfalls.md`
- `src/cli/commands/agents/pages/record.md`
- `src/cli/commands/agents/pages/reply.md`
- `src/cli/commands/agents/pages/resolve.md`
- `src/cli/commands/agents/pages/emit.md`
- `src/cli/commands/agents/pages/show.md`
- `src/cli/commands/agents/pages/ls.md`
- `src/cli/commands/agents/pages/praise.md`
- `src/cli/commands/agents/pages/review.md`
- `src/cli/commands/agents/pages/compact.md`

- [ ] **Step 4: Replace `agents/mod.rs` with the registry-backed implementation**

Replace the contents of `src/cli/commands/agents/mod.rs` with:

```rust
use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    /// Topic name (e.g. `record`, `concepts`, `workflows`). Omit to print the orientation page.
    pub topic: Option<String>,
}

struct Page {
    name: &'static str,
    summary: &'static str,
    body: &'static str,
}

const OVERVIEW: &str = include_str!("pages/_overview.md");

const PAGES: &[Page] = &[
    Page {
        name: "concepts",
        summary: "Annotation model, kinds, supersession, IDs, .qual layout",
        body: include_str!("pages/concepts.md"),
    },
    Page {
        name: "workflows",
        summary: "Worked recipes for common tasks",
        body: include_str!("pages/workflows.md"),
    },
    Page {
        name: "pitfalls",
        summary: "Common mistakes agents make with qualifier",
        body: include_str!("pages/pitfalls.md"),
    },
    Page {
        name: "record",
        summary: "Record a new annotation",
        body: include_str!("pages/record.md"),
    },
    Page {
        name: "reply",
        summary: "Reply to an existing record",
        body: include_str!("pages/reply.md"),
    },
    Page {
        name: "resolve",
        summary: "Resolve (close) a record",
        body: include_str!("pages/resolve.md"),
    },
    Page {
        name: "emit",
        summary: "Emit a raw record of any type",
        body: include_str!("pages/emit.md"),
    },
    Page {
        name: "show",
        summary: "Show annotations for an artifact",
        body: include_str!("pages/show.md"),
    },
    Page {
        name: "ls",
        summary: "List artifacts by kind",
        body: include_str!("pages/ls.md"),
    },
    Page {
        name: "praise",
        summary: "Show who annotated an artifact and why",
        body: include_str!("pages/praise.md"),
    },
    Page {
        name: "review",
        summary: "Check freshness of annotations against current code",
        body: include_str!("pages/review.md"),
    },
    Page {
        name: "compact",
        summary: "Compact a .qual file",
        body: include_str!("pages/compact.md"),
    },
];

fn topic_names() -> String {
    PAGES
        .iter()
        .map(|p| p.name)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn run(args: Args) -> crate::Result<()> {
    match args.topic.as_deref() {
        None => {
            print!("{}", render_overview());
            Ok(())
        }
        Some(name) => {
            if let Some(page) = PAGES.iter().find(|p| p.name == name) {
                print!("{}", page.body);
                Ok(())
            } else {
                eprintln!(
                    "qualifier agents: no such topic '{name}'. Available: {}",
                    topic_names()
                );
                std::process::exit(2);
            }
        }
    }
}

fn render_overview() -> String {
    // Replace the {{TOPICS}} sentinel with a `name — summary` listing.
    let topics_block: String = PAGES
        .iter()
        .map(|p| format!("- `{}` — {}", p.name, p.summary))
        .collect::<Vec<_>>()
        .join("\n");
    OVERVIEW.replace("{{TOPICS}}", &topics_block)
}
```

- [ ] **Step 5: Run the per-topic tests to verify they pass**

Run: `cargo test --test cli_integration test_agents_concepts_topic_prints_body test_agents_all_registered_topics_render`
Expected: PASS.

- [ ] **Step 6: Re-run all earlier agents tests**

Run: `cargo test --test cli_integration test_agents`
Expected: all four `test_agents_*` tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/cli/commands/agents tests/cli_integration.rs
git commit -m "feat(agents): registry and per-topic dispatch with placeholder pages"
```

---

## Task 4: Implement `{{TOPICS}}` substitution in the overview

**Files:**
- Modify: `src/cli/commands/agents/pages/_overview.md`
- Test: `tests/cli_integration.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_agents_overview_renders_topics_index() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _stderr, code) = run_qualifier(dir.path(), &["agents"]);
    assert_eq!(code, 0);
    // Every registered topic name should appear in the rendered overview.
    for topic in [
        "concepts", "workflows", "pitfalls",
        "record", "reply", "resolve", "emit",
        "show", "ls", "praise", "review", "compact",
    ] {
        assert!(
            stdout.contains(topic),
            "overview should mention topic '{topic}': {stdout}"
        );
    }
    // The literal sentinel must not leak through.
    assert!(
        !stdout.contains("{{TOPICS}}"),
        "sentinel should be substituted: {stdout}"
    );
}
```

Also add a unit test inside `src/cli/commands/agents/mod.rs` (append to the file):

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn overview_contains_topics_sentinel() {
        // If this fails, the orientation page lost the {{TOPICS}} substitution
        // anchor and the rendered overview will no longer list children.
        assert!(super::OVERVIEW.contains("{{TOPICS}}"));
    }
}
```

- [ ] **Step 2: Run the integration test to verify it fails**

Run: `cargo test --test cli_integration test_agents_overview_renders_topics_index`
Expected: FAIL — the placeholder `_overview.md` from Task 1 doesn't contain topic names or `{{TOPICS}}`.

- [ ] **Step 3: Replace `_overview.md` with a real (still concise) orientation that uses the sentinel**

Replace contents of `src/cli/commands/agents/pages/_overview.md` with the following. This is the launch version; deeper content is filled in during Task 7.

```markdown
# qualifier — guide for AI coding agents

You are an AI coding agent in a user's repository. The `qualifier` CLI is
installed. This page tells you what qualifier is, when to use it, and how to
get more detail on any specific feature.

## What it is

Qualifier records *annotations* — structured, content-addressed notes about
software artifacts (files, directories, line ranges). Annotations live in
`.qual` files alongside the code, are intended to be committed to version
control, and form a durable, reviewable record of quality observations
attached to specific places in the codebase.

## When to use it

- You found a bug, smell, risk, or stylistic concern that survives this
  edit and is worth surfacing for whoever touches the code next. Record it.
- You want to praise a piece of code so future readers know it was
  intentional, not accidental. Record it.
- An earlier annotation no longer applies (the code changed, the concern
  was addressed). Resolve it.

## When NOT to use it

- One-off scratch debugging notes. Use scratch space.
- Information that belongs in the commit message (what *this* change does
  and why). Use git.
- Project-wide policy or architecture decisions. Those belong in
  `docs/` or an ADR, not as an annotation on a file.

## Quickstart

```bash
# Record a concern about a function in src/foo.rs
qualifier record concern src/foo.rs --message "tight coupling to the cache"

# See what's annotated on a file
qualifier show src/foo.rs

# After fixing it, resolve the annotation by id-prefix or location
qualifier resolve src/foo.rs --message "refactored to inject the cache"
```

## Available topics

Run `qualifier agents <topic>` for any of:

{{TOPICS}}

## Reference

For exact flag tables on any subcommand, run `qualifier <subcommand> --help`.
For the JSONL wire format and library API, see `SPEC.md` in this repo (if
present) or the published spec.
```

- [ ] **Step 4: Run the failing tests to verify they now pass**

Run: `cargo test --test cli_integration test_agents_overview_renders_topics_index`
Expected: PASS.

- [ ] **Step 5: Run the unit test**

Run: `cargo test --lib agents::tests::overview_contains_topics_sentinel`
Expected: PASS.

If `cargo test --lib` doesn't find the test, the agents module may not be exposed under the library crate (it's CLI-only). In that case, the unit test will be discovered when running `cargo test --all-features` or `cargo test --bin qualifier`. Either is fine; the test just needs to run somewhere.

- [ ] **Step 6: Run the whole suite**

Run: `cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/cli/commands/agents tests/cli_integration.rs
git commit -m "feat(agents): render overview with topic index"
```

---

## Task 5: Add the "For AI agents" group to `--help`

**Files:**
- Modify: `src/cli/mod.rs`
- Test: `tests/cli_integration.rs`

- [ ] **Step 1: Write the failing test**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_top_level_help_shows_agents_group() {
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _stderr, code) = run_qualifier(dir.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("For AI agents:"),
        "help should show the agents group header: {stdout}"
    );
    // The agents row should appear under that header, with the "start here" nudge.
    assert!(
        stdout.contains("agents") && stdout.contains("start here"),
        "help should mention the agents subcommand: {stdout}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test cli_integration test_top_level_help_shows_agents_group`
Expected: FAIL — `HELP_TEMPLATE` doesn't yet include the new group.

- [ ] **Step 3: Update `HELP_TEMPLATE` in `src/cli/mod.rs`**

Locate the `const HELP_TEMPLATE: &str = "..."` block. Insert a new group **above** "Record observations:" so the agents row is the first thing under the usage line:

```rust
const HELP_TEMPLATE: &str = "\
{about-with-newline}
{usage-heading} {usage}

For AI agents:
  agents     Self-contained guide for AI coding agents (start here)

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

The only change is inserting the four lines `For AI agents:` … `agents     Self-contained guide for AI coding agents (start here)` plus the blank line that follows. Existing groups are untouched.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --test cli_integration test_top_level_help_shows_agents_group`
Expected: PASS.

- [ ] **Step 5: Run all tests**

Run: `cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs tests/cli_integration.rs
git commit -m "feat(agents): surface agents group at top of --help"
```

---

## Task 6: Format and lint check (mid-implementation gate)

**Files:** none new

This is a checkpoint before writing prose. The infrastructure is in place; lock it in by ensuring the toolchain is happy.

- [ ] **Step 1: Format**

Run: `cargo fmt`
Expected: no output (or whitespace-only diffs).

- [ ] **Step 2: Lint**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 3: If clippy or fmt produced changes, commit them**

```bash
git add -u
git commit -m "style: cargo fmt and clippy fixes for agents module"
```

If both produced no changes, skip the commit.

---

## Task 7: Write content for the topical pages

This task fills in the four topical pages with real prose. Each step is "write one file." Follow the spec's per-page templates exactly.

The spec's templates are reproduced inline below so you don't need to switch documents.

**Files:**
- Modify: `src/cli/commands/agents/pages/concepts.md`
- Modify: `src/cli/commands/agents/pages/workflows.md`
- Modify: `src/cli/commands/agents/pages/pitfalls.md`

(`_overview.md` was finalized in Task 4; no edit needed here.)

- [ ] **Step 1: Write `concepts.md` (~150 lines)**

Replace the placeholder. Cover, in this order, with section headings:

1. **The annotation envelope** — explain the metabox envelope (fields in fixed order: `metabox`, `type`, `subject`, `issuer`, `issuer_type`, `created_at`, `id`, `body`). Note: `metabox` is always `"1"`. Keep terse — agents need to recognize the shape, not memorize it.
2. **Kinds** — list the kinds (`note`, `concern`, `bug`, `praise`, plus any others present in `src/annotation.rs`'s `Kind` enum) and give one-sentence guidance per kind on when to choose it.
3. **Supersession** — explain that an annotation can supersede an earlier one (by id), chains must be acyclic, and cross-subject supersession is rejected.
4. **`.qual` file layout and discovery** — directory-level vs file-level `.qual` files, that discovery walks up looking for VCS markers, and that `.qualignore` and `.gitignore` filter what's considered.
5. **Issuer URIs and `issuer_type`** — issuer must be a URI (contain `:`); `mailto:user@example.com` is the typical agent-recordable form; `issuer_type` is one of `human`, `ai`, `tool`, `unknown`.
6. **Content addressing (BLAKE3)** — IDs are computed from a canonical form of the body; changing canonical fields changes the ID. Implication: don't hand-edit `.qual` files; use `record`/`emit`.

Keep each section to ~20 lines. Use code blocks for example JSON when it clarifies the shape. Source of truth for any specific behavior is `SPEC.md` and `src/annotation.rs`.

- [ ] **Step 2: Write `workflows.md` (~150 lines)**

Replace the placeholder. Five recipes, each a 1–2 sentence motivation followed by a concrete shell example:

1. **Record a finding during code review.** Motivation: agent reviewing a diff notices a concern. Example: `qualifier record concern src/payments/charge.rs --message "..." --span 42-58`.
2. **Reply to an existing observation with new info.** Motivation: another agent (or you, later) has more context to add to an existing annotation. Example: `qualifier reply <id-prefix> --message "..."`.
3. **Resolve a finding once it's addressed.** Motivation: the concern is fixed. Example: `qualifier resolve <id-prefix> --message "fixed in commit X"`.
4. **Triage stale annotations after a refactor.** Motivation: span-bound annotations may no longer point at the right code. Example: `qualifier review` and how to interpret the output.
5. **Compact a noisy `.qual` file.** Motivation: many superseded entries; collapse to a clean snapshot. Example: `qualifier compact <artifact>`.

For each recipe: 1 paragraph + 1 code block. ~25 lines per recipe.

- [ ] **Step 3: Write `pitfalls.md` (~60 lines)**

Replace the placeholder. Bulleted list of common agent mistakes, each with a one-sentence "why this is wrong" and a "do this instead":

- Recording without `--span` when the concern is about a few lines, not the whole file.
- Conflating `reply` (add info) with `resolve` (close).
- Recording an annotation when a commit message would do.
- Editing `.qual` files by hand instead of using `record`/`emit`.
- Choosing `bug` when `concern` fits — kinds matter for downstream filtering.
- Using a non-URI issuer (must contain `:`).

End with: `<!-- Add new pitfalls here as we observe them. -->`

- [ ] **Step 4: Sanity-check all three files render**

Run:

```bash
cargo run --bin qualifier -- agents concepts | head -5
cargo run --bin qualifier -- agents workflows | head -5
cargo run --bin qualifier -- agents pitfalls | head -5
```

Expected: each prints meaningful first lines (the section headings you wrote), not the placeholder.

- [ ] **Step 5: Run all tests**

Run: `cargo test --all-features`
Expected: all tests pass (the parameterized topic test still passes; existing tests unaffected).

- [ ] **Step 6: Commit**

```bash
git add src/cli/commands/agents/pages
git commit -m "docs(agents): write topical pages — concepts, workflows, pitfalls"
```

---

## Task 8: Write content for the per-subcommand pages

Each per-subcommand page follows the same template. The template is reproduced inline so you don't need to switch documents.

**Per-page template (same for all 9 files):**

```markdown
# qualifier <COMMAND>

## Purpose

<One sentence: what this command does.>

## When to use it

<2–3 sentences distinguishing this command from adjacent ones — e.g.,
record vs reply vs resolve, or show vs praise vs ls.>

## Common invocations

\`\`\`bash
# Realistic example 1
qualifier <COMMAND> ...

# Realistic example 2
qualifier <COMMAND> ...
\`\`\`

## Flags worth knowing

<Prose, not a flag table. Mention 2–4 flags an agent will actually use,
what each one is for, and any non-obvious interaction. For the full
flag list, the agent can run `qualifier <COMMAND> --help`.>

## Gotchas

- <Specific failure mode an agent might hit>
- <Another one>
```

**Length target:** 50–100 lines per file. If you're under 50 you've likely under-explained "When to use it"; if over 100 you've duplicated `--help`.

**Files:**
- Modify: `src/cli/commands/agents/pages/record.md`
- Modify: `src/cli/commands/agents/pages/reply.md`
- Modify: `src/cli/commands/agents/pages/resolve.md`
- Modify: `src/cli/commands/agents/pages/emit.md`
- Modify: `src/cli/commands/agents/pages/show.md`
- Modify: `src/cli/commands/agents/pages/ls.md`
- Modify: `src/cli/commands/agents/pages/praise.md`
- Modify: `src/cli/commands/agents/pages/review.md`
- Modify: `src/cli/commands/agents/pages/compact.md`

Source of truth for behavior of each command: read `src/cli/commands/<command>.rs` and run `qualifier <command> --help`. Do not invent flags.

- [ ] **Step 1: Write `record.md`**

Cover, beyond the template: that this is the unified annotation-write verb (replaced per-kind verbs), `--span`, `--stdin` for batch, kind argument, location argument.

- [ ] **Step 2: Write `reply.md`**

Cover: target can be id-prefix or location; reply *adds* to a thread (does not close it). Differentiate from `resolve`.

- [ ] **Step 3: Write `resolve.md`**

Cover: target can be id-prefix or location; this *closes* a record. Differentiate from `reply`.

- [ ] **Step 4: Write `emit.md`**

Cover: this is a low-level passthrough for any record type — agents should generally use `record`/`reply`/`resolve` instead, but `emit` exists for emitting raw `epoch` or `dependency` records or anything the higher-level commands don't cover. Mention `--body '<JSON>'`.

- [ ] **Step 5: Write `show.md`**

Cover: shows annotations for an artifact (threaded). Mention `--format json` for programmatic consumption.

- [ ] **Step 6: Write `ls.md`**

Cover: list artifacts by kind, optional `--kind` filter.

- [ ] **Step 7: Write `praise.md`**

Cover: shows who annotated an artifact and why; alias is `blame` (with a hint that we prefer `praise`). Mention `--vcs` if it surfaces VCS info.

- [ ] **Step 8: Write `review.md`**

Cover: checks freshness of span-bound annotations against current code; an agent should run this after editing files that have annotations on them.

- [ ] **Step 9: Write `compact.md`**

Cover: prunes superseded records and/or snapshots to an epoch record. Mention this is maintenance, not something to run constantly.

- [ ] **Step 10: Sanity-check a couple of pages render with real content**

Run:

```bash
cargo run --bin qualifier -- agents record | head -10
cargo run --bin qualifier -- agents emit | head -10
```

Expected: each prints the new prose, not the placeholder.

- [ ] **Step 11: Run all tests**

Run: `cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 12: Format and lint**

Run: `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 13: Commit**

```bash
git add src/cli/commands/agents/pages
git commit -m "docs(agents): write per-subcommand pages"
```

---

## Task 9: Update CHANGELOG, README, and bump version

**Files:**
- Modify: `Cargo.toml`
- Modify: `CHANGELOG.md`
- Modify: `README.md`

- [ ] **Step 1: Bump the crate version**

Edit `Cargo.toml`. Choose the next minor (e.g., `0.4.0` → `0.5.0`); confirm against published versions on crates.io if unsure. The change is purely additive (no breaking changes), so a minor bump is correct.

```toml
version = "0.5.0"
```

- [ ] **Step 2: Add a CHANGELOG entry**

In `CHANGELOG.md`, add a new section above the `[0.4.0]` section, or under `Added` of an existing unreleased section if one exists:

```markdown
## [0.5.0] — unreleased

### Added

- **`qualifier agents`** — self-contained guide for AI coding agents.
  Bare `qualifier agents` prints an orientation page with an index of
  topics; `qualifier agents <topic>` (e.g. `concepts`, `workflows`,
  `record`) drills into per-topic detail. The agent group also appears
  at the top of `qualifier --help` so models reading the help text
  discover the entry point on their own.
```

If a `[0.5.0] — unreleased` section already exists, add only the bullet under its `### Added` block.

- [ ] **Step 3: Update README CLI tables**

In `README.md`, find the existing "Record observations / Inspect / Maintain" CLI command tables (around line 51 onward). Add a new table **above** them:

```markdown
### For AI agents

| Command | Description |
|---------|-------------|
| `qualifier agents [topic]` | Self-contained guide for AI coding agents (start here) |
```

Keep the existing tables unchanged.

- [ ] **Step 4: Run all tests**

Run: `cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml CHANGELOG.md README.md
git commit -m "chore: bump to 0.5.0 and document agents subcommand"
```

(Adjust the version in the commit message if you chose a different number.)

---

## Task 10: Final verification

**Files:** none new

- [ ] **Step 1: Format**

Run: `cargo fmt`
Expected: no diff.

- [ ] **Step 2: Lint**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 3: Full test suite**

Run: `cargo test --all-features`
Expected: all tests pass.

- [ ] **Step 4: Manual smoke test**

Run each in turn:

```bash
cargo run --bin qualifier -- --help
cargo run --bin qualifier -- agents
cargo run --bin qualifier -- agents concepts
cargo run --bin qualifier -- agents record
cargo run --bin qualifier -- agents bogus 2>&1 ; echo "exit=$?"
```

Expected:
- `--help` shows "For AI agents:" group at the top with the `agents` row.
- `agents` prints the orientation page with a topic list (no literal `{{TOPICS}}`).
- `agents concepts` and `agents record` print real prose.
- `agents bogus` prints `qualifier agents: no such topic 'bogus'. Available: ...` to stderr and exits with `exit=2`.

- [ ] **Step 5: If anything failed, fix and re-run from Step 1**

- [ ] **Step 6: Final commit if there are leftover style fixups**

```bash
git status
# If anything is staged that wasn't covered by an earlier task's commit:
git add -u
git commit -m "chore: final fmt/clippy fixups for agents subcommand"
```

If the working tree is clean, skip this step.
