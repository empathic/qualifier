---
name: handing-off-threads
description: Use before ending a session or handing work to another agent or person — checks that open qualifier threads are self-sufficient and that nothing decided in this session lives only in chat
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*), Bash(git:*)
---

# Handing off threads

## Read as a stranger

List what this session touched — threads it opened, replied to, or
closed (under `--all`, `--tag` also matches the closing resolve):

```bash
qualifier threads --all --tag 'session:<value>' --format json
```

`<value>` is this session's tag — usually `claude-code:<id>`, or whatever
`$QUALIFIER_SESSION` names if it was set; take it from any record you wrote
this session if you're unsure. Don't list the whole project; this query is
the session's working set.

For each thread, ask: would someone who never saw this session know what to
do?

## Fix

Rewrite your own records (`qualifier reply <target> "…" --supersedes <id>`
or `record … --supersedes <id>`, where `<id>` is the full 64-character ID
of the live record from `qualifier threads --format json`) that:

- cite a brief, a prompt, "principle N", "as discussed", or a path outside
  the repository;
- cite record IDs that are now superseded — use the live ID;
- leave a resolution vague ("fix the locking") instead of saying where and
  how.

To comment on a thread this session closed, reply to the closing `resolve`
record — the reply joins the thread, which stays closed. To reopen it,
record a new non-reply record on the same subject whose `--supersedes` is
the closing record's full ID.

## Confirm

- `qual:closing-the-loop` has run for the last change.
- `qualifier threads --status needs-decision` lists anything the human
  still owes.

## Report

"Ready" or "Not ready: …", with the number of threads this session touched
and the IDs still waiting on a decision.
