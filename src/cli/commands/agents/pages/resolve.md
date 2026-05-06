# qualifier resolve

## Purpose

Close an existing record by emitting a `resolve`-kind annotation that
supersedes it.

## When to use it

Use `resolve` after you have fixed the issue described in an annotation and
want it removed from active annotation lists. Under the hood, `resolve` writes
a new annotation with `kind: resolve` and `body.supersedes` pointing at the
target — there is no deletion; the full history is preserved in the `.qual`
file. This is the right command when an issue is actually done. If you just
want to add context, a comment, or acknowledge that work remains, use `reply`
instead; `reply` does not close anything.

## Common invocations

```bash
# Resolve a record by id-prefix with the default message "Resolved"
qualifier resolve a1b2c3d4

# Resolve with a descriptive message
qualifier resolve a1b2c3d4 "Fixed in commit 3aba500 — added null check before session reset"

# Resolve the most-recent active annotation at a location
qualifier resolve src/auth.rs:42 "Rate limit added in middleware"

# Resolve and emit the result as JSON (to capture the new record's ID)
qualifier resolve a1b2c3d4 "Done" --format json
```

## Flags worth knowing

**`<target>`** follows the same resolution rules as `reply`: an id-prefix (4+
chars) or a location string. An id-prefix that matches more than one record,
or a location with multiple tied active records, surfaces a disambiguation
list and exits without writing.

**`[message]`** is optional. When omitted, the resolution record's summary is
set to `"Resolved"`. Provide a message when the reason for closure carries
useful context — the message ends up in the annotation history visible to
`praise` and `show --all`.

**`--ref <VCS-REF>`** pins a VCS commit reference (e.g., `git:3aba500`) to
the resolution record. This is useful when you want reviewers to be able to
jump to the exact commit that addressed the issue.

**`--issuer-type ai`** should be set whenever an agent is the one closing a
record, so the resolution is attributable to a machine rather than a human.

## Gotchas

- `resolve` writes a *new* record with `body.supersedes`; it does not modify
  or delete the original. `qualifier show` hides the resolved record from its
  default view, but `qualifier show --all` still shows it.
- Cross-subject supersession is rejected: the target and the new resolve
  record must share the same subject.
- If the target was already resolved (already superseded), the supersession
  cycle check may reject the new record. Inspect with
  `qualifier show --all <artifact>` first.
- The minimum id-prefix is 4 characters, same as `reply`.
