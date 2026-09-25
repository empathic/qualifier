---
name: recording-design-decisions
description: Use during design or brainstorming when an option is considered and rejected, a risk is knowingly accepted, or a spec or design document is critiqued — records each as a qualifier annotation on the spec so the reasoning outlives the conversation
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*)
---

# Recording design decisions

Design conversations end; the reasons behind the design shouldn't. Record
them on the spec's lines once the spec file exists.

## What to record

- **Rejected option** → `alternative` on the spec lines where the chosen
  design lives. Summary: what the option was. `detail`: why not now.
  Tag `revisit:<condition>` naming something observable — "concurrent
  sessions exceed 1000", "the channels API leaves preview" — never
  "later" or "if needed".
- **Accepted risk** → `waiver` on the lines that accept it, with the risk
  and why it is acceptable now.
- **Critique of the spec** (yours or the user's) → `concern` on the
  spec's line range. The spec's author answers in the thread.

## When

After the spec is written and its line ranges are stable — not
mid-conversation. Collect the decisions from the discussion, then write
them as one batch:

```json
{"kind": "alternative", "location": "docs/specs/cache.md:40:52", "message": "Per-tenant cache processes", "detail": "One shared process is simpler below ~50 tenants", "tags": ["revisit:tenants exceed 50"]}
{"kind": "waiver", "location": "docs/specs/cache.md:60:64", "message": "No cache warm-up on deploy", "detail": "Cold starts cost ~2s p99 for 1 minute; acceptable for internal tools"}
```

Write that to a file with the Write tool, then
`qualifier record --stdin --dry-run < file` and `qualifier record --stdin < file`.
Line shapes: `qualifier agents batch`.

Spans drift as the spec is edited. That is expected: `qual:closing-the-loop`
re-anchors drifted records with `--supersedes` on the new lines rather than
dropping the span.

## Bar

A rejected option is worth recording when someone would plausibly propose
it again. Skip options nobody would revisit.

Next: `qual:planning-from-threads`.
