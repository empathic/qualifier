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

All writes in one batch: Write tool → `triage.jsonl`, then
`qualifier record --stdin --dry-run < triage.jsonl`, then the real run.

```json
{"resolve": "e52b00e4", "message": "Retry budget added in 4f1c2aa", "reason": "fixed", "ref": "git:4f1c2aa"}
{"reply": "7c1d93aa", "message": "Resolution: move the lock into ConnPool::get (src/pool.rs:88) so callers can't forget it"}
```

If a target was superseded, the batch fails naming the live record —
retarget to it.

Next: `qual:escalating-decisions` for anything left needing a decision,
then `qual:planning-from-threads`.
