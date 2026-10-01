# Verifier brief

You are verifying these qualifier findings: `{IDS}`. For each, read its
thread with `{QUALIFIER} threads <id> --format json` (one thread per ID;
read `root.id` and `root.subject`) and check the claim against the current
code.

## Verdict per finding

Judge both the claim and its severity:

- **confirmed** — the claim holds and its kind is right (a `blocker`'s
  `detail` states a concrete `Failure:`). Evidence: the `file:line` that
  shows it, or a command and its output, or a failing test.
- **refuted** — the claim does not hold. Evidence of the same kind.
- **downgraded** — the claim holds but the severity is too high (most
  often a `blocker` with no concrete failure a user would hit before
  merge — a risk or style problem, not a defect). Name the kind it should
  be instead.

Write one reply per finding in a single batch (Write tool → a file under
`{SCRATCH}` — never inside the repository — then
`{QUALIFIER} record --stdin --dry-run < <file>` and
`{QUALIFIER} record --stdin < <file>`). `location` and `references` come from
the finding's own `root.subject`/`root.id` — full ID, no prefixes. Each
reply carries exactly one verdict tag:

`{"kind": "comment", "location": "<root.subject>", "references": "<root.id>", "message": "confirmed: <one line>", "detail": "<evidence>", "tags": ["verified:confirmed"]}`
`{"kind": "comment", "location": "<root.subject>", "references": "<root.id>", "message": "refuted: <one line>", "detail": "<evidence>", "tags": ["verified:refuted"]}`
`{"kind": "comment", "location": "<root.subject>", "references": "<root.id>", "message": "downgraded: concern — <one line>", "detail": "<evidence>", "tags": ["verified:downgraded"]}`

Evidence must stand alone: repository paths and lines, command output,
tests, or public, stable documentation by URL — not this brief. Do not
resolve or supersede anything yourself — the reviewing session resolves
findings you refute, with `--reason invalid`, and re-records findings you
downgrade with the corrected kind, after reading your reply.

## Final message

`<id> confirmed|refuted|downgraded` per line, nothing else.
