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
resolved, and whether any spans you didn't touch have drifted underneath
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

Three sections, all reckoned by record `id`:

1. **Added** — records active on `HEAD` whose `id` is not present at `<ref>`
   at all. Annotation records only; epoch and dependency records are not
   review signals.
2. **Resolved** — records active at `<ref>` that are no longer active on
   `HEAD`. Each row names the closer record (the new annotation whose
   `supersedes` points at it) when one exists, or marks the record as
   *removed* if no successor was authored.
3. **Drifted** — records present at *both* refs whose `body.span.content_hash`
   no longer matches the file's current content. Drift on records that are
   freshly added on this branch is suppressed (you just authored them; their
   span IS the current code).

## Common invocations

```bash
# What's the review trail on this branch?
qualifier diff

# Compare against the upstream branch
qualifier diff origin/main

# Machine-readable summary for CI
qualifier diff origin/main --format json

# Compare arbitrary refs
qualifier diff v0.5.0
```

## Output shape (human)

```
Added on this branch (3)
  + concern    src/cli/commands/ls.rs:46         ls --unqualified is a stub  (da1fabb9)
  + concern    src/cli/commands/emit.rs:130      empty id for custom records (d7b8f76a)
  + suggestion Cargo.toml:24                     petgraph dependency unused  (ccfe88fa)

Resolved on this branch (1)
  - concern    src/auth.rs:88                    Token comparison timing-unsafe  (ce7d1a3c) — resolved by 8f790b7b

Drifted (1)
  ~ concern    src/annotation.rs:243             span content moved (expected b4a15cbd, got 7e2ac1f0)  (b4a15cbd)
```

## Output shape (json)

```json
{
  "ref": "main",
  "added":    [<full record envelopes>],
  "resolved": [{"record": <ref-side record>, "closer": <head-side closer or null>}],
  "drifted":  [{"record": <head-side record>, "expected": "<hash>", "actual": "<hash>"}]
}
```

## Gotchas

- Requires a git repository (`.git` directory at the project root). Other
  VCSes are not supported yet.
- `<ref>` must resolve via `git rev-parse`. A branch name like `main` works;
  an arbitrary commit-ish (`HEAD~10`, `v0.5.0`, a sha) works too.
- A malformed historical line at `<ref>` is reported on stderr and skipped —
  the diff continues. Malformed lines on `HEAD` still abort discovery as
  usual.
- Drift checking reads files from disk. If the working tree is dirty, drift
  reflects that — not the contents of `HEAD`.
