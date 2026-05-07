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
| `--kind <K[,K...]>` | Show only records whose kind matches one of the comma-separated list. Applies to all three buckets. |
| `--issuer-type <T>` | Show only records whose issuer-type is `T` (`human`, `ai`, `tool`, `unknown`). |
| `--fail-on <K[,K...]>` | Exit non-zero if Added contains any record matching one of the listed kinds. The diff is still printed first. |
| `--fail-on-drift` | Exit non-zero if Drifted is non-empty. |
| `--subjects-only` | Print only the affected subjects, deduplicated and sorted, one per line. Suppresses all other output. |
| `--from-tip` | Compare against `<ref>`'s tip rather than the merge-base of HEAD with `<ref>`. |

`--fail-on` and `--fail-on-drift` compose: pass both for a stricter CI
gate. The diff body is always printed before the failure exit, so the
build log shows exactly which record triggered the failure.

## Output shape (human)

```
Comparing HEAD against merge-base of origin/main (a3f1c4e)

Added on this branch (3)
  + concern    src/cli/commands/ls.rs:46         ls --unqualified is a stub  (da1fabb9)
  + concern    src/cli/commands/emit.rs:130      empty id for custom records (d7b8f76a)
  + suggestion Cargo.toml:24                     petgraph dependency unused  (ccfe88fa)

Resolved on this branch (1)
  - concern    src/auth.rs:88                    Token comparison timing-unsafe  (ce7d1a3c) — resolved by 8f790b7b: "constant-time compare landed in PR #142"

Drifted (1)
  ~ concern    src/annotation.rs:243             span content drifted  (b4a15cbd)
      original: "AnnotationBody field-order pin"
      src/annotation.rs:
        242 | }
      > 243 | // line moved underneath the span
        244 | impl AnnotationBody {
```

## Output shape (json)

```json
{
  "ref": "main",
  "base": "<full sha of merge-base or ref tip>",
  "from_tip": false,
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
- If HEAD and `<ref>` share no common ancestor (orphan branches, fresh
  init), the merge-base default falls back to the ref tip and prints a
  one-line hint on stderr.
- A malformed historical line at `<ref>` is reported on stderr and skipped —
  the diff continues. Malformed lines on `HEAD` still abort discovery as
  usual.
- Drift checking reads files from disk. If the working tree is dirty, drift
  reflects that — not the contents of `HEAD`.
