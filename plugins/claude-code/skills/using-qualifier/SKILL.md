---
name: using-qualifier
description: Use in any repository that contains .qual files — maps which qualifier skill applies at each point in design, planning, implementation, review, and handoff, and sets the bar for what is worth recording
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*)
---

# Using qualifier

This repository keeps quality annotations in `.qual` files next to the
code. Treat them as the durable record of critiques, findings, decisions,
and paths not taken — not chat, not PR comments, not TODO comments.

## Which skill, when

| moment | skill | alongside (superpowers) |
|---|---|---|
| An option is rejected, a risk accepted, or a spec critiqued | `qual:recording-design-decisions` | brainstorming |
| Writing a plan from a spec or from open threads | `qual:planning-from-threads` | writing-plans |
| About to modify a file | `qual:consulting-threads` | executing-plans, test-driven-development |
| A change is complete, before commit or "done" | `qual:closing-the-loop` | verification-before-completion |
| Reviewing code or a design | `qual:reviewing-into-qualifier` | requesting-code-review |
| Working through existing threads or review feedback | `qual:triaging-threads` | receiving-code-review |
| A thread needs a human judgment call | `qual:escalating-decisions` | — |
| Ending a session or handing work over | `qual:handing-off-threads` | finishing-a-development-branch |

Invoke the matching skill with the Skill tool when its moment arrives.

## The bar

Record only what the next person to touch this code would want to know.
Not scratch notes, not what belongs in the commit message, not project-wide
policy (that goes in `docs/`). Before recording, run
`qualifier threads <path>` — if a thread on the same lines already says it,
reply there instead of opening a near-duplicate.

## Kinds

Critique → `concern`. Defect that must be fixed → `blocker`. Idea →
`suggestion`. Path not taken → `alternative` with a `revisit:` tag. Accepted
risk → `waiver`. Tags, reasons, and statuses: `qualifier agents conventions`.

## Close authority

Resolve a record only when your session issued it (subagents share the
session), or when your own commit fixed it — and then only with evidence in
the message (a test that now passes, or a command and its output) and
`--ref git:<sha>`. Without evidence, reply with what you found and let the
human close it. Propose every other close to the human. `wontfix` and design
decisions belong to humans.

## Self-sufficiency

Every record must make sense to someone who never saw this session. Cite
repository paths, line ranges, and record IDs. Never cite a prompt, a
brief, "principle N", "as discussed", or anything outside the repository.

## Mechanics

- Target the live record. `reply` and `resolve` refuse superseded targets
  and name the live one — retarget, don't reach for `--allow-superseded`.
- More than two writes: one `record --stdin` batch, written to a file with
  the Write tool, dry-run first (`qualifier agents batch`).
- Leave `--issuer` and `--issuer-type` unset; qualifier marks records from
  this session as `ai` and tags them with the session. Override only in
  `qual:escalating-decisions`.
- If the session context says qualifier is not on PATH, call the wrapper it
  names wherever these skills say `qualifier`.
