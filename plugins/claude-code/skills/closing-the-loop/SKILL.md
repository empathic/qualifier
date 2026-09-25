---
name: closing-the-loop
description: Use when a code change is complete, before committing or claiming the work is done — resolves threads the change fixed, records shortcuts as concerns instead of TODO comments, re-records drifted annotations, and records design decisions settled in conversation
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*), Bash(git:*)
---

# Closing the loop

Run this checklist before you say the work is done.

1. **Threads this change fixed.** After committing the fix, resolve each
   one it fixed, citing evidence — a test that now passes, or a command and
   its output:
   `qualifier resolve <id> "<evidence>" --reason fixed --ref git:<sha>`.
   Then commit the `.qual` change on its own; a resolve cannot live in the
   commit it references. Only threads within close authority
   (`qualifier agents conventions`); without evidence, or for others' threads,
   reply with the commit and let a human close.
2. **Shortcuts you took.** Record each as a `concern` on its lines, with
   what a complete version would do. Do not leave `TODO` comments instead.
3. **Drift.** Run `qualifier review` (or `qualifier review <file>` once per
   touched file, repo-relative — `review` takes at most one subject, and a
   directory or `./`-prefixed path matches nothing). For each `drifted`
   location, find its record with `qualifier threads <location> --format json`
   (the thread whose root span is that location) and read `root.id`.
   Re-anchor it on the new lines with
   `qualifier record <kind> <path>:<start>:<end> "<same summary>" --supersedes <root.id>`,
   using that full ID from `threads --format json` — never a prefix,
   keeping the original kind, summary, `--detail`, and `--suggested-fix`
   exactly as they were, and passing every original tag again with `--tag`
   except `session:*` (the new session tag is added automatically) — a
   dropped `revisit:`, `review:`, or `status:` tag silently drops the thread
   out of the queries that depend on it. If the original record's issuer is
   not this session's, add a line to `--detail` naming the original issuer
   and stating the record was re-anchored unchanged — re-anchoring a
   drifted span is allowed; rewording someone else's finding is not. If it
   no longer applies because you fixed it, step 1 covers it.
4. **Decisions made in chat.** Anything the user and you settled in this
   session that is not in a record: reply on the relevant thread, or record
   an `alternative`/`waiver` on the spec (`qual:recording-design-decisions`).

More than two writes: one `record --stdin` batch (`qualifier agents batch`).

Next: `qual:handing-off-threads` if the session is ending.
