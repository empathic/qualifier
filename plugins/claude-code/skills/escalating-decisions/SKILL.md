---
name: escalating-decisions
description: Use when a qualifier thread needs a human judgment call — design trade-offs, won't-fix calls, or conflicting findings — to mark it as waiting on a decision, walk the human through the options, and record their answer under their own identity
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*), Bash(git:*)
---

# Escalating decisions

## Mark

Reply on the thread with the question and the options:

`{"reply": "<id>", "message": "Needs a decision: <question>", "detail": "Option A: … (consequence). Option B: … (consequence). Recommendation: …", "tags": ["status:needs-decision"]}`

Address a specific person with `status:needs-decision:<their issuer>`.
Pending decisions: `qualifier threads --status needs-decision`.

## Walk

One thread at a time: the question, each option with its consequence, and
your recommendation with its reason. Wait for the answer.

## Record

Record a decision only after the human states it in this session.
Agreement with a recommendation you showed them counts; your inference does
not.

If the user relays someone else's decision, don't write under that person's
identity. Reply as yourself (issuer flags unset) naming who decided and
when, tagged `status:needs-decision:<their issuer>`, so they confirm it.

Once per session, confirm their identity: read `git config user.email` and
ask "Record decisions as mailto:<email>?".

Then reply with their decision under their identity:

`{"reply": "<id>", "message": "Decided: <the decision>", "detail": "<their words, verbatim>", "issuer": "mailto:<email>", "issuer_type": "human", "tags": ["status:decided"]}`

The session tag is still added, which links the record to the session it
was made in. If they decide `wontfix`, resolve with `--reason wontfix`
under their identity in the same way.

Next: `qual:triaging-threads` or `qual:planning-from-threads`.
