# AGENTS-CLI cross-linking + frontmatter migration plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land two related changes for qualifier 0.5.1 — (a) cross-link the new AGENTS-CLI 0.1 protocol from qualifier's README and orientation page, declaring qualifier as the reference implementation; (b) migrate the agents page registry from a hand-coded `&[Page]` const to a build-time-generated registry derived from TOML frontmatter on each `pages/*.md`.

**Architecture:** Phase A is two surgical doc edits. Phase B introduces a `build.rs` that walks `src/cli/commands/agents/pages/`, parses TOML frontmatter via `toml` + `serde`, and emits `$OUT_DIR/agents_pages.rs` containing `OVERVIEW: &str` and `PAGES: &[Page]`. `mod.rs` `include!()`s the generated file and drops the hand-coded const.

**Tech Stack:** Rust 2024, clap 4, new build dependencies `toml = "0.8"` and `serde = { version = "1", features = ["derive"] }`.

**Specs:**
- [AGENTS-CLI.md](../../../AGENTS-CLI.md)
- [docs/superpowers/specs/2026-05-06-frontmatter-migration-design.md](../specs/2026-05-06-frontmatter-migration-design.md)

---

## File Structure

**Created:**
- `build.rs` (crate root) — walks the pages directory at compile time, parses frontmatter, emits the registry.

**Modified:**
- `README.md` — link to AGENTS-CLI.md near the top.
- `src/cli/commands/agents/pages/_overview.md` — add minimal frontmatter; add a conformance footer.
- `src/cli/commands/agents/pages/{concepts,workflows,pitfalls,record,reply,resolve,emit,show,ls,praise,review,compact}.md` — add TOML frontmatter (12 files).
- `src/cli/commands/agents/mod.rs` — replace hand-coded `&[Page]` with `include!()` of the generated file; expand `Page` struct with `sees_also` and `since`.
- `Cargo.toml` — add `[build-dependencies]`; bump version to 0.5.1.
- `CHANGELOG.md` — add a 0.5.1 entry.
- `tests/cli_integration.rs` — one new integration test.

**Boundaries:** `build.rs` is the only code outside the agents module that's affected. The user-visible CLI behavior is unchanged. Existing tests pass without modification.

---

## Task 1: Cross-link AGENTS-CLI from README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Read the current README to find the right insertion point**

The CLI Commands section starts around line 46. The new link should sit *above* the CLI tables but *below* any existing intro/quickstart, so a reader sees "this tool implements AGENTS-CLI" before they scan the command tables.

If there's an existing project-level intro paragraph, place the link right after it. Otherwise place it as a new paragraph just above the `## CLI` heading (or whichever heading currently introduces the command tables).

- [ ] **Step 2: Add the link**

Insert this paragraph at the chosen location:

```markdown
> **For AI coding agents:** qualifier implements [AGENTS-CLI 0.1](AGENTS-CLI.md). Run `qualifier agents` for a self-contained guide.
```

The blockquote prefix (`> `) is intentional — it sets the tone as a sidebar / call-out rather than blending into the prose.

- [ ] **Step 3: Verify the link target exists**

Run: `ls AGENTS-CLI.md`
Expected: file exists at repo root (it does — committed in `a05d6f2`).

- [ ] **Step 4: Render-check**

Open `README.md` in any markdown previewer (or just re-read the diff). Confirm the new line reads cleanly in context and the link is a relative link (not `./AGENTS-CLI.md` or absolute).

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: link AGENTS-CLI from README"
```

---

## Task 2: Conformance footer on `_overview.md`

**Files:**
- Modify: `src/cli/commands/agents/pages/_overview.md`

- [ ] **Step 1: Read the current `_overview.md`**

Find the `## Reference` section near the bottom.

- [ ] **Step 2: Append a conformance line**

After the existing `## Reference` section content, before any final newline, add:

```markdown

## Protocol

qualifier implements [AGENTS-CLI 0.1](https://github.com/empathic/qualifier/blob/main/AGENTS-CLI.md). The protocol defines the `agents` subcommand contract that this page satisfies.
```

The link is absolute (`https://...`) because the orientation page is read by an agent in someone else's repo — relative paths won't resolve. Use the canonical GitHub URL on the main branch.

- [ ] **Step 3: Sanity-check render**

Run: `cargo run --bin qualifier -- agents | tail -10`
Expected: the new "## Protocol" section appears at the bottom of the orientation output.

- [ ] **Step 4: Run all tests to confirm nothing broke**

Run: `cargo test --all-features`
Expected: all pass. (No tests assert on the absence of "## Protocol", so this should be clean.)

- [ ] **Step 5: Commit**

```bash
git add src/cli/commands/agents/pages/_overview.md
git commit -m "docs(agents): declare AGENTS-CLI 0.1 conformance in orientation page"
```

---

## Task 3: Frontmatter migration — atomic refactor

This is the meat of the work. It lands as a single commit because the changes are interlocked: adding frontmatter to a page without a corresponding `build.rs` to strip it would break user-visible output, and shipping `build.rs` without frontmatter on the pages would cause it to fail.

The task is large but each step is bite-sized. Follow the sequence; do not commit between steps.

**Files:**
- Modify: `Cargo.toml`
- Create: `build.rs`
- Modify: `src/cli/commands/agents/pages/_overview.md`
- Modify: `src/cli/commands/agents/pages/{concepts,workflows,pitfalls,record,reply,resolve,emit,show,ls,praise,review,compact}.md` (12 files)
- Modify: `src/cli/commands/agents/mod.rs`
- Test: `tests/cli_integration.rs`

- [ ] **Step 1: Add the new contract test (TDD red)**

Append to `tests/cli_integration.rs`:

```rust
#[test]
fn test_agents_orientation_summaries_match_pages() {
    // Lock in the contract that the orientation page renders the topic
    // index from frontmatter summaries (rather than hard-coded ones in
    // mod.rs). Each topic's summary must appear in bare-agents output.
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _stderr, code) = run_qualifier(dir.path(), &["agents"]);
    assert_eq!(code, 0);
    for needle in [
        "Annotation model, kinds, supersession",       // concepts
        "Worked recipes for common tasks",             // workflows
        "Common mistakes agents make with qualifier",  // pitfalls
        "Record a new annotation",                     // record
    ] {
        assert!(
            stdout.contains(needle),
            "orientation should include summary '{needle}': {stdout}"
        );
    }
}
```

This test passes today (the substrings already appear because `render_overview()` substitutes them in). It is a regression guard for the migration: if the build.rs ever fails to wire summaries through, this catches it.

Run: `cargo test --test cli_integration test_agents_orientation_summaries_match_pages`
Expected: PASS already. (We're locking in the contract before touching the implementation.)

- [ ] **Step 2: Bump version and add build dependencies in `Cargo.toml`**

Find the `[package]` block and change:

```toml
version = "0.5.0"
```

to:

```toml
version = "0.5.1"
```

Then, **above** the existing `[features]` block (or wherever fits the file's structure), add a new `[build-dependencies]` section:

```toml
[build-dependencies]
toml = "0.8"
serde = { version = "1", features = ["derive"] }
```

Run: `cargo build`
Expected: succeeds (no `build.rs` exists yet, so deps are unused but resolved). Cargo may print a warning about unused build-dependencies — that's fine; it goes away in Step 3.

- [ ] **Step 3: Create `build.rs` at the crate root**

Create `build.rs` with the full content below. This is the complete file — copy verbatim, don't trim.

```rust
//! Build-time generator for the `qualifier agents` page registry.
//!
//! Walks `src/cli/commands/agents/pages/*.md`, parses TOML frontmatter
//! delimited by `+++` lines, and emits `$OUT_DIR/agents_pages.rs`
//! containing:
//!
//! - `pub const OVERVIEW: &str = "...";`
//! - `pub const PAGES: &[Page] = &[...];`
//!
//! `Page` is defined in `src/cli/commands/agents/mod.rs`. The generated
//! file is `include!`d there.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

const PAGES_DIR: &str = "src/cli/commands/agents/pages";

#[derive(Deserialize)]
struct PageMeta {
    name: String,
    summary: Option<String>,
    #[serde(default)]
    sees_also: Vec<String>,
    since: Option<String>,
}

fn main() {
    println!("cargo:rerun-if-changed={PAGES_DIR}");
    println!("cargo:rerun-if-changed=build.rs");

    let mut entries: Vec<_> = fs::read_dir(PAGES_DIR)
        .unwrap_or_else(|e| panic!("read {PAGES_DIR}: {e}"))
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    entries.sort_by_key(|e| e.path());

    let mut overview_body: Option<String> = None;
    let mut topic_entries: Vec<String> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for entry in entries {
        let path = entry.path();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("non-utf8 filename: {}", path.display()))
            .to_string();
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

        let after_open = raw.strip_prefix("+++\n").unwrap_or_else(|| {
            panic!(
                "{}: file must begin with '+++' frontmatter delimiter on its first line",
                path.display()
            )
        });
        let close_offset = after_open.find("\n+++\n").unwrap_or_else(|| {
            panic!(
                "{}: missing closing '+++' frontmatter delimiter",
                path.display()
            )
        });
        let frontmatter = &after_open[..close_offset];
        let body = &after_open[close_offset + "\n+++\n".len()..];

        let meta: PageMeta = toml::from_str(frontmatter)
            .unwrap_or_else(|e| panic!("{}: invalid TOML frontmatter: {e}", path.display()));

        if meta.name != stem {
            panic!(
                "{}: frontmatter name '{}' does not match filename stem '{}'",
                path.display(),
                meta.name,
                stem
            );
        }
        if !seen_names.insert(meta.name.clone()) {
            panic!("duplicate page name '{}'", meta.name);
        }

        if stem.starts_with('_') {
            if stem == "_overview" {
                overview_body = Some(body.to_string());
            } else {
                panic!(
                    "{}: unsupported internal page; only '_overview' is recognized",
                    path.display()
                );
            }
        } else {
            let summary = meta.summary.unwrap_or_else(|| {
                panic!(
                    "{}: topic page is missing required 'summary' field",
                    path.display()
                )
            });
            let sees_also_lit = meta
                .sees_also
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            let since_lit = match meta.since {
                Some(v) => format!("Some({v:?})"),
                None => "None".into(),
            };
            topic_entries.push(format!(
                "    Page {{ name: {:?}, summary: {:?}, sees_also: &[{sees_also_lit}], since: {since_lit}, body: {:?} }},",
                meta.name, summary, body,
            ));
        }
    }

    let overview = overview_body.unwrap_or_else(|| {
        panic!("{PAGES_DIR}/_overview.md is required but not found");
    });

    let generated = format!(
        "pub const OVERVIEW: &str = {overview:?};\n\npub const PAGES: &[Page] = &[\n{}\n];\n",
        topic_entries.join("\n"),
    );

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("agents_pages.rs");
    fs::write(&out_path, generated)
        .unwrap_or_else(|e| panic!("write {}: {e}", out_path.display(), ));
}
```

Run: `cargo build`
Expected: FAILS — every page lacks frontmatter, and `build.rs` will panic on the first file that doesn't start with `+++`. The error message will name the file. This is expected; we're about to fix it.

- [ ] **Step 4: Add frontmatter to `_overview.md`**

Open `src/cli/commands/agents/pages/_overview.md`. Prepend, as the very first lines of the file:

```
+++
name = "_overview"
+++

```

(The trailing blank line is part of the prepend so the existing body is offset cleanly.)

The frontmatter for `_overview.md` is intentionally minimal: it has no `summary` because it isn't in the topic registry, and no `since` because it's the orientation page that always exists.

- [ ] **Step 5: Add frontmatter to topical pages**

For each of `concepts.md`, `workflows.md`, `pitfalls.md`, prepend frontmatter using the existing summary strings from the current `mod.rs`. **The summary text MUST match `mod.rs::PAGES` exactly** so that orientation output is byte-identical after the migration.

`concepts.md`:

```
+++
name = "concepts"
summary = "Annotation model, kinds, supersession, IDs, .qual layout"
since = "0.5.0"
+++

```

`workflows.md`:

```
+++
name = "workflows"
summary = "Worked recipes for common tasks"
since = "0.5.0"
+++

```

`pitfalls.md`:

```
+++
name = "pitfalls"
summary = "Common mistakes agents make with qualifier"
since = "0.5.0"
+++

```

- [ ] **Step 6: Add frontmatter to per-subcommand pages**

For each of the 9 per-subcommand pages, prepend frontmatter with the matching summary. The exact summaries to use come from the current hand-coded `PAGES` array in `src/cli/commands/agents/mod.rs`. Read the file once and copy each summary verbatim.

`record.md`:

```
+++
name = "record"
summary = "Record a new annotation"
sees_also = ["reply", "resolve", "emit"]
since = "0.5.0"
+++

```

`reply.md`:

```
+++
name = "reply"
summary = "Reply to an existing record"
sees_also = ["resolve", "record"]
since = "0.5.0"
+++

```

`resolve.md`:

```
+++
name = "resolve"
summary = "Resolve (close) a record"
sees_also = ["reply", "record"]
since = "0.5.0"
+++

```

`emit.md`:

```
+++
name = "emit"
summary = "Emit a raw record of any type"
sees_also = ["record"]
since = "0.5.0"
+++

```

`show.md`:

```
+++
name = "show"
summary = "Show annotations for an artifact"
sees_also = ["ls", "praise", "review"]
since = "0.5.0"
+++

```

`ls.md`:

```
+++
name = "ls"
summary = "List artifacts by kind"
sees_also = ["show"]
since = "0.5.0"
+++

```

`praise.md`:

```
+++
name = "praise"
summary = "Show who annotated an artifact and why"
sees_also = ["show"]
since = "0.5.0"
+++

```

`review.md`:

```
+++
name = "review"
summary = "Check freshness of annotations against current code"
sees_also = ["show", "compact"]
since = "0.5.0"
+++

```

`compact.md`:

```
+++
name = "compact"
summary = "Compact a .qual file"
sees_also = ["review"]
since = "0.5.0"
+++

```

- [ ] **Step 7: Verify `build.rs` produces the registry**

Run: `cargo build 2>&1 | head -40`
Expected: clean build. No more panics. (If a panic happens, the error names the offending file; fix the frontmatter and re-run.)

You can sanity-check the generated file:

```bash
find target -name agents_pages.rs -path '*/build/*' | head -1 | xargs head -5
```

Expected: shows `pub const OVERVIEW: &str = "qualifier agents — guide for AI coding agents...` followed by the `PAGES` array.

- [ ] **Step 8: Update `mod.rs` to consume the generated registry**

Open `src/cli/commands/agents/mod.rs`. Make these specific changes:

**Replace the `Page` struct** (currently `struct Page { name, summary, body }`) with the expanded shape:

```rust
pub struct Page {
    pub name: &'static str,
    pub summary: &'static str,
    pub sees_also: &'static [&'static str],
    pub since: Option<&'static str>,
    pub body: &'static str,
}
```

(Note `pub` on the struct and fields — the generated code references them.)

**Delete the hand-coded `OVERVIEW` line and the entire `PAGES` const definition** (the const ends at the closing `];`).

**Insert in their place:**

```rust
include!(concat!(env!("OUT_DIR"), "/agents_pages.rs"));
```

The rest of the file (`Args`, `topic_names()`, `run()`, `render_overview()`, the `#[cfg(test)] mod tests` block) stays unchanged. They consume `OVERVIEW` and `PAGES` exactly as before.

- [ ] **Step 9: Build and run all tests**

Run: `cargo build && cargo test --all-features`
Expected: full suite green. The orientation output matches byte-for-byte what it produced before the migration.

If a test fails, the most likely cause is summary text drift — verify your frontmatter `summary` strings match the originals exactly.

- [ ] **Step 10: Manual smoke**

```bash
target/debug/qualifier agents | head -10
target/debug/qualifier agents record | head -10
target/debug/qualifier agents bogus 2>&1 ; echo "exit=$?"
```

Expected:
- `agents` shows the orientation including the topic list with all 12 summaries (no `+++` leakage anywhere).
- `agents record` shows the record page (no `+++` leakage).
- `agents bogus` shows `no such topic 'bogus'. Available: concepts, workflows, ...` and `exit=2`.

- [ ] **Step 11: fmt + clippy**

Run: `cargo fmt && cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 12: Commit**

```bash
git add Cargo.toml build.rs src/cli/commands/agents tests/cli_integration.rs
git commit -m "$(cat <<'EOF'
refactor(agents): generate page registry from TOML frontmatter

Replaces the hand-coded &[Page] in src/cli/commands/agents/mod.rs with
a build.rs-generated const derived from frontmatter on each
pages/*.md. Adds toml + serde as build dependencies; emits
$OUT_DIR/agents_pages.rs included by mod.rs. New schema:
name (required), summary (required for topic pages), sees_also
(optional, parsed but not yet rendered), since (optional). User-visible
CLI behavior is unchanged.
EOF
)"
```

---

## Task 4: Add a contract unit test for non-empty summaries

**Files:**
- Modify: `src/cli/commands/agents/mod.rs`

This is a small follow-up that adds belt-and-suspenders coverage on the new build pipeline. Locks in the invariant that every topic page has a non-empty summary.

- [ ] **Step 1: Append to the existing `#[cfg(test)] mod tests` block in `mod.rs`**

The block already contains `overview_contains_topics_sentinel`. Add a second test alongside it:

```rust
    #[test]
    fn all_pages_have_non_empty_summaries() {
        for page in PAGES {
            assert!(
                !page.summary.is_empty(),
                "page '{}' has empty summary",
                page.name
            );
        }
    }
```

- [ ] **Step 2: Run the test**

Run: `cargo test all_pages_have_non_empty_summaries`
Expected: PASS.

- [ ] **Step 3: Run full suite + clippy**

Run: `cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add src/cli/commands/agents/mod.rs
git commit -m "test(agents): assert every page has a non-empty summary"
```

---

## Task 5: CHANGELOG entry

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Open `CHANGELOG.md` and find the existing 0.5.0 section**

The file currently has `## [0.5.0] — unreleased` (or, if 0.5.0 has been published, a dated header). Either way, the new entry goes above it.

- [ ] **Step 2: Add a 0.5.1 section above 0.5.0**

```markdown
## [0.5.1] — unreleased

### Added

- **AGENTS-CLI 0.1 protocol document** (`AGENTS-CLI.md`) — a draft
  cross-tool convention for CLI tools that self-describe to AI coding
  agents. qualifier is named as the reference implementation. The
  protocol defines five MUST rules (`agents` subcommand, bare
  orientation, topic dispatch, exit-2 unknown-topic, `--help`
  discoverability) and four SHOULD recommendations.

### Changed

- **Internal:** the `qualifier agents` page registry is now generated
  at build time from TOML frontmatter on each `pages/*.md` file
  instead of being a hand-coded `&[Page]` array in `mod.rs`. New
  per-page fields: `name`, `summary`, `sees_also`, `since`.
  User-visible CLI behavior is unchanged.
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs: changelog for 0.5.1 (AGENTS-CLI + frontmatter)"
```

---

## Task 6: Final verification

**Files:** none new

- [ ] **Step 1: fmt**

Run: `cargo fmt`
Expected: no diff.

- [ ] **Step 2: clippy**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean.

- [ ] **Step 3: full test suite**

Run: `cargo test --all-features`
Expected: all tests pass — including the existing 6 agents tests and the 2 new ones (`test_agents_orientation_summaries_match_pages` integration, `all_pages_have_non_empty_summaries` unit).

- [ ] **Step 4: end-to-end manual smoke**

```bash
target/debug/qualifier --help
target/debug/qualifier agents
target/debug/qualifier agents concepts
target/debug/qualifier agents record
target/debug/qualifier agents bogus 2>&1 ; echo "exit=$?"
```

Expected:
- `--help` shows the "For AI agents:" group at the top.
- `agents` shows the orientation page with the topic list rendered from frontmatter summaries, ending with the new "## Protocol" footer linking AGENTS-CLI.md.
- `agents concepts` and `agents record` show their pages, no `+++` leakage.
- `agents bogus` exits 2 with the available list.

- [ ] **Step 5: confirm `Cargo.toml` version is 0.5.1**

```bash
grep '^version' Cargo.toml
```

Expected: `version = "0.5.1"`.

- [ ] **Step 6: confirm AGENTS-CLI link from README is live**

```bash
grep -n "AGENTS-CLI" README.md
```

Expected: at least one match referencing `AGENTS-CLI.md`.

- [ ] **Step 7: if anything failed, fix and re-run from Step 1**

- [ ] **Step 8: optional final fmt/clippy commit**

```bash
git status
# If only fmt/clippy fixups remain:
git add -u
git commit -m "chore: final fmt/clippy fixups for 0.5.1"
```

If the working tree is clean, skip.
