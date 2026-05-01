---
layout: base.njk
title: CLI
nav: cli
prose: true
permalink: /cli/
---

# CLI

The `qualifier` crate installs a binary called `qualifier`.

```bash
cargo install qualifier
```

## Commands

**Record signals:**

```
qualifier comment  <location> <message>    Add a comment
qualifier flag     <location> <message>    Flag a concern
qualifier suggest  <location> <message>    Suggest a change
qualifier approve  <location> <message>    Approve an artifact
qualifier reject   <location> <message>    Reject an artifact
qualifier reply    <id-prefix> <message>   Reply to a record
qualifier resolve  <id-prefix> [message]   Resolve a record
```

**Analysis:**

```
qualifier show     <artifact>              Show annotations and scores
qualifier score    [artifact...]           Compute and display scores
qualifier ls       [--below N] [--kind K]  List artifacts by score/kind
qualifier check    [--min-score N]         CI gate: exit non-zero if below threshold
```

**Management:**

```
qualifier attest   <artifact> [options]    Add an annotation (low-level)
qualifier compact  <artifact> [options]    Compact a .qual file
qualifier graph    [--format dot|json]     Visualize the dependency graph
qualifier init                             Initialize qualifier in a repo
qualifier praise   <artifact>              Show who attested and why
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

### Flag and resolve an issue

```bash
# Flag a concern at a specific line
qualifier flag src/parser.rs:42 "Panics on malformed input"

# See the flag
qualifier show src/parser.rs

# Reply to the flag (ID prefix, min 4 chars)
qualifier reply a1b2 "Good catch, fixed in latest commit"

# Close it
qualifier resolve a1b2

# Negative score is gone
qualifier score
```

### Threaded conversations

```bash
qualifier show src/parser.rs

  src/parser.rs
  Raw score:       5
  Effective score: 5

  Records (4):
    [-10] concern  L42 "Panics on malformed input"          alice  2026-03-01  a1b2c3d4
    ├── [   ] comment  "Good catch, fixed in latest commit" bob    2026-03-01  b2c3d4e5
    └── [  0] resolve  "Resolved"                           alice  2026-03-01  c3d4e5f6
    [+40] praise       "Excellent property-based test coverage"  bob  2026-02-24  e5f6a7b8
```

Replies and resolves are threaded under their parent with tree-drawing characters.

### Signal commands at a glance

| Command   | Kind       | Default Score | Use for                             |
| --------- | ---------- | ------------- | ----------------------------------- |
| `comment` | comment    | absent        | Observations, questions, discussion |
| `flag`    | concern    | -10           | Non-blocking issues                 |
| `suggest` | suggestion | -5            | Proposed improvements               |
| `approve` | pass       | +20           | Passes a quality bar                |
| `reject`  | fail       | -20           | Fails a quality bar                 |

### Show details for one artifact

```bash
qualifier show src/parser.rs

  src/parser.rs
  Raw score:       5
  Effective score: 5

  Records (3):
    [-30] concern     L42–58 "Panics on malformed UTF-8 input"  alice  2026-02-24  a1b2c3d4
    [+40] praise      "Excellent property-based test coverage"   bob    2026-02-24  e5f6a7b8
    [ -5] suggestion  "Consider adding fuzzing targets"          carol  2026-02-24  f1f2f3f4
```

Use `--all` to include resolved/superseded records. Use `--pretty` to force colored output.

### Record a quality concern (low-level)

```bash
qualifier attest src/parser.rs --kind concern --score -30 \
  --summary "Panics on malformed UTF-8 input" \
  --suggested-fix "Replace .unwrap() on line 42 with error propagation" \
  --tag robustness --tag error-handling \
  --issuer "mailto:alice@example.com"
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

### Compact old annotations

```bash
# Preview what compaction would do
qualifier compact src/parser.rs --dry-run

# Prune superseded annotations
qualifier compact src/parser.rs

# Collapse everything to a single epoch annotation
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
qualifier ls --unqualified   # artifacts with no annotations
```

### Batch annotation (for agents)

```bash
# Pipe JSONL annotations from stdin
cat annotations.jsonl | qualifier attest --stdin
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

| Priority | Source            | Example                               |
| -------- | ----------------- | ------------------------------------- |
| 1        | CLI flags         | `--graph path/to/graph.jsonl`         |
| 2        | Environment       | `QUALIFIER_GRAPH`, `QUALIFIER_ISSUER` |
| 3        | Project config    | `.qualifier.toml`                     |
| 4        | User config       | `~/.config/qualifier/config.toml`     |
| 5        | Built-in defaults |                                       |
