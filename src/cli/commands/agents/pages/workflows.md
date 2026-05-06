# qualifier — common workflows

## Record a finding during code review

When you notice a concern, smell, or risk in code you're reviewing, record it
immediately. Use `--span` to pin it to the exact lines so it stays actionable
after future edits, and `--suggested-fix` if you know what to do.

```bash
qualifier record concern src/auth.rs:42:58 \
  "Token validation skips expiry check when issuer is internal" \
  --suggested-fix "Check exp claim unconditionally; remove the issuer shortcut" \
  --tag security \
  --issuer "mailto:review-agent@example.com" \
  --issuer-type ai
```

The CLI writes to `src/.qual` (or `src/auth.rs.qual` if it exists), prints
the new record's id, and exits zero. Use `qualifier show src/auth.rs` to
verify the annotation is visible.

## Reply to an existing observation with new info

When a record already exists and you have additional context — a root cause,
a related finding, a status update — reply rather than recording a new
standalone annotation. A reply threads to the original so readers see the
conversation together.

`qualifier reply` accepts either a 4+ character id-prefix or a location
(`file:line`). The default kind is `comment`; override with `--kind`.

```bash
# The original concern has id starting with a1b2c3d4
qualifier reply a1b2 \
  "Root cause: the issuer allow-list is populated from an env var that CI never sets" \
  --issuer "mailto:review-agent@example.com" \
  --issuer-type ai

# Or target by location if you know the span
qualifier reply src/auth.rs:42 \
  "Root cause: the issuer allow-list is populated from an env var that CI never sets" \
  --issuer "mailto:review-agent@example.com" \
  --issuer-type ai
```

Both forms resolve to the most-recent active record at that location.
If multiple records share the newest timestamp, the CLI exits non-zero with
a disambiguation list showing id-prefix, kind, line, and summary.

## Resolve a finding once it's addressed

When the code has been fixed, close the annotation so it no longer appears
in active views. `qualifier resolve` writes a `resolve`-kind record that
supersedes the target; the original concern is removed from `qualifier show`
output but the full history remains in VCS.

```bash
qualifier resolve a1b2 "Fixed in commit abc1234 — expiry check now unconditional" \
  --issuer "mailto:review-agent@example.com" \
  --issuer-type ai
```

The message is optional; it defaults to `"Resolved"`. Prefer a meaningful
message so the tombstone is useful to future readers.

After resolving, confirm with `qualifier show src/auth.rs` — the original
concern should no longer appear (unless you pass `--all`).

## Triage stale annotations after a refactor

Span-addressed annotations include a `content_hash` (BLAKE3 of the spanned
lines at write time). After a refactor, those hashes may no longer match the
current file content. `qualifier review` checks this and reports each
annotation's freshness status.

```bash
# Check all annotations in the project
qualifier review

# Check a specific subject
qualifier review src/auth.rs

# Machine-readable output for programmatic triage
qualifier review --format json
```

Possible statuses:

- `FRESH` — the spanned lines are unchanged. No action needed.
- `DRIFTED` — the lines changed. Review whether the annotation still applies;
  resolve it if addressed, or record a new annotation at the updated span.
- `MISSING` — the file or span no longer exists. The annotation is likely stale;
  resolve it.

Annotations without a span or without a `content_hash` are not checked.
Whole-file annotations and older records written without `--span` fall into
this category.

## Compact a noisy `.qual` file

Append-only files accumulate superseded records over time. `qualifier compact`
prunes them. With `--snapshot`, it goes further and collapses all surviving
records for each subject into a single epoch record, making the file minimal.

```bash
# Preview without writing
qualifier compact src/auth.rs --dry-run

# Prune superseded records only
qualifier compact src/auth.rs

# Collapse to a single epoch per subject (smallest possible file)
qualifier compact src/auth.rs --snapshot

# Compact every .qual file in the project
qualifier compact --all

# Compact everything, preview first
qualifier compact --all --dry-run
```

Compaction is always explicit and user-initiated; it never happens silently.
Records of unrecognized types are preserved unchanged. After compaction the
file is still valid JSONL — no special reader support is needed. VCS history
retains the full pre-compaction records if you need to trace back.
