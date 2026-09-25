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

# Record a suggestion
qualifier record suggestion src/auth.rs "Replace inline regex with a named constant"

# Batch-record from a JSONL file
cat findings.jsonl | qualifier record --stdin
```

Locations are relative to the current directory; subjects are stored
relative to the project root. From `src/net/`, `tcp.rs:42` records against
`src/net/tcp.rs`, written to `src/net/.qual` under the project root. A
location that leaves the project root is an error. The same rule applies to
`location` values in `--stdin` batches.

## Flags worth knowing

**`--span <SPAN>`** overrides any line range parsed from `<location>`. When
you already know the exact span (e.g., `42:58`) and want to keep the location
clean, pass it here rather than embedding it in the location string. The CLI
auto-computes a `content_hash` so that `qualifier review` can later detect
drift.

**`--supersedes <ID>`** marks this record as superseding a prior annotation.
Use this to update or correct an existing annotation rather than leaving both
visible — the superseded record is filtered out by `show`, `praise`, and
`review`. Takes the full 64-character ID of an existing record, which must
be live (not superseded, not closed); a prefix or location is rejected.
`--references <ID>` takes the same full-ID form for its target. A stale
pointer fails and prints the full ID of the live record, or of the
`resolve` that closed it. Get full IDs from `qualifier threads --format json` (`root.id`,
`closed_by.id`), `qualifier show --format json`, or the `id:` line that
`record`/`reply`/`resolve` print.

**`--issuer` / `--issuer-type`**: as an agent, leave `--issuer` and `--issuer-type` unset; see `qualifier agents concepts` for
defaults (`QUALIFIER_*` variables, agent-harness detection).

**`--stdin`** switches to batch mode. This is the path agents should reach
for when emitting more than one annotation in a session — it collapses many
sequential `qualifier record` invocations into a single pipe.

Each stdin line describes one new record, in one of two shapes. A reply is
a line whose `references` is the target's ID; a resolve is a
`kind: "resolve"` line whose `supersedes` is the target's ID (see
`qualifier agents batch`):

```jsonl
{"kind":"concern","location":"src/auth.rs:42:58","message":"Token comparison is timing-unsafe","detail":"Uses == on session_token; replace with constant-time compare.","suggested_fix":"Use subtle::ConstantTimeEq.","tags":["security"]}
{"kind":"suggestion","location":"src/auth.rs:88","message":"Extract magic constant","supersedes":"<full-64-char-id>"}
{"kind":"comment","location":"src/auth.rs","references":"<full-64-char-id>","message":"Confirmed, tracking in #482","tags":["triage"]}
{"kind":"resolve","location":"src/auth.rs","supersedes":"<full-64-char-id>","message":"Fixed in 1a2b3c4","tags":["reason:fixed"],"ref":"git:1a2b3c4"}
```

Recognized keys on the **overrides** form:

- `kind` — required. Any built-in kind or a custom string.
- `location` — required. `path` or `path:line` or `path:start:end`.
- `message` — required. Becomes `body.summary`.
- `detail`, `suggested_fix`, `tags`, `ref`, `references`, `supersedes` —
  optional, all match their `--flag` equivalents on the non-batch CLI.
  `supersedes` and `references` take a full record ID that exists on disk
  or on an earlier line of the same batch and is live, exactly like the
  `--supersedes`/`--references` flags. Only complete-envelope lines have
  IDs known in advance (overrides lines are stamped at planning time), so
  in practice an in-batch pointer names an envelope line.
- On a `kind: "resolve"` line, `tags` may carry at most one `reason:*`
  tag, and its value must be one of `fixed`, `wontfix`, `duplicate`,
  `invalid`, `obsolete` — the same rule as for `record resolve …` and
  every other `resolve` the CLI writes.
- `span` — optional. Same syntax as the `--span` flag (e.g. `"42:58"`).
  Overrides any span parsed from `location`.
- `issuer`, `issuer_type` — optional, with the same defaults as non-batch
  mode. As an agent, leave them unset; see `qualifier agents concepts` for
  defaults (`QUALIFIER_*` variables, agent-harness detection).

`--file` is rejected with `--stdin`.

The **complete record** form is recognized when an object carries both
`subject` and `body` keys; it is taken as a fully-formed envelope and only
the `id` is recomputed; its `supersedes`/`references` are stored as given.
Use this when round-tripping records produced by another tool. The overrides form is the right shape for most agent use.

Behaviour:

- Blank lines and lines starting with `//` are ignored.
- One stdout line is emitted per recorded entry (compact summary + id, or a
  full JSONL record under `--format json`). Trailing summary goes to
  **stderr** so a `--format json` pipe stays clean.
- Parse and validation errors are reported as
  `stdin line N: <reason>: <input>` (the offending input is echoed so you
  can see what was sent without re-piping).
- **Without `--continue-on-error`, the batch is all-or-nothing for
  parse/validation failures:** every line is parsed and validated before
  any record is written. If any line fails that way,
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

**`--dry-run`** validates every line (including the existence and
liveness of `supersedes`/`references` targets) but writes nothing. Output uses the
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
- Issuer defaults come from `QUALIFIER_*` variables, agent-harness
  detection, then your VCS identity (`qualifier agents concepts`). In CI,
  set `QUALIFIER_ISSUER` in the environment rather than passing `--issuer`
  on each call.
- Cross-subject supersession is rejected: a new record can only supersede a
  record with the same subject.
- Span lines are 1-indexed. Passing `--span 0` or `--span 0:5` will store
  line 0, which is outside any real file — validate your line numbers first.
