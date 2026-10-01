+++
name = "show"
summary = "Show annotations for an artifact"
sees_also = ["ls", "praise", "review"]
since = "0.5.0"
+++

# qualifier show

## Purpose

Display the threads on a specific artifact: each open thread with its live
replies, and each closed thread as one line carrying its answer.

## When to use it

`show` is the primary read command for a single artifact. It groups the
artifact's annotations into threads (the same model and renderer as
`qualifier threads`), so a reply never appears without its thread's state:
open threads come first with their live replies indented beneath the root,
then closed threads, one line each, ending in
`— closed (<reason>) by <issuer>: <resolve summary>`. Each record line ends
in `(issuer, date)`, with the issuer type in between when it is not
`human`. Use `ls` when you want a cross-artifact overview (what artifacts
have any annotations at all). Use `praise` when you want authorship detail —
who wrote each annotation and when, with its detail or suggested fix. Use
`review` when you need to know whether span-bound annotations are still
pointing at the right code.

## Common invocations

```bash
# Show active annotations on a file
qualifier show src/auth.rs

# Show with source context rendered inline (compiler-diagnostic style)
qualifier show src/auth.rs --pretty

# Also show edit history and superseded replies
qualifier show src/auth.rs --all

# Programmatic output — JSON object with "records" and "threads" arrays
qualifier show src/auth.rs --format json

# Show only epoch records for an artifact
qualifier show src/auth.rs --type epoch
```

## Flags worth knowing

**`--format json`** emits a JSON object
`{"subject": "...", "records": [...], "threads": [...]}`. `records` holds
every record shown, in file order, as the full envelope and body: each
thread's root, live replies, and closing resolve, then the artifact's other
records (epochs, dependencies, custom types). `threads` has one entry per
thread, `{"origin", "root": "<id>", "state": {...}, "closed_by": "<id>"|null}`,
with `state` as in `qualifier threads --format json`. This is the right mode
when an agent needs to inspect annotation IDs, `body.references` chains, or
span details programmatically. Human output is a summary; JSON is complete.

**`--pretty`** adds source context around each span-addressed annotation,
rendered in the same style as compiler diagnostics (filename, line numbers,
and a few lines of surrounding code). Useful for human review; for agents
parsing output, `--format json --pretty` adds a `"context"` field to each
span record.

**`--all`** also shows each open thread's edit history and superseded
replies, marked `(superseded)`, and superseded non-annotation records (JSON
gets every record of each thread). Closed threads stay one line; use
`qualifier threads --all <id>` to see a closed thread's replies. Use `--all`
when you need to audit the full annotation history or diagnose a
supersession chain.

**`--type <TYPE>`** filters by envelope record type (`annotation`, `epoch`,
`dependency`, or any custom URI). This is distinct from filtering by
annotation `kind` — `--type epoch` shows epoch records; `--kind concern`
is not a flag on `show` (use `ls --kind` for kind-based filtering across
artifacts).

## Gotchas

- An artifact with no records is not an error: `show` exits 0, printing
  `No records found for '<subject>'.` in human output, or empty `records`
  and `threads` arrays in JSON. To fail on empty in a script, check
  `.records | length` in the JSON. A non-zero exit means a real error (bad
  arguments, unreadable `.qual` files).
- Dependency records are hidden in human output (they are graph metadata, not
  quality signals) but appear in `--format json` output.
- A closed thread shows only its root and the resolve that closed it, so a
  question asked in a reply before the close is not shown; the line ends in
  `— question still pending` when the thread closed while its latest
  `status:*` tag was still `status:needs-decision`.
- The artifact is a path relative to the current directory (subjects are
  stored relative to the project root); there is no wildcard matching.
