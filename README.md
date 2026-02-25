# Qualifier

Deterministic quality attestations for software artifacts.

Qualifier records structured quality signals against code artifacts and
computes aggregate scores that propagate through dependency graphs. Everything
is stored in VCS-friendly JSONL files that sit alongside your source code.

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

**Attestations** are immutable quality signals: pass, fail, blocker, concern,
praise, suggestion, waiver. Each carries a score delta (-100 to +100) and is
content-addressed via BLAKE3. Attestations are append-only — updates use
supersession chains rather than mutation.

**Raw score** is the clamped sum of active (non-superseded) attestation scores
for an artifact, bounded to [-100, 100].

**Effective score** is the minimum of an artifact's raw score and the effective
scores of all its dependencies. A low-quality dependency pulls down everything
that depends on it.

**Compaction** prunes superseded attestations or collapses history into epoch
attestations, preserving scores while reducing file size.

**.qual files** are JSONL files containing attestations. The recommended layout
is one `.qual` file per directory. See [SPEC.md](SPEC.md) for layout options
and trade-offs.

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

## Example Workflow

```sh
# Code review finds an issue
qualifier attest src/auth.rs --kind blocker \
  --summary "SQL injection in login query"

# Developer fixes it and supersedes the blocker
qualifier attest src/auth.rs --kind pass --score 20 \
  --summary "Parameterized login query" \
  --supersedes abc12345

# CI verifies the repo is healthy
qualifier check --min-score 0

# Periodically compact history
qualifier compact --all
```

## Agent Integration

Qualifier is built for both humans and coding agents:

- `--format json` on `score`, `show`, and `ls` for structured output
- `--stdin` batch mode reads JSONL attestations for bulk qualification
- `suggested_fix` field carries actionable remediation advice
- `--graph` flag accepts dependency graphs from build tools (Bazel, etc.)

## Configuration

Layered configuration (highest priority first):

1. CLI flags
2. Environment variables (`QUALIFIER_GRAPH`, `QUALIFIER_AUTHOR`)
3. Project config (`.qualifier.toml`)
4. User config (`~/.config/qualifier/config.toml`)
5. Built-in defaults

## Specification

See [SPEC.md](SPEC.md) for the full format specification, scoring algorithm,
and design rationale.

## License

MIT OR Apache-2.0
