# qualifier review

## Purpose

Check whether span-bound annotations still point at the same code they were
written against, reporting each annotation as fresh, drifted, or missing.

## When to use it

Run `review` after editing any file that has span-addressed annotations.
When `qualifier record` writes a span annotation, it computes a
`content_hash` of the referenced lines and stores it in `body.span`. Over
time, edits shift line numbers and change content. `review` re-reads each
spanned region and compares it against the stored hash, flagging annotations
whose code has moved or changed. This is not a general record-listing
command — it only operates on active, span-addressed annotations that have a
`content_hash`. Annotations without a span (or with a span but no hash) are
silently skipped.

## Common invocations

```bash
# Check all span-bound annotations in the project
qualifier review

# Check only annotations on one file
qualifier review src/auth.rs

# Machine-readable output for scripting
qualifier review --format json
```

## Flags worth knowing

**`[subject]`** is optional. When supplied, only annotations whose subject
matches the given string are checked. Useful in a pre-commit or CI step to
limit the check to files that were actually changed (e.g., iterate over
`git diff --name-only` and call `qualifier review <file>` per path).

**`--format json`** emits a JSON array. Each element has `subject`,
`location` (e.g., `src/auth.rs:42`), `kind`, `summary`, `status`
(`"fresh"`, `"drifted"`, or `"missing"`), and a `detail` object with
`expected`/`actual` hashes for drifted results, or a `reason` string for
missing ones. Parse this when automating annotation triage after a refactor.

## Gotchas

- Only annotations with **both** a span **and** a `content_hash` are
  checked. Annotations recorded without a span, or recorded using an older
  version of the tool before content hashing was added, are silently skipped.
  The summary line ("N annotations checked") counts only the subset that had
  hashes to check.
- A `"drifted"` status means the lines still exist but their content has
  changed. A `"missing"` status means the file could not be read or the
  line range is now out of bounds. Neither status causes a non-zero exit
  code — `review` is informational. Act on drifted or missing annotations
  by re-recording them with updated spans (`qualifier record --supersedes`)
  or resolving them if they are no longer relevant.
- `review` does not fix annotations automatically. It is a diagnostic
  tool; remediation is the caller's responsibility.
