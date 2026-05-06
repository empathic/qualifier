# `qualifier agents` — agent-bootstrap subcommand

## Goal

Let an AI coding agent working in a user's repo bootstrap into productive use of `qualifier` without the user installing a separate skill, prompt, or doc bundle. The agent reads `qualifier --help`, sees a clearly agent-targeted entry, and runs `qualifier agents` to get a self-contained guide. The guide covers concepts, workflows, and a per-subcommand reference, all bundled with the binary.

## Non-goals

- Replacing or competing with `--help`. Clap-generated help stays for humans.
- Auto-generating content from clap definitions. Pages are hand-written prose, including syntax sections.
- Multi-format rendering. Output is plain UTF-8 markdown source — no ANSI, no pager, no HTML.
- Online updates. The guide ships frozen with the installed binary version. An agent gets exactly the doc that matches the tool it can call.

## Naming

Subcommand is `agents` (plural). Rationale: matches the established `AGENTS.md` convention that coding agents already pattern-match on, and reads as "instructions for agents" the way `man` reads as "manual." Help description carries the imperative meaning: `Self-contained guide for AI coding agents (start here)`.

The existing `AGENTS.md` at the repo root is contributor-facing (instructions for an agent helping develop qualifier). The new `agents` subcommand is consumer-facing (instructions for an agent using qualifier as a tool). The two coexist; the spec does not modify `AGENTS.md`.

## Module layout

```
src/cli/commands/agents/
  mod.rs                 // clap Args, run(), dispatch, registry
  pages/
    _overview.md         // printed by bare `qualifier agents`
    concepts.md
    workflows.md
    pitfalls.md
    record.md
    reply.md
    resolve.md
    emit.md
    show.md
    ls.md
    praise.md
    review.md
    compact.md
```

`haiku` is intentionally omitted — it's a flavor command with no agent-relevant usage guidance. The page set covers exactly the subcommands an agent should learn.

`pages/` holds pure markdown content. `mod.rs` owns dispatch and the registry. The leading underscore on `_overview.md` keeps the filename out of the topic namespace and signals "not a topic — it's the index."

## Registry

```rust
struct Page {
    name: &'static str,
    summary: &'static str,
    body: &'static str,
}

const PAGES: &[Page] = &[
    Page { name: "concepts",  summary: "Annotation model, kinds, supersession, IDs, .qual layout",
           body: include_str!("pages/concepts.md") },
    Page { name: "workflows", summary: "Worked recipes for common tasks",
           body: include_str!("pages/workflows.md") },
    Page { name: "pitfalls",  summary: "Common mistakes agents make with qualifier",
           body: include_str!("pages/pitfalls.md") },
    Page { name: "record",    summary: "Record a new annotation",
           body: include_str!("pages/record.md") },
    // … one entry per remaining subcommand
];
```

The registry is the only place that needs editing when a topic is added or removed. The overview's `{{TOPICS}}` substitution renders `name — summary` for each entry.

## CLI surface

```rust
#[derive(clap::Parser)]
pub struct Args {
    /// Topic name (e.g. `record`, `concepts`, `workflows`). Omit to print the orientation page.
    topic: Option<String>,
}

pub fn run(args: Args) -> crate::Result<()>;
```

Behavior:

| Invocation                     | Behavior                                                                                  |
|--------------------------------|-------------------------------------------------------------------------------------------|
| `qualifier agents`             | Print rendered overview (body of `_overview.md` with `{{TOPICS}}` expanded)               |
| `qualifier agents <topic>`     | Print that page's `body` verbatim to stdout                                               |
| `qualifier agents <unknown>`   | `eprintln!("qualifier agents: no such topic '<x>'. Available: ...")` and exit code 2     |
| `qualifier agents --help`      | Standard clap help (one positional arg, no flags). Untouched.                             |

Why a single positional rather than nested clap subcommands: every page prints text and accepts no flags. A clap subcommand tree would add boilerplate without earning a meaningful per-topic `--help` (the page *is* the help).

Output is stdout only; no ANSI, no pager. An agent consuming the output wants raw text.

The error path uses exit code 2 to match clap's convention for usage errors. Since `cli::run` exits 1 on any returned `Err`, `agents::run` handles its own usage error: print the message to stderr and call `std::process::exit(2)` directly rather than returning an `Err`.

## Help template integration

`HELP_TEMPLATE` in `src/cli/mod.rs` gets a new group above the existing ones:

```
For AI agents:
  agents     Self-contained guide for AI coding agents (start here)

Record observations:
  ...
```

The group header `For AI agents:` is the discoverability signal — a model scanning grouped help output recognizes this entry as targeted at it. Description ("start here") is the imperative nudge. `agents` does not appear in any other group.

The `Commands` enum gains:

```rust
/// Self-contained guide for AI coding agents (start here)
Agents(commands::agents::Args),
```

dispatched in the `run` match like every other subcommand.

The existing comment above `HELP_TEMPLATE` already warns to keep template and enum in sync — no new instruction needed.

## Page content templates

Actual prose is written during implementation; the spec defines structure and sizing.

### `_overview.md` (~80–120 lines)

1. Two-sentence framing: what qualifier is, what shape of artifact it produces.
2. "When to use this tool" — three bullets.
3. "When NOT to use it" — e.g., one-off scratch debugging, info that belongs in a commit message.
4. Quickstart: one minimal end-to-end example (record → show → resolve).
5. `{{TOPICS}}` index — auto-rendered from registry.
6. Pointer: "For more on any topic, run `qualifier agents <topic>`."

### `concepts.md` (~150 lines)

- Annotation envelope (metabox, type, subject, issuer, body) and why each matters to an agent.
- Kinds (`note`, `concern`, `bug`, `praise`, etc.) and how to choose.
- Supersession: mechanics, why chains must be acyclic, cross-subject rejection.
- `.qual` file layout: directory-level vs file-level, discovery rules, `.qualignore`.
- Issuer URIs and `issuer_type`.
- BLAKE3 content addressing — implication that changing canonical body changes the ID.

### `workflows.md` (~150 lines)

Five recipes, each a short narrative plus commands:

1. Record a finding during code review.
2. Reply to an existing observation with new info.
3. Resolve a finding once it's addressed.
4. Triage stale annotations after a refactor (`review`).
5. Compact a noisy `.qual` file.

### `pitfalls.md` (~60 lines, grows over time)

- Forgetting `--span` when scope is narrower than the file.
- Conflating `reply` (add info) with `resolve` (close).
- Recording an annotation when a commit message would do.
- Editing `.qual` files by hand instead of using `record`/`emit`.
- Section labelled "Add new pitfalls here as we observe them."

### Per-subcommand pages (~50–100 lines each)

Each follows the same template:

1. One-sentence purpose.
2. When to use it (vs adjacent commands).
3. Common invocation forms with realistic examples.
4. Flags worth knowing about, in prose (not a flag table — agents can run `qualifier <cmd> --help` for the table).
5. Gotchas specific to this command.

## Testing

CLI integration tests live in `tests/cli_integration.rs`. Add cases:

1. `qualifier agents` exits 0 and prints non-empty output containing the rendered topic index (assert that each registry entry's name and summary appear).
2. `qualifier agents concepts` exits 0 and prints non-empty output (smoke check).
3. `qualifier agents <every_registered_topic>` exits 0 in a parameterized loop, asserting non-empty output. Catches missing `include_str!` files and registry mismatches.
4. `qualifier agents bogus-topic` exits non-zero and stderr contains `no such topic`.
5. `qualifier --help` output contains the exact line `For AI agents:` and the `agents` row, ensuring discoverability stays wired.

A small unit test in `agents/mod.rs` verifies that `_overview.md` contains the literal `{{TOPICS}}` sentinel — guards against accidentally removing it.

No new test fixtures or helpers are needed. The pages are static; coverage is mostly "they render and are non-empty."

## Out of scope / future

- A `qualifier agents pitfalls` page that grows from observed agent failures over time. Stub at launch.
- Republishing pages to the docs site verbatim. The .md files are website-friendly; if/when desired this is a docs-site change, not a qualifier change.
- Search or fuzzy match for topic names. If `agents fop` should suggest `agents foo`, defer that until we have a topic count where guessing helps.
- Auto-derived per-subcommand syntax sections from clap. Pages stay hand-written; the small drift cost is accepted.
- Localization. English only.

## File touch list

- New: `src/cli/commands/agents/mod.rs`
- New: `src/cli/commands/agents/pages/*.md` (13 files: `_overview` + 3 topical + 9 per-subcommand)
- Modify: `src/cli/commands.rs` or `src/cli/commands/mod.rs` — add `pub mod agents;`
- Modify: `src/cli/mod.rs` — add `Agents` variant, add dispatch arm, update `HELP_TEMPLATE`
- Modify: `tests/cli_integration.rs` — add the test cases above
- Modify: `Cargo.toml` — bump version (per AGENTS.md "Keeping Things in Sync")
- Modify: `CHANGELOG.md` — note the new subcommand
- Modify: `README.md` — add `agents` row to the CLI Commands table if one exists
