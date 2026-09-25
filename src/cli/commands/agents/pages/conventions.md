+++
name = "conventions"
summary = "Tag vocabulary, the alternative kind, close authority, and self-sufficient records"
sees_also = ["threads", "batch", "workflows"]
since = "0.8.0"
+++

# qualifier — conventions

These conventions carry state that the record format does not model as
fields. Tools and agents rely on the exact spellings.

## Tags

| tag | meaning |
|---|---|
| `status:needs-decision` | Waiting on a human judgment call. Address it with `status:needs-decision:<issuer>`. |
| `status:decided` | A human made the call; the record carrying this tag states it. |
| `status:deferred` | Acknowledged; intentionally not now. |
| `reason:fixed` `reason:wontfix` `reason:duplicate` `reason:invalid` `reason:obsolete` | Why a `resolve` closed its target. `qualifier resolve --reason <r>` adds it. |
| `session:<harness>:<id>` | The session whose reasoning produced the record. Added automatically (see `qualifier agents concepts`, issuer defaults). |
| `revisit:<condition>` | On an `alternative`: when to reconsider it. |
| `depends-on:<id>` | On a reply in thread B: B cannot land before the thread whose root has this full ID. List with `qualifier threads --tag 'depends-on:*'`. |

A thread's status is its *latest* `status:*` tag. Find open decisions with
`qualifier threads --status needs-decision`.

## Kinds for design work

- A critique → `concern`. A defect that must be fixed → `blocker`. An idea →
  `suggestion`. An accepted risk → `waiver`.
- A path considered and not taken → `alternative` (a custom kind). Put what
  it was in the summary, why not now in `--detail`, and an observable
  trigger in a `revisit:` tag:

```bash
qualifier record alternative docs/design.md:40:52 "Per-session worker pool" \
  --detail "One pool is simpler until sessions exceed ~1k" \
  --tag "revisit:concurrent sessions exceed 1000"
```

## Close authority

Resolve a record only when (a) your session issued it (subagents share the
session), or (b) your own commit fixed it — and then only with evidence in
the message (a test that now passes, or a command and its output) and
`--ref git:<sha>`. Commit the `.qual` change separately: a resolve cannot
live in the commit it references. Without evidence, reply with what you
found and let a human close it. `wontfix` and decisions belong to humans.

## Self-sufficient records

A record must make sense to someone who never saw the session that wrote
it. Cite repository paths, line ranges, and record IDs. Never cite a
prompt, a brief, "principle 3", "as discussed", or files outside the
repository.
