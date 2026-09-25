+++
name = "batch"
summary = "Write many records at once: one new record per stdin line"
sees_also = ["record", "reply", "resolve", "threads"]
since = "0.8.0"
+++

# qualifier — batch writes

Use `qualifier record --stdin` for more than a couple of writes. Write the
lines to a file (never quote long text through the shell), dry-run, then
run.

## Line shapes

Every line describes one new record, in the same shape as
`qualifier record`. There are no reply or resolve verbs: a reply is a record
whose `references` is the target's ID, and a resolve is a `kind: "resolve"`
record whose `supersedes` is the target's ID.

Get targets from `qualifier threads --format json`: each thread's
`root.id` is the live record to point at, and `root.subject` is its
location.

```json
{"kind": "concern", "location": "src/net.rs:10:20", "message": "…", "detail": "…", "tags": ["review"]}
{"kind": "comment", "location": "<root.subject>", "references": "<root.id>", "message": "confirmed — see tests/net.rs:44"}
{"kind": "resolve", "location": "<root.subject>", "supersedes": "<root.id>", "message": "fixed by retry budget", "tags": ["reason:fixed"], "ref": "git:abc1234"}
```

- `supersedes` / `references` take a **full** 64-character record ID — no
  prefixes, no locations. The record must exist (on disk or on an earlier
  line of the same batch) and be live: not superseded and not closed. A
  stale pointer fails and prints the full ID of the live record, or of the
  `resolve` that closed it.
- Only complete-envelope lines (`subject` + `body`) have IDs known in
  advance; an overrides line is stamped when it is planned. In practice an
  in-batch pointer names an envelope line; everything else comes from
  `threads --format json`, `show --format json`, or the `id:` line a write
  command prints.
- A `kind: "resolve"` line carries at most one `reason:*` tag, one of
  `fixed`, `wontfix`, `duplicate`, `invalid`, `obsolete`.
- To comment on a closed thread, reference its `closed_by.id` (from
  `threads --all --format json`). To reopen one, record a non-reply line
  with the same location that supersedes `closed_by.id`.
- `location` is relative to the current directory, like the single
  commands.

## Semantics

```bash
qualifier record --stdin --dry-run --format json < writes.jsonl   # validate, write nothing
qualifier record --stdin < writes.jsonl                            # all-or-nothing
```

Without `--continue-on-error`, every line is validated first, every
failure is reported as `stdin line N: …`, and nothing is written if any
line fails. With `--continue-on-error`, valid lines are written and the
exit code is non-zero if any failed.
