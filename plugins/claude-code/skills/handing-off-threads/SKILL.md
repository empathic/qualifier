---
name: handing-off-threads
description: Use before ending a session or handing work to another agent or person — checks that open qualifier threads are self-sufficient and that nothing decided in this session lives only in chat
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*), Bash(git:*)
---

# Handing off threads

## Read as a stranger

List what this session touched. `--tag` only matches a thread's root or a
live reply, not the `resolve` that closed it, so a thread this session
resolved (but didn't open or reply to) won't show up in a tag-filtered
query — check both:

```bash
qualifier threads --all --format json --tag 'session:<value>'
qualifier threads --all --format json
```

`<value>` is this session's tag — usually `claude-code:<id>`, or whatever
`$QUALIFIER_SESSION` names if it was set; take it from any record you wrote
this session if you're unsure. From the second, unfiltered listing, also
count threads whose `closed_by.body.tags` include `session:<value>` — those
were resolved by this session and are missing from the first query.

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
