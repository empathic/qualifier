+++
name = "resolve"
summary = "Resolve (close) a record"
sees_also = ["reply", "record"]
since = "0.5.0"
+++

# qualifier resolve

## Purpose

Close an existing record by emitting a `resolve`-kind annotation that
supersedes it.

## When to use it

`resolve` is a user-directed action. As an agent, do not invoke `resolve`
unless the user has explicitly told you to close a specific annotation.
Closing a record hides it from `qualifier show`, `qualifier ls`, and other
active views; if you close a concern you do not fully understand, you
silently bury something the user may still want to act on. When you
encounter an annotation that *might* be addressable, surface it to the
user (e.g., quote it, or print `qualifier show <artifact>` for them) and
let them decide.

When the user does direct you to resolve, this is what the command does:
it writes a new annotation with `kind: resolve` and `body.supersedes`
pointing at the target. There is no deletion; the full history is
preserved in the `.qual` file. If you instead want to *add context* to a
record (a comment, a status update, an acknowledgment that work remains),
use `reply` — `reply` does not close anything.

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
chars) or a location string. Locations are relative to the current
directory; subjects are stored relative to the project root. A location
never resolves to a `resolve` record. An id-prefix that matches more than one record,
or a location with multiple tied active records, surfaces a disambiguation
list and exits without writing.

**`[message]`** is optional. When omitted, the resolution record's summary is
set to `"Resolved"`. Provide a message when the reason for closure carries
useful context — the message ends up in the annotation history visible to
`praise` and `show --all`.

**`--ref <VCS-REF>`** pins a VCS commit reference (e.g., `git:3aba500`) to
the resolution record. This is useful when you want reviewers to be able to
jump to the exact commit that addressed the issue.

**`--issuer` / `--issuer-type`**: as an agent, leave `--issuer` and `--issuer-type` unset; see `qualifier agents concepts` for
defaults (`QUALIFIER_*` variables, agent-harness detection). The
defaults already mark a resolution written from an agent session as `ai`,
so the user can review what their agent closed.

**`--reason fixed|wontfix|duplicate|invalid|obsolete`** adds the tag
`reason:<value>`. A resolve carries at most one `reason:*` tag, and its
value must be one of these; `resolve` rejects anything else. It is a tag
convention (§2.12), not a body field.

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

## Superseded targets

If the target has been superseded, the command fails and names the live
record — target (resolve) that one instead. If the target was resolved, the command
reports it as closed. `--allow-superseded` overrides both checks; use it
only to add context to history.
