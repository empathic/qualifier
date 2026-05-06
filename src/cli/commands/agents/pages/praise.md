+++
name = "praise"
summary = "Show who annotated an artifact and why"
sees_also = ["show"]
since = "0.5.0"
+++

# qualifier praise

## Purpose

Show who annotated an artifact and why, with authorship detail for each
active record.

## When to use it

`praise` is the attribution command. Where `show` presents a threaded view
of open quality signals, `praise` focuses on who left each annotation, when,
and with what issuer type — making it easy to see whether a review was done
by a human, an AI agent, or a tool. It is useful when auditing the annotation
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

**`--format json`** returns a JSON object with `subject` and a `records`
array. Each entry includes `id`, `kind`, `summary`, `issuer`,
`created_at`, and optionally `issuer_type`, `detail`, `suggested_fix`, and
`span`. This is the right mode for an agent that needs to programmatically
find who to notify or which annotations came from other agents.

**`--vcs`** delegates to the VCS `blame` / `annotate` command on the `.qual`
file itself (git or hg), showing which VCS commit last touched each line.
This is complementary to the record-based view: `--vcs` shows commit
authorship at the `.qual` file level, not the annotation-level issuer field.
It requires a supported VCS to be detected.

## Gotchas

- `praise` only shows **active** records — superseded and resolved
  annotations are filtered out. If you need the full history, use
  `qualifier show --all`.
- The `issuer` field is a URI (e.g., `mailto:alice@example.com`). The human
  output strips the `mailto:` prefix and the domain to show a short name;
  the JSON output always includes the full URI.
- `--vcs` requires git or hg. For other VCS systems the command exits with
  an error and suggests running your VCS tool directly on the `.qual` file.
- `praise` exits with an error if no records are found for the artifact, the
  same as `show`.
