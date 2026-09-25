---
name: handing-off-threads
description: Use before ending a session or handing work to another agent or person — checks that open qualifier threads are self-sufficient and that nothing decided in this session lives only in chat
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*), Bash(git:*)
---

# Handing off threads

## Read as a stranger

List what this session touched:

```bash
qualifier threads --all --format json --tag 'session:claude-code:${CLAUDE_SESSION_ID}'
```

If `${CLAUDE_SESSION_ID}` above was not replaced with an ID, take the
`session:claude-code:…` tag from any record you wrote this session.

For each thread, ask: would someone who never saw this session know what to
do?

## Fix

Rewrite your own records (`qualifier reply <target> "…" --supersedes <prefix>`
or `record … --supersedes <prefix>`) that:

- cite a brief, a prompt, "principle N", "as discussed", or a path outside
  the repository;
- cite record IDs that are now superseded — use the live ID;
- leave a resolution vague ("fix the locking") instead of saying where and
  how.

## Confirm

- `qual:closing-the-loop` has run for the last change.
- `qualifier threads --status needs-decision` lists anything the human
  still owes.

## Report

"Ready" or "Not ready: …", with the number of threads this session touched
and the IDs still waiting on a decision.
