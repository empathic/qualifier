+++
name = "batch"
summary = "Write many records at once: record, reply, and resolve lines on stdin"
sees_also = ["record", "reply", "resolve", "threads"]
since = "0.8.0"
+++

# qualifier — batch writes

Use `qualifier record --stdin` for more than a couple of writes. Write the
lines to a file (never quote long text through the shell), dry-run, then
run.

## Line shapes

```json
{"kind": "concern", "location": "src/net.rs:10:20", "message": "…", "detail": "…", "tags": ["review"]}
{"reply": "1a2b3c4d", "message": "confirmed — see tests/net.rs:44", "kind": "comment"}
{"resolve": "src/net.rs:10", "message": "fixed by retry budget", "reason": "fixed", "ref": "git:abc1234"}
```

- `reply` / `resolve` take an ID prefix (≥ 4 characters) or a location,
  resolved exactly like the single commands — including records created
  earlier in the same batch.
- Superseded or closed targets are rejected; add `"allow_superseded": true`
  only to annotate history.
- `supersedes` / `references` accept ID prefixes.

## Semantics

```bash
qualifier record --stdin --dry-run --format json < writes.jsonl   # resolve + validate, write nothing
qualifier record --stdin < writes.jsonl                            # all-or-nothing
```

Without `--continue-on-error`, every line is resolved and validated first,
every failure is reported as `stdin line N: …`, and nothing is written if
any line fails. With `--continue-on-error`, valid lines are written and the
exit code is non-zero if any failed.
