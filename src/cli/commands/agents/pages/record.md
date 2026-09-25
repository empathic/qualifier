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
`review`. Takes an ID or ID prefix (≥ 4 characters); it must name a live
record unless `--allow-superseded` is also passed. `--references <ID>` takes
the same ID-or-prefix form for its target.

**`--issuer-type <TYPE>`** takes `human`, `ai`, `tool`, or `unknown`. Always
set `--issuer-type ai` when writing from an agent; this lets human reviewers
distinguish machine-generated annotations from their own.

**`--stdin`** switches to batch mode. This is the path agents should reach
for when emitting more than one annotation in a session — it collapses many
sequential `qualifier record` invocations into a single pipe.

Each stdin line is one of four shapes:

```jsonl
{"kind":"concern","location":"src/auth.rs:42:58","message":"Token comparison is timing-unsafe","detail":"Uses == on session_token; replace with constant-time compare.","suggested_fix":"Use subtle::ConstantTimeEq.","tags":["security"],"issuer":"mailto:agent@ci.example.com","issuer_type":"ai"}
{"kind":"suggestion","location":"src/auth.rs:88","message":"Extract magic constant","supersedes":"<id-or-prefix>"}
{"reply":"<id-prefix-or-location>","message":"Confirmed, tracking in #482","tags":["triage"]}
{"resolve":"<id-prefix-or-location>","message":"Fixed in 1a2b3c4","reason":"fixed","ref":"git:1a2b3c4"}
```

Recognized keys on the **overrides** form:

- `kind` — required. Any built-in kind or a custom string.
- `location` — required. `path` or `path:line` or `path:start:end`.
- `message` — required. Becomes `body.summary`.
- `detail`, `suggested_fix`, `tags`, `ref`, `references`, `supersedes`,
  `allow_superseded` — optional, all match their `--flag` equivalents on
  the non-batch CLI. `supersedes` and `references` accept an ID or an ID
  prefix (≥ 4 characters), resolved the same way as the non-batch
  `--supersedes`/`--references` flags; the target must be live unless
  `allow_superseded` is set.
- `span` — optional. Same syntax as the `--span` flag (e.g. `"42:58"`).
  Overrides any span parsed from `location`.
- `issuer`, `issuer_type` — optional. Default to the same VCS detection
  used in non-batch mode. **Always set `"issuer_type":"ai"` from agent code.**

The **reply** form (has a `reply` key) responds to an existing thread the
same way `qualifier reply` does: `reply` names the target (an ID prefix or
a `<location>`), `message` is required, and `kind`, `detail`,
`suggested_fix`, `tags`, `issuer`, `issuer_type`, `ref`, `supersedes`, and
`allow_superseded` mirror the `reply` command's flags.

The **resolve** form (has a `resolve` key) closes an existing thread the
same way `qualifier resolve` does: `resolve` names the target, and
`message`, `reason`, `tags`, `issuer`, `issuer_type`, `ref`, and
`allow_superseded` mirror the `resolve` command's flags. A line must not
set both `reply` and `resolve`.

For `reply`/`resolve` lines, the target may name a record created earlier
in the same batch — targets are resolved against the records on disk plus
everything already planned by prior lines in this batch.

The **complete record** form is recognized when an object carries both
`subject` and `body` keys; it is taken as a fully-formed envelope and only
the `id` is recomputed. Use this when round-tripping records produced by
another tool. The overrides form is the right shape for most agent use.

Behaviour:

- Blank lines and lines starting with `//` are ignored.
- One stdout line is emitted per recorded entry (compact summary + id, or a
  full JSONL record under `--format json`). Trailing summary goes to
  **stderr** so a `--format json` pipe stays clean.
- Parse, resolution, and validation errors are reported as
  `stdin line N: <reason>: <input>` (the offending input is echoed so you
  can see what was sent without re-piping).
- **Without `--continue-on-error`, the batch is all-or-nothing for
  parse/resolve/validation failures:** every line is parsed, resolved,
  and validated before any record is written. If any line fails that way,
  every failing line is reported and *nothing* is written — including
  lines before the failure.
- This guarantee does not cover I/O failures while writing. Planning
  happens first and in full, but the write itself still happens line by
  line; if appending a planned record to disk fails partway through (disk
  full, permissions revoked mid-run, etc.), the lines already written stay
  written. The error message names how many: `wrote N of M records before
  an I/O error appending stdin line L: <cause>`.

**`--continue-on-error`** collects every failed line, writes the records
that did pass, and exits non-zero with a final count. Use this when an
agent submits a large batch and you want to see *all* the validation
errors at once rather than fix them serially:

```bash
cat findings.jsonl | qualifier record --stdin --continue-on-error
# stderr:  stdin line 7: summary must not be empty: {"kind":"pass",...}
#          Recorded 12 of 13 records from stdin, 1 failed
```

**`--dry-run`** validates every line (resolving `reply`/`resolve` targets
and `supersedes`/`references` in full) but writes nothing. Output uses the
verb `would-record` so a glance at stdout confirms nothing was committed.
Combine with `--continue-on-error` to find every bad line in a batch:

```bash
cat candidates.jsonl | qualifier record --stdin --dry-run --continue-on-error
```

**`--format json`** mode is fully structured on both streams:

- *stdout* — one JSONL record per processed line.
- *stderr* — one JSON object per failed line (`{"line":N,"error":"...","input":"..."}`)
  followed by a final summary trailer
  (`{"summary":{"recorded":N,"failed":M,"total":N+M,"dry_run":bool,"written":bool}}`).
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
