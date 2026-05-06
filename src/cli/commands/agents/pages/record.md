+++
name = "record"
summary = "Record a new annotation"
sees_also = ["reply", "resolve", "emit"]
since = "0.5.0"
+++

# qualifier record

## Purpose

Record a new annotation against a source artifact.

## When to use it

`record` is the primary write verb — use it whenever you want to leave a
quality signal on an artifact. It replaced the earlier per-kind verbs
(`comment`, `flag`, `suggest`, `approve`, `reject`, `attest`); all of those
kinds are now values for the first argument rather than separate subcommands.
Use `reply` when you are responding to an existing annotation thread (it
sets `body.references` automatically). Use `resolve` when you are closing an
open concern.

## Common invocations

```bash
# Leave a concern on a file
qualifier record concern src/auth.rs "Missing rate-limit on login endpoint"

# Annotate a specific line with extended detail
qualifier record concern src/auth.rs:42 "Null check missing" \
  --detail "The function returns early but does not reset the session token." \
  --suggested-fix "Add session.reset() before the return."

# Record a suggestion with explicit AI issuer identity
qualifier record suggestion src/auth.rs "Replace inline regex with a named constant" \
  --issuer "mailto:agent@ci.example.com" \
  --issuer-type ai

# Batch-record from a JSONL file
cat findings.jsonl | qualifier record --stdin
```

## Flags worth knowing

**`--span <SPAN>`** overrides any line range parsed from `<location>`. When
you already know the exact span (e.g., `42:58`) and want to keep the location
clean, pass it here rather than embedding it in the location string. The CLI
auto-computes a `content_hash` so that `qualifier review` can later detect
drift.

**`--supersedes <ID>`** marks this record as superseding a prior annotation.
Use this to update or correct an existing annotation rather than leaving both
visible — the superseded record is filtered out by `show`, `praise`, and
`review`. The value is stored verbatim — no prefix expansion is performed
here. Pass the full 64-character ID. (Short prefixes are only resolved by
`reply` and `resolve`, not `record`.)

**`--issuer-type <TYPE>`** takes `human`, `ai`, `tool`, or `unknown`. Always
set `--issuer-type ai` when writing from an agent; this lets human reviewers
distinguish machine-generated annotations from their own.

**`--stdin`** switches to batch mode. Each stdin line must be a JSON object
with at least `kind`, `location`, and `message` keys, plus any optional
overrides. Lines starting with `//` are ignored. Useful for emitting many
annotations in one pass.

## Gotchas

- All three positional arguments (`<kind>`, `<location>`, `<message>`) are
  required in non-batch mode. Missing any one of them produces a validation
  error rather than a prompt.
- The issuer defaults to your VCS user email wrapped in `mailto:`. If running
  in a CI environment with no git config, the fallback is
  `mailto:unknown@localhost` — set `--issuer` explicitly so records are
  attributable.
- Cross-subject supersession is rejected: a new record can only supersede a
  record with the same subject.
- Span lines are 1-indexed. Passing `--span 0` or `--span 0:5` will store
  line 0, which is outside any real file — validate your line numbers first.
