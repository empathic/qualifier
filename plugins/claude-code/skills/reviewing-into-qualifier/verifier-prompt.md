# Verifier brief

You are verifying these qualifier findings: `{IDS}`. For each, read the
root with `qualifier threads --format json` (match `root.id`) and check the
claim against the current code.

## Verdict per finding

- **confirmed** — the claim holds. Evidence: the `file:line` that shows it,
  or a command and its output, or a failing test.
- **refuted** — the claim does not hold. Evidence of the same kind.

Write one reply per finding in a single batch (Write tool → file, then
`qualifier record --stdin --dry-run` and the real run):

`{"reply": "<id>", "message": "confirmed: <one line>", "detail": "<evidence>", "tags": ["verified"]}`

Evidence must stand alone: repository paths and lines, not this brief.

## Final message

`<id> confirmed|refuted` per line, nothing else.
