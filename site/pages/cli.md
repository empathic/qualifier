---
layout: base.njk
title: CLI
nav: cli
permalink: /cli/
---

# CLI

The `qualifier` crate installs a binary called `qualifier`.

```bash
cargo install qualifier
```

## Commands

```
qualifier
  attest    <artifact> [options]     Add an attestation
  show      <artifact>               Show attestations and scores
  score     [artifact...]            Compute and display scores
  ls        [--below N] [--kind K]   List artifacts by score/kind
  check     [--min-score N]          CI gate: exit non-zero if below threshold
  compact   <artifact> [options]     Compact a .qual file
  graph     [--format dot|json]      Visualize the dependency graph
  init                               Initialize qualifier in a repo
  blame     <artifact>               Per-line VCS attribution
```

All commands that produce output accept `--format json` for machine-readable output.

<svg class="topo topo-wide" viewBox="0 0 900 40" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <line x1="0" y1="20" x2="900" y2="20" stroke="#818cf8" stroke-width="0.5" opacity="0.1"/>
  <line x1="0" y1="0" x2="0" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="180" y1="10" x2="180" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="360" y1="0" x2="360" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="540" y1="10" x2="540" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="720" y1="0" x2="720" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="900" y1="0" x2="900" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
</svg>

## Typical workflows

### Record a quality concern

```bash
qualifier attest src/parser.rs --kind concern --score -30 \
  --summary "Panics on malformed UTF-8 input" \
  --suggested-fix "Replace .unwrap() on line 42 with error propagation" \
  --tag robustness --tag error-handling \
  --author "alice@example.com"
```

### See scores for all artifacts

```bash
qualifier score

  ARTIFACT              RAW    EFF   STATUS
  lib/crypto            -20    -20   ██░░░░░░░░  blocker
  src/auth.rs           -30    -30   █░░░░░░░░░  blocker
  lib/http               50     50   ████████░░  healthy
  src/parser.rs            5      5   ██████░░░░  ok
  bin/server              50    -30   █░░░░░░░░░  blocker
```

### CI gating

```bash
# In your CI pipeline
qualifier check --min-score 0

# Fails with exit code 1 if any artifact is below threshold
# Stderr shows which artifacts failed:
#   FAIL  lib/crypto      eff: -20  (threshold: 0)
#   FAIL  src/auth.rs     eff: -30  (threshold: 0)
#   FAIL  bin/server      eff: -30  (threshold: 0)
```

### Show details for one artifact

```bash
qualifier show src/parser.rs

  src/parser.rs
  Raw score:       5
  Effective score: 5

  Attestations (3):
    [-30] concern     "Panics on malformed UTF-8 input"       alice  2026-02-24
    [+40] praise      "Excellent property-based test coverage" bob    2026-02-24
    [ -5] suggestion  "Consider adding fuzzing targets"        carol  2026-02-24
```

### Compact old attestations

```bash
# Preview what compaction would do
qualifier compact src/parser.rs --dry-run

# Prune superseded attestations
qualifier compact src/parser.rs

# Collapse everything to a single epoch attestation
qualifier compact src/parser.rs --snapshot

# Compact every .qual file in the repo
qualifier compact --all
```

### Visualize the dependency graph

```bash
# Output as Graphviz DOT
qualifier graph --format dot | dot -Tpng -o graph.png

# Output as JSON
qualifier graph --format json
```

### Initialize qualifier in a repo

```bash
qualifier init

  Created qualifier.graph.jsonl (empty — populate with your dependency graph)
  Detected VCS: git
  Added *.qual merge=union to .gitattributes
```

### List the worst offenders

```bash
qualifier ls --below 0
qualifier ls --kind blocker
qualifier ls --unqualified   # artifacts with no attestations
```

### Batch attestation (for agents)

```bash
# Pipe JSONL attestations from stdin
cat attestations.jsonl | qualifier attest --stdin
```

<svg class="topo topo-wide" viewBox="0 0 900 40" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <line x1="0" y1="20" x2="900" y2="20" stroke="#818cf8" stroke-width="0.5" opacity="0.1"/>
  <line x1="0" y1="0" x2="0" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="225" y1="10" x2="225" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="450" y1="0" x2="450" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="675" y1="10" x2="675" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="900" y1="0" x2="900" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
</svg>

## Configuration

Qualifier uses layered configuration (highest wins):

| Priority | Source             | Example                                    |
| -------- | ------------------ | ------------------------------------------ |
| 1        | CLI flags          | `--graph path/to/graph.jsonl`              |
| 2        | Environment        | `QUALIFIER_GRAPH`, `QUALIFIER_AUTHOR`      |
| 3        | Project config     | `.qualifier.toml`                          |
| 4        | User config        | `~/.config/qualifier/config.toml`          |
| 5        | Built-in defaults  |                                            |

