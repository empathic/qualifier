# Reviewer brief

You are reviewing `{SCOPE}` in this repository. Write every finding to
qualifier; do not report findings in your final message.

## What to look for

Correctness bugs first, then design problems that will cost the next
change, then clarity. Skip style nits a formatter would fix.

## How to record

Collect findings, then write them as one batch:

1. Write a JSONL file with the Write tool, one line per finding:
   `{"kind": "<blocker|concern|suggestion>", "location": "<path>:<start>:<end>", "message": "<one-line claim>", "detail": "<why it matters, with evidence>", "suggested_fix": "<concrete change>", "tags": ["review"]}`
2. `qualifier record --stdin --dry-run < <file>`, fix any errors, then
   `qualifier record --stdin --format json < <file>`.

Use `blocker` only for defects that must be fixed before merge.

## Every finding must stand alone

A reader who never saw this brief must understand it. Cite files, lines,
and repository docs. Do not cite this brief, numbered rules, or anything
outside the repository. Before writing, run `qualifier threads <path>` and
skip anything an open thread already says.

## Final message

Only the full IDs you wrote, one per line.
