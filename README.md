# Qualifier

**Quality signals shouldn't need a process.** Don't wait until PR time to comment on code. Do it now — it'll be there when you (or your bots) get to it.

## The Problem

Quality improvement is batched behind gates. You notice a problem on Tuesday but the PR isn't until Friday. Inline comments disappear into merged PRs. Three sprints later, nobody remembers what was flagged, what was fixed, and what was quietly ignored.

Qualifier lets you record quality signals the moment you see them — concerns, suggestions, approvals, observations — as structured records in `.qual` files next to your source. No process required. Signals accumulate continuously and thread into conversations. Scored signals (concerns, passes, blockers) carry numeric deltas that propagate through your dependency graph. Unscored signals (comments, observations) capture knowledge without affecting scores. VCS-friendly JSONL that merges cleanly, diffs readably, and never gets lost.

## What Qualifier Adds

| What                     | Without Qualifier             | With Qualifier                                      |
| ------------------------ | ----------------------------- | --------------------------------------------------- |
| Quality signals          | Batched behind PRs and gates  | Recorded when you see them, always available         |
| Knowledge capture        | Spreadsheets, tickets, memory | Structured `.qual` files in your repo                |
| Score propagation        | Manual dependency analysis    | Automatic through the dependency graph               |
| CI gating                | Custom scripts                | `qualifier check --min-score 0`                      |
| Agent integration        | None                          | JSON output, batch annotation, suggested fixes      |
| Merge conflicts          | Guaranteed with shared files  | Structurally impossible (append-only JSONL)           |
| History                  | Lost in ticket graveyards     | VCS-native — blame, diff, bisect all work            |

## Quick Start

```sh
# Install
cargo install qualifier

# Initialize in your repo
qualifier init

# Flag a concern at a specific line
qualifier flag src/parser.rs:42 "Panics on malformed input"

# See the flag with threaded display
qualifier show src/parser.rs

# Reply to it (ID prefix, min 4 chars)
qualifier reply a1b2 "Good catch, fixed in latest commit"

# Close it
qualifier resolve a1b2

# See how scores look now
qualifier score

# CI gate (exits non-zero if any artifact is below threshold)
qualifier check --min-score 0
```

## Core Concepts

**Signals** — Flag, comment, suggest, approve, reject. Each creates an immutable record in a `.qual` file next to your source. No ceremony, no PR required. Reply to any record to build a conversation. Resolve when the issue is fixed.

**Scored signals** (concerns, passes, blockers, suggestions, etc.) carry numeric quality deltas (-100 to +100). The raw score is the clamped sum of active scored signals. Effective scores propagate through the dependency graph — your worst dependency is your ceiling.

**Unscored signals** (comments, observations) capture knowledge without moving the needle. A comment on line 42 is just as much a first-class record as a blocker — it threads, persists, and shows up in `qualifier show`.

**Records** are the building blocks: immutable, content-addressed (BLAKE3), append-only. The signal's kind determines its nature — `concern`, `pass`, `comment`, `resolve`, etc. Updates use supersession chains rather than mutation.

**Compaction** prunes superseded records or collapses history into epoch records, preserving scores while reducing file size.

**.qual files** are JSONL files containing records. The recommended layout is one `.qual` file per directory. See [SPEC.md](SPEC.md) for layout options and trade-offs.

**File discovery** respects `.gitignore` and `.qualignore` (gitignore-compatible syntax) by default, so vendored or generated `.qual` files can be excluded. Pass `--no-ignore` to bypass all ignore rules. See [SPEC.md §10](SPEC.md#10-file-discovery) for details.

## CLI Commands

**Record signals:**

| Command | Description |
|---------|-------------|
| `qualifier comment <location> <message>` | Add a comment |
| `qualifier flag <location> <message>` | Flag a concern |
| `qualifier suggest <location> <message>` | Suggest a change |
| `qualifier approve <location> <message>` | Approve an artifact |
| `qualifier reject <location> <message>` | Reject an artifact |
| `qualifier reply <id-prefix> <message>` | Reply to a record |
| `qualifier resolve <id-prefix> [message]` | Resolve a record |

**Analysis:**

| Command | Description |
|---------|-------------|
| `qualifier show <artifact>` | Show annotations and scores (threaded) |
| `qualifier score` | Display scores for all qualified artifacts |
| `qualifier ls` | List artifacts, filterable by score or kind |
| `qualifier check` | CI gate: exit non-zero if scores below threshold |

**Management:**

| Command | Description |
|---------|-------------|
| `qualifier attest <artifact>` | Record an annotation (low-level) |
| `qualifier compact <artifact>` | Prune or snapshot a .qual file |
| `qualifier graph` | Visualize the dependency graph |
| `qualifier blame <artifact>` | VCS attribution for a .qual file |
| `qualifier init` | Initialize qualifier in a repository |

All read commands support `--format json` for machine-readable output.

## Agent Integration

Qualifier is built for both humans and coding agents:

- `--format json` on `score`, `show`, and `ls` for structured output
- `--stdin` batch mode reads JSONL records for bulk qualification
- `suggested_fix` field carries actionable remediation advice
- `span` field targets specific line ranges for precise annotations
- `--graph` flag accepts dependency graphs from build tools
- `qualifier reply <id> <message>` for threaded agent follow-ups to human signals
- `qualifier resolve <id>` to close issues after fixes are applied

## Specification

See [SPEC.md](SPEC.md) for the full format specification, signal model, scoring algorithm, and design rationale.

## License

MIT OR Apache-2.0
