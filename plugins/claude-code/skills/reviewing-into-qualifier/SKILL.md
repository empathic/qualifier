---
name: reviewing-into-qualifier
description: Use when reviewing code or a design document, whether asked for a review or reviewing your own work before merge — writes every finding as a qualifier annotation instead of reporting it in chat, then verifies each finding against the code
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*), Bash(git:*)
---

# Reviewing into qualifier

Findings go into `.qual` files, never only into chat. Your chat report is
a count and the command that lists them.

## Review

- Small scope (a few files): review inline and write one batch.
- Large scope: split by subsystem and dispatch one subagent per part with
  `reviewer-prompt.md`, filling in the scope. Each returns only the IDs it
  wrote.
- Kinds and the bar: `qual:using-qualifier`; spellings:
  `qualifier agents conventions`. Tag every finding `review` and with a
  tag scoped to this review, e.g. `review:<branch-or-date>` — pick one
  before dispatching and give it to every reviewer subagent, so the report
  below can list just this review's findings.

## Verify

Every finding gets a verification reply before the review is done:

- Always dispatch a fresh verifier subagent with `verifier-prompt.md` —
  never the reviewer that wrote the findings, and not yourself if you
  reviewed inline.
- Large reviews: one verifier per batch of finding IDs.

A verification is a reply carrying exactly one verdict tag:
`verified:confirmed` (the claim holds and its kind is right),
`verified:refuted` (the claim does not hold), or `verified:downgraded`
(the claim holds but the severity is too high; the message names the kind
it should be, e.g. "downgraded: concern — …") — each with evidence: a
`file:line`, a command and its output, a test, or public, stable
documentation by URL. The verifier never resolves or supersedes anything.

## After verification

In order:

1. Resolve every `verified:refuted` finding, citing the refuting reply
   (the session issued these findings, so close authority allows it):
   `qualifier resolve <root.id> "<cite the refuting reply ID>" --reason invalid`.
2. Re-record every `verified:downgraded` finding with the corrected kind —
   same location, message, detail, suggested fix, and tags, superseding
   the original: `qualifier record <kind> <location> "<message>" --detail
   "<detail>" --suggested-fix "<fix>" --tag <tag> --supersedes <root.id>`.
3. Commit the review's own records — the `.qual` files this review created
   or changed — in their own commit on the branch under review, e.g.
   `chore(qual): record review <tag>`.

## Filling the brief

`reviewer-prompt.md` and `verifier-prompt.md` are complete templates for a
subagent that starts with none of this session's context. Fill every
placeholder before dispatch; never send a brief with an unfilled `{…}`:

- `{QUALIFIER}` — the exact command the session context gives for running
  qualifier (the plugin's wrapper `exec` form), copied verbatim. If you are
  running a qualifier you installed yourself, its path instead.
- `{SCRATCH}` — a directory outside the repository created for this run
  (e.g. `mktemp -d`), so a batch file can never land in the working tree.
- `{SCOPE}` — what to review: a path, glob, or description of the subsystem
  (`reviewer-prompt.md` only).
- `{TAG}` — the tag scoped to this review (`reviewer-prompt.md` only); the
  same one for every reviewer subagent in this review.
- `{IDS}` — the finding IDs this subagent verifies (`verifier-prompt.md`
  only).

## Never

- Put a finding only in chat or a PR comment.
- Give subagents numbered principles or other context a finding could cite.
  Everything a finding cites must be something a stranger can check too: the
  repository, or public, stable documentation by URL.

## Report

Counts come from queries, not by hand:

- Total and open, by kind: `qualifier threads --all --tag review:<tag> --format json`
  (each thread's `root.body.kind` and `open`), or the count line
  `qualifier threads --all --tag review:<tag>` prints ("N threads (M
  open)").
- Verdicts: `qualifier threads --tag verified:confirmed` and
  `qualifier threads --tag verified:downgraded` (those threads stay open,
  no `--all` needed); `qualifier threads --all --tag verified:refuted`
  (resolving a refuted finding closes its thread, so `--all` is required
  to still see it).

"Recorded N findings (B blockers, C concerns, S suggestions); V confirmed,
D downgraded, R refuted. List: `qualifier threads --all --tag review:<tag>`."
(`--tag review` alone lists every review this project has ever recorded,
not just this one.)

Next: `qual:triaging-threads`.
