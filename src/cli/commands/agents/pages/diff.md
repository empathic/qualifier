+++
name = "diff"
summary = "Show records added, resolved, or drifted since a git ref"
sees_also = ["review", "show", "ls"]
since = "0.6.0"
+++

# qualifier diff

## Purpose

Compare the active set of records on this branch against a git ref (default
`main`). Use this before opening a PR to see what you've added, what you've
edited or re-anchored, what you've resolved, and whether any spans you didn't touch have drifted underneath
existing annotations.

## When to use it

- **Before opening a PR.** `qualifier diff main` summarizes the review trail
  you're proposing to merge. Paste the output into the PR description.
- **In CI.** Run `qualifier diff origin/main --format json` and gate the
  merge on the `drifted` array being empty (or on no new `blocker`-kind
  records).
- **As an end-of-session summary.** After an agent has recorded several
  findings, `qualifier diff HEAD` (against the merge-base) gives a clean
  list of what was authored.

## What it shows

Four sections:

1. **Added** — records active on `HEAD` whose `id` is not present at `<ref>`
   at all. Annotation records only; epoch and dependency records are not
   review signals.
2. **Changed** — threads open on both sides whose root was edited or
   re-anchored on this branch (`--supersedes` without resolving). Matched
   by thread origin, so several edits in a row still pair with the root at
   `<ref>`. Each row shows the new root and, under it, what it was at
   `<ref>`. These records appear here instead of under Added and Resolved.
3. **Resolved** — records active at `<ref>` that are no longer active on
   `HEAD`. When the record's thread is closed on `HEAD`, the row names the
   `resolve` that closed it, even when that resolve targets a later edit
   rather than the record itself. It also names every other record whose
   `supersedes` points at it (an edit, or a second resolve after merging
   branches that each closed it), or marks the record as *removed* if no
   successor was authored.
4. **Drifted** — records present at *both* refs whose `body.span.content_hash`
   no longer matches the file's current content. Drift on records that are
   freshly added on this branch is suppressed (you just authored them; their
   span IS the current code).

## Thread state

Rows carry their thread's state on `HEAD`, from the same thread model and
wording as `qualifier threads`, `show`, and `praise`, so a closed thread
never appears without the record that closed it:

- A reply (Added, Resolved, or Drifted) gets `on thread <root ID>, <state>`,
  for example `on thread 31ef1c78, closed (wontfix): Won't change b()
  (9c04aa12)`.
- A thread's root gets its state when there is something to say:
  `needs decision`, `needs decision from <issuer>`, `decided`, or
  `closed (<reason>): <closer summary>  (<closer ID>)`. The root of an
  open thread with no decision pending gets no state line.
- A thread that closed while its latest `status:*` tag was still
  `status:needs-decision` ends in `— question still pending`.

## Comparison point

By default, `<ref>` is resolved via `git merge-base HEAD <ref>` — the
diff is reckoned against the point where this branch forked. Records
that landed on `<ref>` *after* the branch forked count as "old" and do
not appear under Added. This matches what a PR is asking to introduce.

Pass `--from-tip` to compare against the literal tip of `<ref>` instead.
Useful for "is my branch in sync with the latest main."

## Common invocations

```bash
# What does this branch propose to merge?
qualifier diff

# Compare against an upstream branch (uses merge-base of HEAD with origin/main)
qualifier diff origin/main

# Compare against the tip — what's different right now, regardless of fork point
qualifier diff main --from-tip

# CI gate: fail the build if any blocker is added or any annotation drifted
qualifier diff origin/main --fail-on blocker --fail-on-drift

# Filter to one kind, or to AI-authored records only
qualifier diff main --kind blocker
qualifier diff main --issuer-type ai

# Pipe-friendly subject list
qualifier diff main --subjects-only | xargs qualifier show

# Machine-readable summary
qualifier diff origin/main --format json
```

## Filtering and CI gating

| Flag | Effect |
|------|--------|
| `--kind <K[,K...]>` | Show only records whose kind matches one of the comma-separated list. Applies to every section; a Changed row matches on its old or new kind. |
| `--issuer-type <T>` | Show only records whose issuer-type is `T` (`human`, `ai`, `tool`, `unknown`). |
| `--fail-on <K[,K...]>` | Exit non-zero if Added contains any record matching one of the listed kinds, or a Changed record's kind moved into the list (e.g. `concern` -> `blocker`). Re-anchoring or rewording an existing blocker does not trip it. The diff is still printed first. |
| `--fail-on-drift` | Exit non-zero if Drifted is non-empty. |
| `--subjects-only` | Print only the affected subjects, deduplicated and sorted, one per line. Suppresses all other output. |
| `--from-tip` | Compare against `<ref>`'s tip rather than the merge-base of HEAD with `<ref>`. |
| `--no-ignore` | Read `.qual` files that `.gitignore` or `.qualignore` would skip. Without it, the working tree's ignore rules apply to both sides, so a newly ignored path is not reported as removed. |

Kinds are matched exactly. A kind in `--kind` or `--fail-on` that is
neither built in nor carried by any record on either side (a typo such as
`blockers`) prints `qualifier diff: warning: kind 'blockers' matches no
known kind` on stderr; the command still runs, since custom kinds are legal.

`--fail-on` and `--fail-on-drift` compose: pass both for a stricter CI
gate. The diff body is always printed before the failure exit, so the
build log shows exactly which record triggered the failure.

## Output shape (human)

Captured with `COLUMNS=80`:

```
Comparing HEAD against merge-base of main (1849719)

Added on this branch (3)
  + concern    src/auth.rs:5:7  login() ignores the user argument  (8d78d25b)
  + comment    src/config.rs  (a96ca4bd)
      Fall back to defaults, or fail with a clear message?
      on thread a14f47cb, needs decision
  + suggestion src/parser.rs:2  (bfc40981)
      split on ',' allocates; return an iterator

Changed on this branch (1)
  * blocker    src/config.rs:2  (a14f47cb)
      unwrap on a missing config file panics at startup
      needs decision
      was concern src/config.rs:2 (f7113f7b)

Resolved on this branch (1)
  - concern    src/auth.rs:2  (60077cb4)
      Token comparison is not constant-time
      closed (fixed): constant-time compare landed in PR #142  (8fcfb3a0)

Drifted (1)
  ~ concern    src/parser.rs:5:7  (4713c174)
      depth() counts bytes, not nesting
        src/parser.rs:
          2 |     input.split(",").collect()
          3 | }
          4 |
        > 5 | pub fn depth(s: &str) -> usize {
        > 6 |     s.matches('(').count()
        > 7 | }
```

A row fits on one line (`marker KIND LOCATION  SUMMARY  (ID)`) only when
it has nothing else to show and fits the width. Otherwise the header
keeps the kind, location, and ID, and the summary and any further detail
(the thread state, the `was ...` line under Changed, each `resolved by` /
`superseded by` record under Resolved, the current code under Drifted) follow as
indented lines, each cut to the width with `…`. The width is `$COLUMNS`
when exported, else the terminal width, else 80.

## Output shape (json)

```json
{
  "ref": "main",
  "base": "<full SHA of the commit compared against>",
  "from_tip": false,
  "comparison": "merge-base",
  "added":    [<full record envelopes>],
  "changed":  [{"record": <head-side root>, "previous": <ref-side root>}],
  "resolved": [{"record": <ref-side record>, "closer": <newest head-side closer or null>, "closers": [<every head-side closer, oldest first>]}],
  "drifted":  [{"record": <head-side record>, "expected": "<hash>", "actual": "<hash>"}],
  "threads":  [{"origin": "<id>", "root": "<id>", "state": {...}, "closed_by": "<id>"|null, "records": ["<id>", ...]}]
}
```

`threads` has one entry per `HEAD` thread that holds a listed record:
`origin`, `root`, `state`, and `closed_by` as in `qualifier show --format
json` (`state` as in `qualifier threads --format json`), plus `records`,
the IDs of the records listed above that belong to the thread (for
Changed, both `record` and `previous`). A record removed on this branch
has no thread on `HEAD` and appears in no entry. `closer` and `closers`
under `resolved` are the records whose `supersedes` points at that
record; the thread's `closed_by` is the resolve that closed the thread.

`comparison` says which commit `base` is: `"merge-base"` (the default),
`"tip"` (`--from-tip`), or `"fallback-tip"` (no merge-base exists, so the
tip of `<ref>` was used).

## Gotchas

- Requires a git repository (`.git` directory at the project root). Other
  VCSes are not supported yet.
- `<ref>` must resolve via `git rev-parse`. A branch name like `main` works;
  an arbitrary commit-ish (`HEAD~10`, `v0.5.0`, a sha) works too.
- If HEAD and `<ref>` share no common ancestor (orphan branches, fresh
  init), the merge-base default falls back to the ref tip, prints a
  one-line hint on stderr, and says so in the header
  (`Comparing HEAD against main (tip; no merge-base)`) and in the JSON
  `comparison` field.
- A malformed historical line at `<ref>` is reported on stderr and skipped —
  the diff continues. Malformed lines on `HEAD` are skipped the same way,
  with a `file:line` warning on stderr.
- Drift checking reads files from disk. If the working tree is dirty, drift
  reflects that — not the contents of `HEAD`.
