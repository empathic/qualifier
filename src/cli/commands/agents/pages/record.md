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

**`--stdin`** switches to batch mode. This is the path agents should reach
for when emitting more than one annotation in a session — it collapses many
sequential `qualifier record` invocations into a single pipe.

Each stdin line is one of two shapes:

```jsonl
{"kind":"concern","location":"src/auth.rs:42:58","message":"Token comparison is timing-unsafe","detail":"Uses == on session_token; replace with constant-time compare.","suggested_fix":"Use subtle::ConstantTimeEq.","tags":["security"],"issuer":"mailto:agent@ci.example.com","issuer_type":"ai"}
{"kind":"suggestion","location":"src/auth.rs:88","message":"Extract magic constant","supersedes":"<full-64-char-id>"}
```

Recognized keys on the **overrides** form:

- `kind` — required. Any built-in kind or a custom string.
- `location` — required. `path` or `path:line` or `path:start:end`.
- `message` — required. Becomes `body.summary`.
- `detail`, `suggested_fix`, `tags`, `ref`, `references`, `supersedes` —
  optional, all match their `--flag` equivalents on the non-batch CLI.
- `span` — optional. Same syntax as the `--span` flag (e.g. `"42:58"`).
  Overrides any span parsed from `location`.
- `issuer`, `issuer_type` — optional. Default to the same VCS detection
  used in non-batch mode. **Always set `"issuer_type":"ai"` from agent code.**

The **complete record** form is recognized when an object carries both
`subject` and `body` keys; it is taken as a fully-formed envelope and only
the `id` is recomputed. Use this when round-tripping records produced by
another tool. The overrides form is the right shape for most agent use.

Behaviour:

- Blank lines and lines starting with `//` are ignored.
- One stdout line is emitted per recorded entry (compact summary + id, or a
  full JSONL record under `--format json`). Trailing summary goes to
  **stderr** so a `--format json` pipe stays clean.
- Validation, IO, and parse errors are reported as
  `stdin line N: <reason>: <input>` (the offending input is echoed so you
  can see what was sent without re-piping). The batch aborts on the first
  error by default.

**`--continue-on-error`** collects every failed line, writes the records
that did pass, and exits non-zero with a final count. Use this when an
agent submits a large batch and you want to see *all* the validation
errors at once rather than fix them serially:

```bash
cat findings.jsonl | qualifier record --stdin --continue-on-error
# stderr:  stdin line 7: summary must not be empty: {"kind":"pass",...}
#          Recorded 12 of 13 records from stdin, 1 failed
```

**`--dry-run`** validates every line but writes nothing. Output uses the
verb `would-record` so a glance at stdout confirms nothing was committed.
Combine with `--continue-on-error` to find every bad line in a batch:

```bash
cat candidates.jsonl | qualifier record --stdin --dry-run --continue-on-error
```

**`--format json`** mode is fully structured on both streams:

- *stdout* — one JSONL record per processed line.
- *stderr* — one JSON object per failed line (`{"line":N,"error":"...","input":"..."}`)
  followed by a final summary trailer
  (`{"summary":{"recorded":N,"failed":M,"total":N+M,"dry_run":bool}}`).
  The top-level `qualifier:` text line is suppressed so consumers can
  parse stderr line-by-line.

The same flag set is mirrored on `qualifier emit --stdin` for non-annotation
record types (epoch, dependency, custom URIs).

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
