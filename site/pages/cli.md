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

**Initialize and orient:**

```
qualifier init                             Bootstrap a project: VCS merge config and agent directives
qualifier agents   [topic]                 Self-contained guide for AI coding agents
```

**Write records:**

```
qualifier record   <kind> <location> [message]   Record an annotation
qualifier reply    <target> <message>            Reply to an existing record
qualifier resolve  <target> [message]            Resolve (close) an existing record
qualifier emit     <type> <subject> --body JSON  Emit a raw record of any type
```

`<kind>` accepts the built-in kinds (`pass`, `fail`, `blocker`, `concern`,
`comment`, `praise`, `suggestion`, `waiver`, `resolve`) or any custom string.
`<location>` is a path with an optional span (e.g., `src/auth.rs:42`).
`<target>` is an ID prefix (4 or more lowercase hex characters) or a
`<location>`.

**Inspect:**

```
qualifier show     <artifact>              Show annotations for an artifact
qualifier threads  [location...]           List conversation threads across the project
qualifier ls       [--kind K]              List artifacts (optionally by kind)
qualifier praise   <artifact>              Show who annotated and why (alias: blame)
qualifier review   [subject]               Check freshness of span-bound annotations
qualifier diff     [ref]                   Show records added, changed, resolved, or drifted since a git ref
```

**Maintain:**

```
qualifier compact  <artifact> [options]    Compact a .qual file
```

The read commands (`show`, `threads`, `ls`, `praise`, `review`, `diff`)
and `record`, `reply`, and `resolve` accept `--format json` for
machine-readable output.

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

### Record and resolve an issue

```bash
# Record a concern at a specific line
qualifier record concern src/parser.rs:42 "Panics on malformed input"

# See it
qualifier show src/parser.rs

# Reply (id-prefix, min 4 chars — or a location)
qualifier reply a1b2 "Good catch, fixed in latest commit"
qualifier reply src/parser.rs:42 "Good catch, fixed in latest commit"

# Close it
qualifier resolve a1b2
```

### Threaded conversations

```bash
qualifier show src/parser.rs

  src/parser.rs

  Open threads (2):
    [c1acc3f5] praise     src/parser.rs  Excellent property test coverage  (bob, 2026-10-01)

    [e5daa3cd] suggestion src/parser.rs:10:12  Consider fuzzing  (alice, ai, 2026-10-01) — needs decision
        [f9156cb5] comment    Worth it for parse()  (carol, 2026-10-01)

  Closed threads (1):
    [274357ca] concern    src/parser.rs:42  Panics on malformed input  (alice, 2026-10-01) — closed (fixed) by alice: Resolved
```

Replies are indented under the record they answer. A closed thread is one
line that carries its closing `resolve`: the reason, who closed it, and
the resolve's summary.

Use `--all` to include edit history and superseded records. Use `--pretty`
to print the source lines around each span (with `--format json`, it adds a
`context` field). `--type <TYPE>` keeps only records of one envelope type.
An artifact with no records prints `No records found` and exits 0.

### Record a quality concern with full options

```bash
qualifier record concern src/parser.rs "Panics on malformed UTF-8 input" \
  --suggested-fix "Replace .unwrap() on line 42 with error propagation" \
  --tag robustness --tag error-handling \
  --issuer "mailto:alice@example.com" \
  --span 42:58
```

`--span` overrides any span parsed from `<location>`. When the source file is
readable, `content_hash` is auto-computed.

### Emit a raw record (raw or non-annotation types)

```bash
# A SPDX license record
qualifier emit license src/lib.rs --body '{"spdx_id":"MIT"}'

# A custom URI-typed record (round-trips via Record::Unknown)
qualifier emit https://example.com/lint/v1 src/parser.rs \
  --body '{"rule":"no-panic","matches":3}'
```

`emit` is a low-level passthrough: the body's fields and values are kept
as given (written with keys in canonical order). When `<type>` is
`annotation`, the body is validated against the annotation schema; other
types are not validated.

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

### List artifacts

```bash
qualifier ls                 # every artifact with live records, and their counts
qualifier ls --kind blocker  # only artifacts with a live blocker
```

### Batch annotation (for agents)

```bash
# Pipe overrides JSONL from stdin: {kind, location, message, ...}
cat overrides.jsonl | qualifier record --stdin

# Or pipe complete records (envelope + body)
cat records.jsonl | qualifier emit --stdin
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

| Priority | Source            | Example                           |
| -------- | ----------------- | --------------------------------- |
| 1        | CLI flags         | `--issuer mailto:me@example.com`  |
| 2        | Environment       | `QUALIFIER_ISSUER`                |
| 3        | Project config    | `.qualifier.toml`                 |
| 4        | User config       | `~/.config/qualifier/config.toml` |
| 5        | Built-in defaults |                                   |
