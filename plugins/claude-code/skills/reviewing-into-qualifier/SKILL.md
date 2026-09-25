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

A verification is a reply, `confirmed` or `refuted`, with evidence: a
`file:line`, a command and its output, or a test. Refuted findings you
issued in this session are resolved `--reason invalid`.

## Never

- Put a finding only in chat or a PR comment.
- Give subagents numbered principles or other context a finding could cite.
  Everything a finding cites must be in the repository.

## Report

"Recorded N findings (B blockers, C concerns, S suggestions); V confirmed,
R refuted. List: `qualifier threads --tag review:<that>`." (`--tag review`
alone lists every review this project has ever recorded, not just this
one.)

Next: `qual:triaging-threads`.
