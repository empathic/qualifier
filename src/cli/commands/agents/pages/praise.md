+++
name = "praise"
summary = "Show who annotated an artifact and why"
sees_also = ["show"]
since = "0.5.0"
+++

# qualifier praise

## Purpose

Show who annotated an artifact and why, thread by thread, with authorship
detail for each record shown.

## When to use it

`praise` is the attribution command. It renders threads with the same model
and renderer as `show` and `threads` (open threads with their live replies;
closed threads as one line carrying the resolve that closed them, and who
closed it), and adds each record's suggested fix or detail beneath it. Each
record line ends in `(issuer, issuer type, date)`, the type omitted for
`human` — making it easy to see whether a review was done by a human, an AI
agent, or a tool. It is useful when auditing the annotation
history of a file or when you need to find the contact for an annotation
before replying. The alias `blame` also works, but the CLI will print a hint
suggesting `praise` — the format is designed to surface helpers rather than
assign fault.

## Common invocations

```bash
# Show attribution for all active annotations on a file
qualifier praise src/auth.rs

# Machine-readable JSON output (includes issuer_type, span, detail)
qualifier praise src/auth.rs --format json

# Invoke via the alias (produces a hint, then runs normally)
qualifier blame src/auth.rs

# Use VCS blame on the underlying .qual file instead
qualifier praise src/auth.rs --vcs
```

## Flags worth knowing

**`--format json`** returns a JSON object with `subject`, a `records`
array, and a `threads` array. `records` holds each thread's root, live
replies, and closing resolve, then the artifact's live epochs, in file
order. Each entry includes `id`, `kind`, `summary`, `issuer`,
`created_at`, and optionally `issuer_type`, `detail`, `suggested_fix`, and
`span`. `threads` has one entry per thread,
`{"origin", "root": "<id>", "state": {...}, "closed_by": "<id>"|null}`, with
`state` as in `qualifier threads --format json`. This is the right mode for
an agent that needs to programmatically find who to notify or which
annotations came from other agents.

**`--vcs`** delegates to the VCS `blame` / `annotate` command on the `.qual`
file itself (git or hg), showing which VCS commit last touched each line.
This is complementary to the record-based view: `--vcs` shows commit
authorship at the `.qual` file level, not the annotation-level issuer field.
It requires a supported VCS to be detected.

## Gotchas

- `praise` leaves out superseded records and edit history, and shows a
  closed thread only as its root and closing resolve. For the full history,
  use `qualifier show --all`, or `qualifier threads --all <id>` for one
  closed thread's replies.
- The `issuer` field is a URI (e.g., `mailto:alice@example.com`). The human
  output strips the `mailto:` prefix and the domain to show a short name;
  the JSON output always includes the full URI.
- `--vcs` requires git or hg. For other VCS systems the command exits with
  an error and suggests running your VCS tool directly on the `.qual` file.
- An artifact with no records is not an error, the same as `show`: `praise`
  exits 0, printing `No records found for '<subject>'.` in human output, or
  empty `records` and `threads` arrays in JSON. (`--vcs` still fails when
  there is no `.qual` file to blame.)
