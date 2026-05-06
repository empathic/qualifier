+++
name = "emit"
summary = "Emit a raw record of any type"
sees_also = ["record"]
since = "0.5.0"
+++

# qualifier emit

## Purpose

Emit a raw record of any type, bypassing the higher-level annotation
scaffolding.

## When to use it

Most agents should use `record`, `reply`, and `resolve` for day-to-day
annotation work — those commands construct well-formed annotation envelopes
and validate the body against the spec. `emit` is for cases that those
commands do not cover: writing `epoch` or `dependency` records, emitting
extension types defined by a custom pipeline (e.g., `license`,
`security-advisory`, `perf-measurement`, or any custom URI type), or
forwarding a pre-built record from an external tool. The body JSON is passed
through unchanged, so the caller is responsible for correctness. When
`record_type` is `annotation`, `emit` does run standard annotation validation;
for all other types, the body is stored verbatim.

## Common invocations

```bash
# Emit an epoch record to checkpoint the annotation history
qualifier emit epoch src/auth.rs \
  --body '{"summary":"Reviewed at v2.3.0","refs":["git:3aba500"]}'

# Emit a dependency record linking two artifacts
qualifier emit dependency src/auth.rs \
  --body '{"depends_on":["src/session.rs"]}'

# Emit a custom extension record type
qualifier emit https://example.com/types/perf-measurement src/hot_path.rs \
  --body '{"p99_ms":14,"baseline_ms":12}'

# Batch-emit pre-built records from a JSONL file
cat records.jsonl | qualifier emit --stdin
```

## Flags worth knowing

**`--body '<JSON>'`** is required in single-record mode. The value must be a
valid JSON object; the CLI will reject non-object values. For annotation
records the body must conform to the annotation body schema; for other types
the body is stored as-is. Quote the argument carefully — shell word splitting
on embedded spaces or braces is a common source of errors.

**`--stdin`** reads complete JSONL records from stdin, one per line. Each
line must be a full record (envelope + body). The positional `record_type`
and `subject` arguments, when provided alongside `--stdin`, act as defaults
applied to lines that are missing those fields.

**`--issuer`** and **`--issuer-type`** work the same as in `record`. When
emitting machine-generated records such as pipeline measurements, set
`--issuer-type tool` to distinguish them from human annotations.

## Gotchas

- `emit` is intentionally low-level. Prefer `record` / `reply` / `resolve`
  for annotation work — they guard against common mistakes (missing `kind`,
  invalid supersession, etc.) that `emit` does not catch.
- When `record_type` is `annotation`, the body must include at least `kind`
  and `summary`; the command will validate and reject non-conforming bodies.
  For non-annotation types, no body validation is run — a malformed body will
  be stored silently.
- The subject positional is the artifact name (e.g., `src/auth.rs`), not a
  `.qual` file path. Span notation in the subject is not parsed; `emit` is
  the low-level shape.
- Lines starting with `//` are treated as comments and skipped in `--stdin`
  mode. Blank lines are also skipped.
