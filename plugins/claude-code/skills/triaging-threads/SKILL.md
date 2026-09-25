---
name: triaging-threads
description: Use when working through existing qualifier threads or review feedback — checks each claim against the current code, proposes closes with reasons as one batch for the human to approve, and replies with concrete resolutions for the rest
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*), Bash(git:*)
---

# Triaging threads

## Input

`qualifier threads --format json` scoped as asked (path, glob, `--kind`,
`--tag`). Reply to or resolve `root.id`, never `origin`.

## For each thread, one outcome

1. **Close** — the claim is fixed, wrong, a duplicate, or obsolete.
   Choose a reason: `fixed`, `invalid`, `duplicate`, `obsolete`.
   (`wontfix` is a human's call — escalate instead.)
2. **Reply** with a concrete resolution: what should change, where, and
   how, so an implementer can act without this session.
3. **Escalate** — needs a human judgment call →
   `qual:escalating-decisions`.

Check every claim against the code. For large sets, dispatch subagents with
`triager-prompt.md`, one subsystem each.

## Approval

Closes outside close authority (`qualifier agents conventions`) go to the
human as one table before anything is written:

| ID | summary | proposed | evidence |
|---|---|---|---|

Apply only the rows they approve.

## Writing

All writes in one batch, written outside the working tree so it can't be
committed by accident: Write tool → `<scratch>/triage.jsonl`, where
`<scratch>` is your scratch directory or a `mktemp -d` directory, then
`qualifier record --stdin --dry-run < <scratch>/triage.jsonl`, then the
real run.

```json
{"kind": "resolve", "location": "<root.subject>", "supersedes": "<root.id>", "message": "cargo test net::retry passes; retry budget added in 4f1c2aa", "tags": ["reason:fixed"], "ref": "git:4f1c2aa"}
{"kind": "comment", "location": "<root.subject>", "references": "<root.id>", "message": "Resolution: move the lock into ConnPool::get (src/pool.rs:88) so callers can't forget it"}
```

`supersedes` and `references` take the thread's full `root.id`, exactly as
`qualifier threads --format json` gives it — no prefixes, no locations.

A `fixed` close needs evidence in the message (a test that now passes, or a
command and its output) and a `"ref"` naming the commit; commit the `.qual`
change separately from the fix it references.

If a target was superseded, the batch fails naming the live record —
retarget to it. If it was already closed, the batch names the closing
`resolve` record; comment by targeting that record instead, or reopen the
thread with a new non-reply line whose `supersedes` is its ID.

Next: `qual:escalating-decisions` for anything left needing a decision,
then `qual:planning-from-threads`.
