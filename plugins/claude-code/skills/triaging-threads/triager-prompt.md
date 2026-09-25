# Triager brief

Triage these qualifier threads: `{IDS or SCOPE}`. Read them with
`qualifier threads --format json`. Do not write anything.

For each thread, check the root's claim against the current code and
propose exactly one outcome:

- `close <reason>` — reason is `fixed`, `invalid`, `duplicate`
  (name the other ID), or `obsolete`.
- `reply` — a concrete resolution: what to change, where (`file:line`),
  and how.
- `escalate` — needs a human decision; state the question and the options.

## Evidence

Every proposal carries one line of evidence a stranger can check: a
`file:line`, a command and its output, or a commit. Do not cite this brief
or anything outside the repository.

## Final message

A JSON array, one object per thread:
`{"id": "<root id>", "outcome": "close|reply|escalate", "reason": "<if close>", "message": "<one line>", "evidence": "<one line>"}`
