# qualifier reply

## Purpose

Add a follow-up annotation to an existing record, continuing a thread.

## When to use it

Use `reply` when you want to comment on or respond to an existing annotation
without closing it. The new record is linked to the target via
`body.references` — `qualifier show` will render replies indented under their
parent. This is distinct from `resolve`, which closes an annotation by
writing a record with `kind: resolve` and `body.supersedes` pointing at the
target. If you only want to add context, a correction, or acknowledge
something that still needs action, use `reply`. If the issue has been fixed
and you want it to stop appearing in active annotation lists, use `resolve`.

## Common invocations

```bash
# Reply to a record by id-prefix (4+ characters required)
qualifier reply a1b2c3d4 "Confirmed — also affects the logout path"

# Reply to the most-recent active annotation at a location
qualifier reply src/auth.rs:42 "Fixed in PR #88 but needs backport to v2"

# Reply with a suggested fix and ai issuer-type
qualifier reply a1b2c3d4 "Here is a safer pattern" \
  --suggested-fix "Use constant-time comparison: crypto.timingSafeEqual(a, b)" \
  --issuer-type ai

# Reply with a non-default kind
qualifier reply src/auth.rs "This is intentional per security policy" \
  --kind waiver
```

## Flags worth knowing

**`<target>`** accepts either an id-prefix (at least 4 hex characters) or a
location string like `src/auth.rs` or `src/auth.rs:42`. The id-prefix form
matches any record in the project whose ID starts with those characters — if
more than one record matches, the command fails with a list of candidates so
you can narrow it. The location form resolves to the most-recent active record
at that location; if multiple records share the newest timestamp, the command
also fails with a disambiguation list.

**`--kind <KIND>`** overrides the default kind of `comment`. Any standard
kind (`concern`, `suggestion`, `waiver`, etc.) or custom string is accepted.
This lets a reply carry semantic weight — for example, a `waiver` reply is
meaningful to tools that consume the annotation graph.

**`--format json`** prints the emitted record as JSON on stdout, which is
useful when you need to capture the new record's ID for a subsequent
`resolve` or `reply`.

## Gotchas

- A reply does **not** close the parent record. To close, use `resolve`.
- The `body.references` field is set automatically to the target record's
  full ID — you do not pass `--references` yourself.
- Location resolution only considers **active** records (those not superseded
  by a later annotation). If the record at a location has already been
  resolved, the location will return "no active record" rather than the
  resolved one. Use an id-prefix instead if you need to target a closed record.
- The minimum id-prefix length is 4 characters. Passing 3 or fewer produces a
  validation error.
