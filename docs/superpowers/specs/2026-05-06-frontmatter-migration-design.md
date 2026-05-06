# Frontmatter migration for `qualifier agents` pages

## Goal

Replace the hand-coded `&[Page]` registry in `src/cli/commands/agents/mod.rs` with a build-time-generated registry derived from TOML frontmatter on each `pages/*.md` file. Adding, renaming, or describing a topic becomes a single-file edit; the Rust source tree no longer carries a duplicated topic list to keep in sync.

## Non-goals

- Changing the runtime CLI surface. The user-visible behavior of `qualifier agents`, `qualifier agents <topic>`, and `qualifier agents <unknown>` is unchanged.
- Rendering `sees_also` cross-links in page output. The field is parsed and stored on `Page` but not yet rendered. A future PR can wire up a "See also:" footer.
- Standardizing this storage format across tools. AGENTS-CLI 0.1 explicitly leaves source-tree storage to each tool's choice; this is qualifier's choice.
- Runtime parsing. Pages are still read at compile time via `include_str!`-equivalent mechanics — never from the filesystem at run time.

## Frontmatter schema

Every `pages/*.md` file begins with a TOML frontmatter block delimited by `+++` lines:

```
+++
name = "record"
summary = "Record a new annotation"
sees_also = ["reply", "resolve"]
since = "0.5.0"
+++

# qualifier record

(page body...)
```

**Fields:**

| Field       | Required | Type           | Purpose                                                                  |
|-------------|----------|----------------|--------------------------------------------------------------------------|
| `name`      | yes      | string         | Topic key. Must match the filename stem (or be `_overview` for overview).|
| `summary`   | yes      | string         | One-line description used in the topic index.                            |
| `sees_also` | no       | array<string>  | Names of related topics. Stored on `Page`; rendering deferred.           |
| `since`     | no       | string         | Version the topic was first present (e.g., `"0.5.0"`).                   |

The first `+++` line MUST be the very first line of the file. The frontmatter ends at the next line that is exactly `+++`. The body is everything after that, leading whitespace preserved (the body normally starts with a blank line before the `#` heading).

## Filename convention

- Filenames matching `[a-z][a-z0-9_-]*\.md` are topics. Their `name` frontmatter field MUST equal the filename stem.
- Filenames beginning with `_` are *internal* pages, excluded from the topic registry. The build emits them as named consts. Currently only `_overview.md` exists; the convention generalizes to e.g. `_about.md` or `_compat.md` later.
- The build fails if it encounters a filename that doesn't match either convention.

## `Page` struct (new shape)

In `src/cli/commands/agents/mod.rs`:

```rust
pub struct Page {
    pub name: &'static str,
    pub summary: &'static str,
    pub sees_also: &'static [&'static str],
    pub since: Option<&'static str>,
    pub body: &'static str,
}
```

The build emits `PAGES: &[Page]` and individual consts for underscore-prefixed pages.

## `build.rs` algorithm

A new `build.rs` at the crate root performs, at compile time:

1. Read `cargo:rerun-if-changed=src/cli/commands/agents/pages` so edits trigger a rebuild.
2. Walk `src/cli/commands/agents/pages/*.md` (sorted lexicographically for deterministic output).
3. For each file:
   1. Read full contents as UTF-8.
   2. Confirm the first line is exactly `+++`. Locate the next `+++`-only line.
   3. Parse the lines between as TOML using the `toml` crate, deserialized via `serde::Deserialize` into a `PageMeta` struct.
   4. Capture the body (everything after the closing `+++` line).
   5. Validate:
      - `name` is non-empty.
      - For non-underscore filenames: `name == filename_stem`.
      - For `_overview.md`: `name == "_overview"`.
      - `summary` is non-empty (for non-underscore files; the overview SHOULD have a summary too but enforcement is on topic pages).
4. Detect duplicates: no two `Page`s may share the same `name`.
5. Emit `$OUT_DIR/agents_pages.rs` containing:
   - `pub const OVERVIEW: &str = "<body of _overview.md>";`
   - `pub const PAGES: &[Page] = &[...];` populated from non-underscore files in lexicographic order.

Build errors include the offending file path and a clear reason. Examples:

- `src/cli/commands/agents/pages/record.md: missing required field 'summary' in frontmatter`
- `src/cli/commands/agents/pages/record.md: name 'reply' does not match filename stem 'record'`
- `src/cli/commands/agents/pages/typo.md: no '+++' frontmatter delimiter on first line`
- `pages/record.md and pages/aliased.md: duplicate name 'record'`

## `mod.rs` changes

The current code:

```rust
const OVERVIEW: &str = include_str!("pages/_overview.md");

const PAGES: &[Page] = &[
    Page { name: "concepts", summary: "...", body: include_str!("pages/concepts.md") },
    // ... 11 more entries
];
```

becomes:

```rust
include!(concat!(env!("OUT_DIR"), "/agents_pages.rs"));
```

The hand-coded summaries are removed. The `Page` struct definition stays in `mod.rs` (or moves to a tiny `page.rs` sibling for clarity; either is fine).

`render_overview()`, `topic_names()`, and `run()` keep their current shape. They consume the generated `OVERVIEW` and `PAGES` consts identically.

## Page-file edits

Each existing page gets a frontmatter prepend. Concrete edits:

- `_overview.md` — add `name = "_overview"`. No other frontmatter required.
- `concepts.md`, `workflows.md`, `pitfalls.md` — add `name`, `summary`, `since = "0.5.0"`.
- `record.md`, `reply.md`, `resolve.md`, `emit.md`, `show.md`, `ls.md`, `praise.md`, `review.md`, `compact.md` — add `name`, `summary`, `since = "0.5.0"`. Optionally `sees_also` for the obviously-related pairs (`record` ↔ `reply`/`resolve`; `reply` ↔ `resolve`; etc.) — see "Suggested `sees_also` map" below.

The summaries used in frontmatter MUST match the strings currently hand-coded in `mod.rs::PAGES` to preserve the exact `qualifier agents` orientation output.

## Suggested `sees_also` map

Not enforced; included so the implementer doesn't have to invent it. Skip any pairing that doesn't feel natural.

- `record` → `["reply", "resolve", "emit"]`
- `reply` → `["resolve", "record"]`
- `resolve` → `["reply", "record"]`
- `emit` → `["record"]`
- `show` → `["ls", "praise", "review"]`
- `ls` → `["show"]`
- `praise` → `["show"]`
- `review` → `["show", "compact"]`
- `compact` → `["review"]`
- `concepts`, `workflows`, `pitfalls` → omit (`sees_also = []` or absent)

## Dependencies added

In `Cargo.toml`:

```toml
[build-dependencies]
toml = "0.8"
serde = { version = "1", features = ["derive"] }
```

These do not affect the runtime binary — they are only used by `build.rs`.

## Testing

Existing tests pass without changes — the user-visible CLI behavior is identical.

Add to `tests/cli_integration.rs`:

```rust
#[test]
fn test_agents_orientation_summaries_match_pages() {
    // Lock in the contract that the orientation page renders the topic
    // index from frontmatter summaries. We assert each topic's summary
    // appears in the bare-agents output.
    let dir = tempfile::tempdir().unwrap();
    let (stdout, _stderr, code) = run_qualifier(dir.path(), &["agents"]);
    assert_eq!(code, 0);
    for needle in [
        "Annotation model, kinds, supersession",     // concepts summary
        "Worked recipes for common tasks",            // workflows summary
        "Common mistakes agents make with qualifier",// pitfalls summary
        "Record a new annotation",                    // record summary
    ] {
        assert!(
            stdout.contains(needle),
            "orientation should include summary '{needle}': {stdout}"
        );
    }
}
```

Add a unit test in `agents/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn overview_contains_topics_sentinel() {
        // Existing test, preserved.
        assert!(super::OVERVIEW.contains("{{TOPICS}}"));
    }
}
```

## File touch list

- New: `build.rs` at crate root (~80 lines)
- New: dependency lines in `Cargo.toml` `[build-dependencies]`
- Modified: `src/cli/commands/agents/mod.rs` — drop hand-coded `&[Page]`, add `include!`, expand `Page` struct
- Modified: `src/cli/commands/agents/pages/_overview.md` — add minimal frontmatter
- Modified: `src/cli/commands/agents/pages/{concepts,workflows,pitfalls,record,reply,resolve,emit,show,ls,praise,review,compact}.md` — add frontmatter (12 files)
- Modified: `tests/cli_integration.rs` — add the orientation/summary test
- Modified: `Cargo.toml` — bump version to 0.5.1 (patch — no user-visible behavior change)
- Modified: `CHANGELOG.md` — note the build-time refactor under an internal/changed section
- Untouched: README.md (no user-visible change)
- Untouched: AGENTS-CLI.md (this migration is qualifier-internal; the protocol is silent on storage)

## Out of scope / future

- Rendering `sees_also` as a "See also: …" footer on each page.
- A `--probe` flag for AGENTS-CLI discovery (covered separately if/when AGENTS-CLI 0.2 lands).
- A JSON output mode (deferred per AGENTS-CLI 0.1 out-of-scope list).
- Auto-checking that `since` versions are monotone or match `Cargo.toml`.
