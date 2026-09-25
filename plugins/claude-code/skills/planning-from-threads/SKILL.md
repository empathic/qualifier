---
name: planning-from-threads
description: Use when writing an implementation plan from a spec or from open qualifier threads — every plan task names the thread IDs it addresses, and ordering constraints are recorded as references rather than only in prose
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*)
---

# Planning from threads

## Input

`qualifier threads --format json` scoped to the work: a path, a glob, or
`--tag`. Each object's `root.id` is the live record to cite.

## Shape the plan

- Group threads into workstreams by subsystem.
- Order: `blocker`, then `concern`, then `suggestion`; within that, by
  dependency.
- Every task lists the IDs (8-character prefixes) of the threads it
  closes, e.g. "Closes e52b00e4, 7c1d93aa."
- Threads you are deliberately not doing: say so in the plan with the ID.

## Record ordering constraints

When thread B cannot land before thread A, reply on B:

```json
{"reply": "<B prefix>", "message": "Depends on <A prefix>: <one-line reason>", "tags": ["depends-on:<full ID of A's root>"]}
```

A reply's `references` already points into B, so the tag carries the edge.
List them with `qualifier threads --tag 'depends-on:*'`. The plan may repeat
the dependency; the record is authoritative because it survives the plan
document.

Next: implementation, where `qual:consulting-threads` applies before each
file is edited.
