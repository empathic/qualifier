# Verifier brief

You are verifying these qualifier findings: `{IDS}`. For each, read its
thread with `qualifier threads <id> --format json` (one thread per ID; read
`root.id` and `root.subject`) and check the claim against the current code.

## Verdict per finding

- **confirmed** — the claim holds. Evidence: the `file:line` that shows it,
  or a command and its output, or a failing test.
- **refuted** — the claim does not hold. Evidence of the same kind.

Write one reply per finding in a single batch (Write tool → file, then
`qualifier record --stdin --dry-run < <file>` and
`qualifier record --stdin < <file>`). `location` and `references` come from
the finding's own `root.subject`/`root.id` — full ID, no prefixes:

`{"kind": "comment", "location": "<root.subject>", "references": "<root.id>", "message": "confirmed: <one line>", "detail": "<evidence>", "tags": ["verified"]}`
`{"kind": "comment", "location": "<root.subject>", "references": "<root.id>", "message": "refuted: <one line>", "detail": "<evidence>", "tags": ["verified"]}`

Evidence must stand alone: repository paths and lines, not this brief. Do
not resolve anything yourself — the reviewing session resolves findings
you refute, with `--reason invalid`, after reading your reply.

## Final message

`<id> confirmed|refuted` per line, nothing else.
