# Qualifier

**Know where the bodies are buried.** Structured quality signals for code, with scores that propagate through your dependency graph.

## The Problem

Someone dropped 30,000 lines of slopcode in your lap. The test suite passes (mostly), the docs are "coming soon," and the last meaningful code review was three sprints ago. You need to know what's safe and what's not — and your tools give you nothing but green checkmarks or silence.

Qualifier records what you actually know about code quality and computes aggregate scores that tell you where to look first. Everything lives in VCS-friendly JSONL files alongside your source code. No server, no database, no lock-in.

## What Qualifier Adds

| What                     | Without Qualifier             | With Qualifier                                      |
| ------------------------ | ----------------------------- | --------------------------------------------------- |
| Quality tracking         | Spreadsheets, tickets, memory | Structured `.qual` files in your repo                |
| Score propagation        | Manual dependency analysis    | Automatic through the dependency graph               |
| CI gating                | Custom scripts                | `qualifier check --min-score 0`                      |
| Agent integration        | None                          | JSON output, batch attestation, suggested fixes      |
| Merge conflicts          | Guaranteed with shared files  | Structurally impossible (append-only JSONL)           |
| History                  | Lost in ticket graveyards     | VCS-native — blame, diff, bisect all work            |

## Quick Start

```sh
# Install
cargo install qualifier

# Initialize in your repo
qualifier init

# Record a quality concern
qualifier attest src/parser.rs \
  --kind concern \
  --score -30 \
  --summary "Panics on malformed input"

# View scores
qualifier score

# Show details for one artifact
qualifier show src/parser.rs

# CI gate (exits non-zero if any artifact is below threshold)
qualifier check --min-score 0
```

## Core Concepts

**Attestations** are immutable quality signals: pass, fail, blocker, concern, praise, suggestion, waiver. Each carries a score delta (-100 to +100) and is content-addressed via BLAKE3. Attestations are append-only — updates use supersession chains rather than mutation.

**Raw score** is the clamped sum of active (non-superseded) attestation scores for an artifact, bounded to [-100, 100].

**Effective score** is the minimum of an artifact's raw score and the effective scores of all its dependencies. A low-quality dependency pulls down everything that depends on it.

**Compaction** prunes superseded attestations or collapses history into epoch records, preserving scores while reducing file size.

**.qual files** are JSONL files containing records. The recommended layout is one `.qual` file per directory. See [SPEC.md](SPEC.md) for layout options and trade-offs.

## CLI Commands

| Command | Description |
|---------|-------------|
| `qualifier attest <artifact>` | Record an attestation |
| `qualifier show <artifact>` | Show attestations and scores for an artifact |
| `qualifier score` | Display scores for all qualified artifacts |
| `qualifier ls` | List artifacts, filterable by score or kind |
| `qualifier check` | CI gate: exit non-zero if scores below threshold |
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
- `span` field targets specific line ranges for precise attestations
- `--graph` flag accepts dependency graphs from build tools

## Specification

See [SPEC.md](SPEC.md) for the full format specification, scoring algorithm, and design rationale.

## License

MIT OR Apache-2.0
