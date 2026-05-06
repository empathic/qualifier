+++
name = "show"
summary = "Show annotations for an artifact"
sees_also = ["ls", "praise", "review"]
since = "0.5.0"
+++

# qualifier show

## Purpose

Display the active annotations for a specific artifact, threaded by reply
chain.

## When to use it

`show` is the primary read command for a single artifact. It filters out
superseded records by default and renders replies indented under their parent,
giving a conversational view of all open concerns, comments, and suggestions
on that file. Use `ls` when you want a cross-artifact overview (what
artifacts have any annotations at all). Use `praise` when you want authorship
detail — who wrote each annotation and when, with issuer type shown. Use
`review` when you need to know whether span-bound annotations are still
pointing at the right code.

## Common invocations

```bash
# Show active annotations on a file
qualifier show src/auth.rs

# Show with source context rendered inline (compiler-diagnostic style)
qualifier show src/auth.rs --pretty

# Show all records including resolved ones
qualifier show src/auth.rs --all

# Programmatic output — returns JSON object with a "records" array
qualifier show src/auth.rs --format json

# Show only epoch records for an artifact
qualifier show src/auth.rs --type epoch
```

## Flags worth knowing

**`--format json`** emits a JSON object `{"subject": "...", "records": [...]}`.
Each record is the full envelope and body. This is the right mode when an
agent needs to inspect annotation IDs, `body.references` chains, or span
details programmatically.

**`--pretty`** adds source context around each span-addressed annotation,
rendered in the same style as compiler diagnostics (filename, line numbers,
and a few lines of surrounding code). Useful for human review; for agents
parsing output, `--format json --pretty` adds a `"context"` field to each
span record.

**`--all`** disables the default filter that hides superseded records and
`resolve` tombstones. Use it when you need to audit the full annotation
history or diagnose a supersession chain.

**`--type <TYPE>`** filters by envelope record type (`annotation`, `epoch`,
`dependency`, or any custom URI). This is distinct from filtering by
annotation `kind` — `--type epoch` shows epoch records; `--kind concern`
is not a flag on `show` (use `ls --kind` for kind-based filtering across
artifacts).

## Gotchas

- `show` exits with an error if no records are found for the artifact. An
  empty result is not silent — test the exit code when scripting.
- Dependency records are hidden in human output (they are graph metadata, not
  quality signals) but appear in `--format json` output.
- Resolved records are hidden by default. If an artifact appears to have no
  annotations but you expect some, try `--all` to check whether they were
  resolved.
- The `artifact` argument is the subject name as stored in the `.qual` file
  (e.g., `src/auth.rs`), not a filesystem glob. There is no wildcard matching.
