# Qualifier Specification

**Version:** 0.5.0
**Status:** Draft
**Authors:** Alex Kesling

---

## Abstract

Qualifier is a deterministic system for recording, propagating, and querying
typed metadata records against software artifacts. It provides a VCS-friendly
file format (`.qual`), a Rust library (`libqualifier`), and a CLI binary
(`qualifier`) that together enable humans and agents to annotate code with
structured quality signals without waiting for a formal process. Each
annotation carries a `kind` (concern, comment, suggestion, pass, fail,
blocker, praise, waiver, resolve, or any custom string), letting tools
filter, thread, and aggregate however they need. Records thread, persist,
and compose through a single content-addressed model.

Records use the [Metabox](METABOX.md) envelope format: a fixed envelope
(`metabox`, `type`, `subject`, `issuer`, `issuer_type`, `created_at`, `id`)
wrapping a type-specific `body` object. Records are content-addressed, append-only, and
human-writable. No server, no database, no PKI required.

## 0. Why Qualifier

Software is full of structured observations that have no good home. A reviewer
notices that a function panics on malformed input. A scanner reports a CVE in a
transitive dependency. A profiler measures a regression on a hot path. A
licensing audit confirms that a vendored file is MIT. Today, each of these
observations lands in a different system — a PR comment, a SARIF report, a
spreadsheet, a wiki page — and none of those systems talks to the others. The
observations decay because they live somewhere code does not. Structured
knowledge about code deserves the same rigor we apply to the code itself.

Consider the alternatives we currently reach for. **GitHub PR comments** are
tied to a diff window and disappear from view the moment the PR merges; the URL
still resolves, but nothing in the working tree points at it, no tool can query
it, and a refactor that touches the same lines a year later has no idea the
conversation ever happened. **SARIF reports** are produced once by a tool, then
either ignored or archived; they have no notion of human reply, threading, or
follow-up. **`// TODO:` comments** are unstructured prose hidden in code, with
no type, no severity, no author beyond `git blame`, and no way to thread a
discussion. **Issue trackers** are separate from the code they describe; they
collect bit-rot, can't address a specific span, and require context-switching
to a different application to learn anything about the file in front of you.

Each of these tools fails the same way: the observation is not a first-class,
addressable, durable artifact alongside the code. Qualifier's wager is that if
you make structured observations look like code — files in the repo, version
controlled, content-addressed, append-only, threadable — they stop evaporating.
A concern raised in February is still queryable in October. A reply written by
an agent threads to the human comment that prompted it. A resolution
supersedes the original signal without erasing it. Merges are clean under
normal workflows because the file format is designed for it. Tooling can read
every record because the envelope is uniform.

The same skeleton that holds a human concern also holds a license declaration,
a security advisory, or a performance measurement. The format is a substrate;
annotations are simply its first and most-developed application. The cost of
adoption is one JSONL file per directory and a CLI; the payoff is that the
structured knowledge you produce — by hand, by review, by tool, by agent —
finally has somewhere to live where it accumulates instead of decays.

If you are forwarding this document to convince a teammate: the pitch is that
your team already produces this metadata. It is currently scattered across
five systems and lost on every merge. Qualifier gives it a single home, in
files, that survives.

## 1. Design Principles

1. **Files are the API.** The `.qual` format is the primary interface. Every
   tool — CLI, editor plugin, CI bot, coding agent — reads and writes the same
   files. No server, no database, no lock-in.

2. **VCS-native.** `.qual` files are append-only JSONL. They merge cleanly,
   diff readably, and blame usefully. Conflicts are structurally impossible
   under normal workflows (append-only + file-per-artifact).

3. **Open record types.** The format is a substrate, not a single application.
   The Metabox envelope is fixed; record bodies are typed and extensible. New
   record types extend the system without changing the envelope, and
   unrecognized types pass through harmlessly. Annotations are the primary
   record type and the reason qualifier exists, but the same skeleton supports
   license declarations, security advisories, performance measurements, build
   provenance, or any other structured observation about a software artifact.
   Choosing the format does not lock you into a single domain.

4. **Ambient annotation.** Record observations the moment you see them. No PR
   required, no review window, no formal ceremony. A human reading code can
   leave a `concern` in five seconds; an agent finishing a task can leave a
   `comment` to flag a follow-up; a scanner can drop a `security-advisory`
   into the same file. The practice is structurally enabled by append-only
   JSONL plus content-addressed records — adding a record never conflicts with
   another, and every record has a stable, addressable identity from the
   moment it is written.

5. **Deterministic record IDs.** A record's `id` is the BLAKE3 hash of its
   Metabox Canonical Form (§2.8). Identical inputs produce identical IDs
   on every implementation — no language-specific or library-specific drift.

6. **Propagation through the graph.** Quality is more than local. Software has
   dependencies. An artifact's *effective* quality is a function of its own
   annotations AND the effective quality of everything it depends on. A
   pristine binary that links a cursed library inherits the curse.

7. **Human-first, agent-friendly.** The CLI is designed for humans at a
   terminal. The JSONL format and library API are designed for agents and
   tooling. Both are first-class.

8. **Composable.** The record format uses the Metabox envelope — a uniform
   frame (who said something about which subject) wrapping typed payloads
   (what they said). Records compose into threads via `references`, into
   chains via `supersedes`, and into graphs via `dependency` records.

9. **Interoperable.** Qualifier records project losslessly into in-toto
   annotation predicates. SARIF results import into qualifier annotations.
   The format bridges the gap between supply-chain annotation frameworks and
   human-scale quality tracking.

## 2. Record Model

### 2.1 Records

A **record** is a single, immutable, content-addressed JSON object that says
something about a software artifact. Records are the atoms of the system.

Every record has a **Metabox envelope** — a fixed set of fields that identify
*who* said *what kind of thing* about *which subject* and *when* — plus a
**body** object containing type-specific fields.

### 2.2 Metabox Envelope

Every record uses the [Metabox](METABOX.md) envelope format with these fields:

| Field          | Type     | Required | Description |
|----------------|----------|----------|-------------|
| `metabox`      | string   | yes      | Envelope version. MUST be `"1"`. |
| `type`         | string   | yes*     | Record type identifier (see 2.5). *May be omitted in `.qual` files; defaults to `"annotation"`. |
| `subject`      | string   | yes      | Qualified name of the target artifact |
| `issuer`       | string   | yes      | Who or what created this record (URI) |
| `issuer_type`  | string   | no       | Issuer classification: `human`, `ai`, `tool`, `unknown` |
| `created_at`   | string   | yes      | RFC 3339 timestamp, hashed exactly as written (§2.8.1 rule 9) |
| `id`           | string   | yes      | Content-addressed BLAKE3 hash (see 2.8) |
| `body`         | object   | yes      | Type-specific payload — see §3 for body schemas by type |

These eight fields form the **uniform interface**. They are the same for every
record type, they are stable across spec revisions, and they are sufficient
to answer the questions "who said what kind of thing about what and when?"
without understanding the body.

### 2.3 Subject Names

A **subject** is any addressable unit of software that can be qualified.
Subjects are identified by a **qualified name** (a string), which SHOULD
correspond to a logical unit in the codebase:

- A file path: `src/parser.rs`
- A module: `crate::parser`
- A build target: `//services/auth:lib`
- A package: `pkg:npm/lodash@4.17.21`

Qualifier does not enforce a naming scheme. The names are opaque strings.
Conventions are a project-level decision.

#### 2.3.1 Subject Renames

Qualifier identifies subjects by their qualified name. Renaming a subject
(e.g., `src/parser.rs` to `src/ast_parser.rs`) requires the following steps:

1. Rename the `.qual` file to match the new subject name.
2. Update dependency records to reference the new name wherever the old name
   appeared (both as `subject` and in `depends_on` arrays).
3. **Note:** Existing records inside the renamed `.qual` file still contain the
   old `subject` field in their JSON. Since record IDs are content-addressed,
   changing the `subject` field would change the ID, breaking supersession
   chains.

The RECOMMENDED workflow after a rename is:

1. Rename the `.qual` file and update dependency records.
2. Run `qualifier compact <new-name> --snapshot` to collapse history into a
   fresh epoch under the new name.
3. Commit the rename and compacted file together.

### 2.4 Spans

A **span** identifies a sub-range within a subject. When present in the body,
the record addresses a specific region rather than the whole artifact.

```json
"span": {
  "start": { "line": 42 },
  "end": { "line": 58 }
}
```

A span is an object with these fields:

| Field          | Type   | Required | Description |
|----------------|--------|----------|-------------|
| `start`        | object | yes      | Start of the range (inclusive) |
| `end`          | object | no       | End of the range (inclusive). Defaults to `start`. |
| `content_hash` | string | no       | BLAKE3 hash of the spanned lines (see 2.4.3) |

Each position has:

| Field  | Type    | Required | Description |
|--------|---------|----------|-------------|
| `line` | integer | yes      | 1-indexed line number |
| `col`  | integer | no       | 1-indexed column number |

#### 2.4.1 Span Forms

```json
// Lines 42 through 58:
"span": {"start": {"line": 42}, "end": {"line": 58}}

// Line 42 only (end defaults to start):
"span": {"start": {"line": 42}}

// Columns 5–15 on line 42:
"span": {"start": {"line": 42, "col": 5}, "end": {"line": 42, "col": 15}}

// Cross-line range with column precision:
"span": {"start": {"line": 42, "col": 5}, "end": {"line": 58, "col": 80}}
```

#### 2.4.2 Span Normalization

Before hashing (see 2.8), spans are normalized:

- If `end` is absent, it is set equal to `start`.
- If `col` is absent from a position, it remains absent (not defaulted).
- `content_hash` is not modified during normalization. It passes through
  unchanged and participates in the record ID computation when present.

After normalization, `{"start":{"line":42}}` and
`{"start":{"line":42},"end":{"line":42}}` produce identical canonical forms
and therefore identical record IDs.

#### 2.4.3 Content Hashing

When `content_hash` is present, it records a BLAKE3 hash of the source lines
covered by the span at the time the annotation was created. This enables
**freshness checking** — detecting whether the annotated code has changed
since the annotation was written.

**Hash computation:**

1. Read the file identified by the record's `subject`.
2. Extract lines `start.line` through `end.line` (inclusive, 1-indexed).
   Columns are ignored — full lines are always hashed.
3. Join the extracted lines with `\n` (no trailing newline).
4. Compute the BLAKE3 hash of the resulting byte string.
5. Encode as lowercase hex.

**When computed:** The CLI auto-computes `content_hash` when creating span-
addressed annotations (`record` with a span, and `record --stdin`
overrides lines; `reply` and `resolve` write no span) if the subject file
exists and the span is within bounds. If the file does not exist or the span extends beyond EOF, `content_hash`
is omitted.

**Relationship to `ref`:** The `ref` field pins an annotation to a VCS
revision (e.g., `git:3aba500`). `content_hash` pins the annotation to
specific file content. They are complementary: `ref` answers "which commit?"
while `content_hash` answers "has the code changed?"

**Freshness states:**

| State     | Meaning |
|-----------|---------|
| Fresh     | `content_hash` matches current file content |
| Drifted   | `content_hash` differs from current file content |
| Missing   | File not found or span beyond EOF |
| No hash   | Annotation has no `content_hash` (older or whole-file annotations) |

#### 2.4.4 Spans Address Subjects

Span-addressed records attach to their parent **subject**. An annotation
about `src/parser.rs` at span `{start: {line: 42}, end: {line: 58}}` is a
record about `src/parser.rs` that happens to point at lines 42–58.

Spans are addressing granularity. They tell you *where* within the subject
a signal applies but do not create separate addressing targets.

### 2.5 Record Types

The `type` field is a string that identifies the body schema. Implementations
MUST support the `annotation`, `epoch`, and `dependency` types. Additional
types defined in this spec are RECOMMENDED but not strictly required —
implementations that don't understand them MUST still preserve them (forward
compatibility).

| Type                 | Description |
|----------------------|-------------|
| `annotation`         | A quality signal (see 2.6) |
| `epoch`              | A compaction snapshot (see 3.2) |
| `dependency`         | A dependency edge (see 3.4) |
| `license`            | A license declaration (see 3.5) |
| `security-advisory`  | A known vulnerability or weakness (see 3.6) |
| `perf-measurement`   | A performance measurement (see 3.7) |

Implementations MUST ignore records with unrecognized types (forward
compatibility). Unrecognized records MUST be preserved during file operations
(compaction, rewriting) — they are opaque pass-through data.

When `type` is omitted in a `.qual` file, it defaults to `"annotation"`.
In canonical form (for hashing), `type` is always materialized.

### 2.6 Annotation Records

An **annotation** is a quality signal about a subject. It is the primary
record type and the reason qualifier exists.

Metabox envelope fields (section 2.2) plus body fields:

| Field           | Type     | Required | Description |
|-----------------|----------|----------|-------------|
| `detail`        | string   | no       | Extended description, markdown allowed |
| `kind`          | string   | yes      | The type of annotation (see 2.7) |
| `ref`           | string   | no       | VCS reference pin (e.g., `"git:3aba500"`). Opaque to qualifier. |
| `references`    | string   | no       | ID of a related record (see 2.11) |
| `span`          | object   | no       | Sub-artifact range (see 2.4) |
| `suggested_fix` | string   | no       | Actionable suggestion for improvement |
| `summary`       | string   | yes      | Human-readable one-liner |
| `supersedes`    | string   | no       | ID of a prior record this replaces (see 2.9) |
| `tags`          | string[] | no       | Freeform classification tags |

Body fields are listed in alphabetical order, which matches the Metabox
Canonical Form (MCF) serialization order.

**Example:**

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-25T10:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","ref":"git:3aba500","span":{"start":{"line":42},"end":{"line":58}},"suggested_fix":"Use the ? operator instead of unwrap()","summary":"Panics on malformed input","tags":["robustness"]}}
```

**Shorthand (equivalent):** Since `type` defaults to `"annotation"`, it may
be omitted:

```json
{"metabox":"1","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-02-25T10:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","summary":"Panics on malformed input"}}
```

### 2.7 Annotation Kinds

The `kind` field is an open enum. The following kinds are defined by the spec;
implementations MUST support them and MAY define additional kinds.

| Kind          | Meaning |
|---------------|---------|
| `pass`        | The artifact meets a stated quality bar |
| `fail`        | The artifact does NOT meet a stated quality bar |
| `blocker`     | A blocking issue that must be resolved before release |
| `concern`     | A non-blocking issue worth tracking |
| `comment`     | An observation or discussion point with no scoring impact |
| `praise`      | Positive recognition of quality |
| `resolve`     | Closes a prior record via supersession |
| `suggestion`  | A proposed improvement (typically paired with `suggested_fix`) |
| `waiver`      | An acknowledged issue explicitly accepted (with rationale) |

#### 2.7.1 Sign Conventions

The kinds carry an implicit polarity that downstream tools (scoring,
filtering, gating) can use. Implementations layering numeric signals on
top SHOULD respect these signs:

| Kind          | Polarity |
|---------------|----------|
| `pass`        | positive |
| `praise`      | positive |
| `waiver`      | positive |
| `comment`     | neutral  |
| `resolve`     | neutral  |
| `concern`     | negative |
| `suggestion`  | negative |
| `fail`        | negative |
| `blocker`     | negative |

The format itself does not carry a numeric score. Tools MAY add custom
body fields (e.g., a `score` integer) and define their own evaluation
semantics on top of the kind polarity — see §4 for one possible
shape.

#### 2.7.2 Custom Kinds

Any string is a valid `kind`. Implementations SHOULD detect likely typos
(edit distance <= 2 from a built-in kind, `resolve` included) and warn the
user. The reference CLI rejects them: `qualifier record resovle …` fails
with `unknown kind 'resovle', did you mean 'resolve'?`.

### 2.8 Record IDs & Canonical Form

A record ID is a lowercase hex-encoded BLAKE3 hash of the **Metabox
Canonical Form (MCF)** of the record, with the `id` field set to the empty
string `""` during hashing. This makes IDs deterministic and
content-addressed.

#### 2.8.1 Metabox Canonical Form (MCF)

To ensure that every implementation — regardless of language or JSON library —
produces identical bytes for the same record, the canonical serialization MUST
obey the following rules:

1. **Normalization.** Before serialization:
   - `type` MUST be materialized. If absent, set to `"annotation"`.
   - `metabox` MUST be materialized. If absent, set to `"1"`.
   - `span.end` MUST be materialized (in body). If absent, set equal to
     `span.start`.
   - `id` MUST be set to `""` (the empty string).

2. **Envelope field order.** Envelope fields MUST appear in this fixed order:
   `metabox`, `type`, `subject`, `issuer`, `issuer_type`, `created_at`, `id`,
   `body`. Optional envelope fields (`issuer_type`) are omitted when absent.

3. **Body field order.** The body's top-level keys MUST appear in
   lexicographic (alphabetical) order. Custom body fields that the record
   type does not define (see §4) are part of the body: implementations
   MUST preserve them when rewriting a record, and they are hashed sorted
   together with the defined fields, never appended after them.

   This rule does not reorder keys inside nested values. A `span` keeps
   the order `start`, `end`, `content_hash`, and a position the order
   `line`, `col`, as in §2.4. Free-form JSON values (the values of custom
   fields, and the bodies of record types the implementation does not
   know) are serialized with their keys in lexicographic order at every
   level. Whether nested keys of defined fields will also be sorted is an
   open question; it would change the ID of every span-addressed record.

4. **Absent optional fields.** Optional fields whose value is absent (null,
   None, etc.) MUST be omitted entirely. `tags` MUST be omitted when the
   array is empty. The `id` field is the sole exception — it is always
   present (set to `""`).

5. **Whitespace.** No whitespace between tokens. No space after `:` or `,`.
   No trailing newline. The output is a single compact JSON line.

6. **No trailing commas.** Standard JSON — no trailing commas.

7. **String encoding.** Standard JSON escaping (RFC 8259 Section 7).
   Implementations MUST NOT add escapes beyond what JSON requires.

8. **Number encoding.** Integers serialize as bare decimal with no leading
   zeros, no decimal point, no exponent. Negative values use a leading `-`.

9. **Timestamps.** `created_at` is hashed exactly as it is written in the
   record. Implementations MUST NOT re-encode it (normalize the offset,
   add or drop fractional digits) when hashing or rewriting a record, so a
   record keeps its ID wherever it is copied. Any valid RFC 3339 timestamp
   is accepted. Records an implementation creates SHOULD use the
   **canonical timestamp form**: UTC with a `Z` suffix and 0, 3, 6 or 9
   fractional-second digits, the fewest that represent the instant exactly
   (`2026-02-24T10:00:00Z`, `2026-02-24T10:00:00.500Z`,
   `2026-02-24T10:00:00.123456Z`). Qualifier writes only this form.

Records of types the implementation does not know (§2.5) are hashed the
same way: envelope fields in the order of rule 2 (any other top-level
fields after them, in lexicographic order), `metabox` materialized, `id`
set to `""`.

See the [Metabox specification](METABOX.md) for the full MCF definition.

#### 2.8.2 Example

Given an annotation with no optional body fields, the MCF is:

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-02-24T10:00:00Z","id":"","body":{"kind":"concern","summary":"Panics on malformed input"}}
```

With a span and issuer_type:

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-24T10:00:00Z","id":"","body":{"kind":"concern","span":{"start":{"line":42},"end":{"line":42}},"summary":"Panics on malformed input"}}
```

Note that `span.end` has been materialized (it was omitted in the input,
defaulting to `start`), body fields appear in alphabetical order, and
`issuer_type` is in the envelope between `issuer` and `created_at`.

> **Rationale.** MCF extends the behavior of serde_json with
> `#[serde(skip_serializing_if)]` annotations. Alphabetical body field
> ordering is simpler than per-type field orders and eliminates the need for
> type-specific canonical form definitions.

### 2.9 Supersession

Records are immutable once written. To "update" a signal, you write a new
annotation with a `supersedes` field (in the body) pointing to the prior
record's `id`.

**Constraints:**

- The superseding and superseded records MUST have the same `subject` field.
  Cross-subject supersession is forbidden. Implementations MUST reject it.
- The `span` field MAY differ between superseder and superseded. (The
  problematic code may have moved.)
- Supersession chains MUST be acyclic. Implementations MUST detect and reject
  cycles.
- When evaluating active records, a superseded record MUST be excluded.
  Only the tip of each chain is active.
- Dangling `supersedes` references (pointing to IDs not present in the current
  file set) are allowed. The referencing record remains active.

**Resolve pattern:** A `resolve`-kind annotation supersedes its target,
withdrawing the target from the active set. This is the canonical way to
close an issue: the target's thread is closed, and the resolve record is
its answer (`closed_by`, §7).

### 2.10 The `.qual` File Format

A `.qual` file is a UTF-8 encoded file where each line is a complete JSON
object representing one record. This is JSONL (JSON Lines).

**Placement:** A `.qual` file can contain records for any subjects in its
directory or subdirectories. The `subject` field in each record is the
authoritative identifier — not the filename.

**Layout strategies:**

| Strategy | Example | Pros | Cons |
|----------|---------|------|------|
| **Per-directory** (recommended) | `src/.qual` | Clean tree, good merge behavior | Slightly more merge contention than 1:1 |
| Per-file | `src/parser.rs.qual` | Maximum merge isolation | Noisy file tree |
| Per-project | `.qual` at repo root | Simplest setup | High merge contention |

All layouts are backwards-compatible and can coexist in the same project.

**Rules:**
- Each line MUST be a valid JSON object conforming to a known or unknown
  record type.
- Lines MUST be separated by a single `\n` (LF).
- The file MUST end with a trailing `\n`.
- Empty lines and lines starting with `//` are ignored (comments).
- Implementations MUST preserve ordering; older records come first.
- New records MUST be appended, never inserted.
- The sole exception to append-only is **compaction** (see 3.3), which
  rewrites the file.

**Example (mixed record types):**

```jsonl
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-24T10:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","ref":"git:3aba500","span":{"start":{"line":42},"end":{"line":58}},"suggested_fix":"Replace .unwrap() with proper error propagation","summary":"Panics on malformed UTF-8 input","tags":["robustness","error-handling"]}}
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:bob@example.com","issuer_type":"human","created_at":"2026-02-24T11:00:00Z","id":"e5f6a7b8...","body":{"kind":"praise","summary":"Excellent property-based test coverage","tags":["testing"]}}
```

### 2.11 References

The `references` body field provides a lightweight "re:" pointer from one
annotation to another. Unlike `supersedes` (which removes the referenced
record from the active set), `references` is purely informational — both
the original and the referencing record remain active.

**Semantics:**

- A `references` value is a single record ID string.
- The referenced record is NOT filtered out. Both records remain active.
- Cross-subject references are allowed. An annotation on `src/lexer.rs`
  MAY reference a record on `src/parser.rs` ("see also").
- Dangling references are allowed (same policy as `supersedes`). The
  referenced record may live in a different file or not be loaded.
- Self-references are forbidden. An annotation MUST NOT reference its own
  ID. Implementations MUST reject this at write time.

**Use cases:**

- Reply threads: an AI follow-up to a human observation.
- Resolution chains: "this addresses the concern raised in <id>".
- Cross-file commentary: "see also the related concern on lexer.rs".

**Threading semantics:** Records referencing the same parent form a thread.
Implementations SHOULD display these as threaded conversations, with each
reply under the record it answers (the reference CLI indents replies; see
§6.6). Reply depth is unbounded — a reply to a reply is a valid thread.

**Example:**

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:bob@example.com","created_at":"2026-03-01T10:00:00Z","id":"b2c3d4e5...","body":{"kind":"comment","references":"a1b2c3d4...","summary":"This was addressed in the latest refactor"}}
```

**Full lifecycle example (flag → reply → resolve):**

```jsonl
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-03-01T09:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","span":{"start":{"line":42}},"summary":"Panics on malformed input"}}
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:bob@example.com","created_at":"2026-03-01T10:00:00Z","id":"b2c3d4e5...","body":{"kind":"comment","references":"a1b2c3d4...","summary":"Good catch — fixed in latest commit"}}
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-03-01T11:00:00Z","id":"c3d4e5f6...","body":{"kind":"resolve","summary":"Resolved","supersedes":"a1b2c3d4..."}}
```

After the resolve, the original concern leaves the active set; the reply
remains visible in the thread for context.

### 2.12 Reserved Tag Namespaces

Tags are free-form, but these prefixes carry meaning that tools interpret.
A tag in a reserved namespace must follow its rule.

| namespace | form | meaning |
|---|---|---|
| `status:` | `status:needs-decision[:<issuer>]`, `status:decided`, `status:deferred` | Workflow state of a thread. A thread's status is its latest `status:*` tag by `created_at`, over the root and live replies. An `:<issuer>` suffix addresses a decision to someone. |
| `reason:` | `reason:fixed`, `reason:wontfix`, `reason:duplicate`, `reason:invalid`, `reason:obsolete` | Why a `resolve` closed its target. At most one per record. |
| `session:` | `session:<harness>:<id>` | The agent session whose reasoning produced the record. A pointer for readers who have the transcript; the record must stand alone without it. |
| `revisit:` | `revisit:<condition>` | On an `alternative` annotation: the observable condition under which to reconsider the option. |
| `depends-on:` | `depends-on:<record id>` | On a reply in a thread: the thread cannot land before the thread whose `origin` (§7, `qualifier::threads`) has this full ID. The origin, unlike the root, is stable when the root is edited or re-anchored. |

## 3. Record Type Specifications

### 3.1 Annotation (`type: "annotation"`)

Defined in section 2.6. This is the primary record type.

### 3.2 Epoch (`type: "epoch"`)

An **epoch** is a synthetic compaction summary produced by the compactor. It
replaces a set of records with a single record that preserves their refs.

Body fields (alphabetical):

| Field         | Type     | Required | Description |
|---------------|----------|----------|-------------|
| `refs`        | string[] | yes      | IDs of the compacted records |
| `span`        | object   | no       | Sub-artifact range |
| `summary`     | string   | yes      | `"Compacted from N records"` |

Epoch records MUST set `issuer` to `"urn:qualifier:compact"` and
`issuer_type` to `"tool"` (in the envelope).

**Example:**

```json
{"metabox":"1","type":"epoch","subject":"src/parser.rs","issuer":"urn:qualifier:compact","issuer_type":"tool","created_at":"2026-02-25T12:00:00Z","id":"f9e8d7c6...","body":{"refs":["a1b2...","c3d4..."],"summary":"Compacted from 12 records"}}
```

The `refs` field exists for auditability — it lets you trace back (via VCS
history) to the individual records that were folded in.

### 3.3 Compaction

Append-only files grow without bound. **Compaction** is the mechanism for
reclaiming space.

A compaction rewrites a `.qual` file by:

1. **Pruning** superseded records. If record B supersedes A, only B is
   retained, and the entire chain collapses to its tip. A superseded record
   is kept when a retained record names it in `references`; every record
   that supersedes a kept record is then kept too. Pruning therefore never
   changes how the remaining records group into threads (§2.11): a
   resolved thread with replies keeps its root, and its replies do not
   become threads of their own.
2. **Optionally snapshotting.** When `--snapshot` is passed, superseded
   records are pruned and the surviving annotation and epoch records for
   each subject are replaced by a single epoch record whose `refs` lists
   those surviving records. A subject whose only record is already an
   epoch is left unchanged.

Compacting one subject (`qualifier compact <artifact>`) rewrites only that
subject's records, in every `.qual` file that holds them; records of other
subjects in the same file are written back unchanged.

#### 3.3.1 Compaction Rules

- Compaction MUST be explicit and user-initiated — never automatic or silent.
- Compaction MUST preserve records of unrecognized types (they are opaque
  pass-through).
- After compaction, the file is a valid `.qual` file. No special reader
  support is needed.
- `qualifier compact --dry-run` MUST be supported.
- A snapshot that would fold an open `blocker` or `concern` thread into an
  epoch MUST be refused unless the user forces it (`--force`).

### 3.4 Dependency (`type: "dependency"`)

A **dependency** record declares directed dependency edges from one subject
to others.

Body fields:

| Field        | Type     | Required | Description |
|--------------|----------|----------|-------------|
| `depends_on` | string[] | yes      | Subject names this subject depends on |

**Example:**

```json
{"metabox":"1","type":"dependency","subject":"bin/server","issuer":"https://build.example.com","created_at":"2026-02-25T10:00:00Z","id":"1a2b3c4d...","body":{"depends_on":["lib/auth","lib/http","lib/db"]}}
```

The dependency graph implied by these records MUST be a DAG.

Dependency records are wire-format-level: they declare edges that
downstream tools can use to propagate signals across artifacts. The
reference CLI does not consume them today (the `qualifier graph` command
and built-in graph engine were yanked along with scoring), but they
round-trip through `.qual` files unchanged.

### 3.5 License (`type: "license"`)

A **license** record declares the licensing terms that apply to a subject.
License records are typically produced by a license scanner or written by
hand during a licensing audit.

Body fields:

| Field        | Type    | Required | Description |
|--------------|---------|----------|-------------|
| `confidence` | number  | no       | Detector confidence in `[0.0, 1.0]`. Omit for hand-asserted records. |
| `evidence`   | string  | no       | Free-form provenance for the assertion (e.g., `"LICENSE file SHA256:abc..."`, `"package.json#license"`). |
| `spdx_id`    | string  | yes      | SPDX license identifier (e.g., `"MIT"`, `"Apache-2.0"`, `"GPL-3.0-or-later"`). |

**Example:**

```json
{"metabox":"1","type":"license","subject":"vendor/lodash","issuer":"https://license-scanner.example.com","issuer_type":"tool","created_at":"2026-03-01T10:00:00Z","id":"...","body":{"confidence":0.98,"evidence":"LICENSE file SHA256:9f86d081...","spdx_id":"MIT"}}
```

A license record documents an attribute of the subject; if a licensing
problem warrants a quality signal, write a separate `annotation` (e.g.,
`kind: "blocker"`) and optionally `references` the license record.

### 3.6 Security Advisory (`type: "security-advisory"`)

A **security-advisory** record records a known vulnerability or weakness
affecting a subject. Records of this type are typically produced by a
vulnerability scanner, an SBOM tool, or written by hand when triaging a CVE.

Body fields:

| Field               | Type   | Required | Description |
|---------------------|--------|----------|-------------|
| `affected_versions` | string | no       | Version range expression (e.g., `"<1.4.2"`, `">=2.0.0,<2.3.1"`). |
| `cve_id`            | string | no       | CVE identifier (e.g., `"CVE-2024-1234"`). |
| `cwe_id`            | string | no       | CWE identifier (e.g., `"CWE-79"`). |
| `severity`          | string | yes      | One of `critical`, `high`, `medium`, `low`, `info`. |
| `summary`           | string | yes      | Human-readable one-line description of the issue. |

At least one of `cve_id` or `cwe_id` SHOULD be present, but neither is
strictly required (some advisories predate CVE assignment or describe
project-specific issues).

**Example:**

```json
{"metabox":"1","type":"security-advisory","subject":"vendor/openssl","issuer":"https://osv.dev","issuer_type":"tool","created_at":"2026-03-01T10:00:00Z","id":"...","body":{"affected_versions":"<3.0.8","cve_id":"CVE-2023-0286","severity":"high","summary":"X.400 address type confusion in X.509 GeneralName"}}
```

To turn a security advisory into a quality signal, write an `annotation`
(e.g., `kind: "blocker"`) on the same subject that `references` the advisory.

### 3.7 Performance Measurement (`type: "perf-measurement"`)

A **perf-measurement** record captures a single performance measurement for
a subject. Records of this type are typically produced by a benchmark
harness, a profiler, or a CI job that records production telemetry.

Body fields:

| Field      | Type   | Required | Description |
|------------|--------|----------|-------------|
| `baseline` | number | no       | Reference value to compare against (e.g., the previous measurement). |
| `metric`   | string | yes      | Metric identifier (e.g., `"latency_p99_ms"`, `"throughput_rps"`, `"binary_size_bytes"`). |
| `unit`     | string | no       | Unit of measure (e.g., `"ms"`, `"req/s"`, `"bytes"`). May be embedded in the metric name; this field is for explicit cases. |
| `value`    | number | yes      | The measured value. |

**Example:**

```json
{"metabox":"1","type":"perf-measurement","subject":"bin/server","issuer":"https://ci.example.com","issuer_type":"tool","created_at":"2026-03-01T10:00:00Z","id":"...","body":{"baseline":42.0,"metric":"latency_p99_ms","unit":"ms","value":47.3}}
```

A regression worth flagging should be expressed as an `annotation` (e.g.,
`kind: "concern"` or `kind: "blocker"`) that may `references` the underlying
measurement record.

### 3.8 Defining New Record Types

Per design principle 3 (Open record types), implementations and integrations
MAY define new record types. New record types are identified by a string
value in the `type` field. Types defined outside this spec SHOULD use a URI
to avoid collisions:

```json
{"metabox":"1","type":"https://example.com/qualifier/build-provenance/v1","subject":"bin/server","issuer":"https://build.example.com","created_at":"...","id":"...","body":{"builder":"github-actions","commit":"abc123"}}
```

Types defined in this spec use short unqualified names (`annotation`,
`epoch`, `dependency`, `license`, `security-advisory`, `perf-measurement`).
The spec reserves all unqualified type names (strings that do not contain
`:` or `/`) for future standardization.

A record type specification MUST define the body fields, their types, and
which are required. Body fields are always serialized in lexicographic
order per MCF.

## 4. Layering Quality Signals on Top

The format itself does not prescribe a numeric model. Annotations carry a
`kind` (with implicit polarity, see §2.7.1) and a free-form body; tools
that want to compute aggregate quality signals layer on top by adding
custom body fields and defining their own evaluation semantics.

This section is an **example** of one such layer. Nothing here is required
of conforming implementations.

### 4.1 Example: A `score` body field

A tool MAY add a `score: integer` field to annotation bodies. Treat
`score` as a signed quality delta — negative for problems, positive for
positives, absent for neutral observations. A reasonable default mapping
follows the polarity table in §2.7.1:

| Kind          | Example default |
|---------------|-----------------|
| `pass`        | +20             |
| `fail`        | -20             |
| `blocker`     | -50             |
| `concern`     | -10             |
| `comment`     | absent          |
| `praise`      | +30             |
| `resolve`     | absent          |
| `suggestion`  | -5              |
| `waiver`      | +10             |

These are illustrative. A tool may pick any range or mapping that suits
its aggregation strategy.

### 4.2 Example: Aggregating across a subject

A tool that defines a `score` body field as above might define a **raw
score** for a subject as the sum of `score` fields of its active
(non-superseded) annotation records, clamped to a chosen range.

When a dependency graph (§3.4) is present, the tool might further define
an **effective score** that propagates negative signals along edges
(e.g., `effective(A) = min(raw(A), min(effective(D) for D in deps(A)))`),
so a problem in a leaf subject lowers the score of everything that
depends on it.

These are choices the tool makes, not invariants of the format. A
different tool might weight by `kind`, decay by age, or ignore signed
deltas entirely in favour of a categorical bar (e.g., "any active
`blocker` fails the build").

### 4.3 Span behaviour

Span-addressed records (§2.4) attach to their `subject`. Whatever
aggregation a tool defines, the span identifies where the signal applies
within the subject; the tool may surface span-level views for display,
but the canonical addressing unit is the subject.

## 5. Interoperability

### 5.1 in-toto Predicate Projection

Qualifier records project losslessly into [in-toto v1 Statement](https://github.com/in-toto/annotation/blob/main/spec/v1/statement.md)
predicates for use with DSSE signing and Sigstore distribution.

**Mapping (annotation):**

```json
{
  "_type": "https://in-toto.io/Statement/v1",
  "subject": [
    {
      "name": "src/parser.rs",
      "digest": {"blake3": "<artifact-content-hash>"}
    }
  ],
  "predicateType": "https://qualifier.dev/annotation/v1",
  "predicate": {
    "qualifier_id": "a1b2c3d4...",
    "kind": "concern",
    "span": {"start": {"line": 42}, "end": {"line": 58}},
    "summary": "Panics on malformed input",
    "tags": ["robustness"],
    "issuer": "mailto:alice@example.com",
    "issuer_type": "human",
    "created_at": "2026-02-25T10:00:00Z",
    "ref": "git:3aba500",
    "supersedes": null
  }
}
```

**Field mapping:**

| Qualifier field | in-toto location |
|----------------|------------------|
| `subject` | `subject[0].name` |
| `body.span` | `predicate.span` |
| `id` | `predicate.qualifier_id` |
| `issuer` | `predicate.issuer` (also DSSE signer) |
| `issuer_type` | `predicate.issuer_type` |
| All body fields | `predicate.*` |

The in-toto `subject[0].digest` contains the content hash of the artifact
file. This is populated by the signing tool, not by qualifier itself.
Qualifier's `id` is the hash of the *record*, not the *artifact*.

**Predicate type URIs:**

| Qualifier type | Predicate type URI |
|---------------|-------------------|
| `annotation` | `https://qualifier.dev/annotation/v1` |
| `epoch` | `https://qualifier.dev/epoch/v1` |
| `dependency` | `https://qualifier.dev/dependency/v1` |

### 5.2 SARIF Import

SARIF v2.1.0 results can be converted to qualifier annotations:

| SARIF field | Qualifier field |
|-------------|----------------|
| `result.locations[0].physicalLocation.artifactLocation.uri` | `subject` |
| `result.locations[0].physicalLocation.region.startLine` | `body.span.start.line` |
| `result.locations[0].physicalLocation.region.startColumn` | `body.span.start.col` |
| `result.locations[0].physicalLocation.region.endLine` | `body.span.end.line` |
| `result.locations[0].physicalLocation.region.endColumn` | `body.span.end.col` |
| `result.ruleId` | `body.kind` (as custom kind) |
| `result.level` | `body.kind` (`error` → `fail`, `warning` → `concern`, `note` → `comment`) |
| `result.message.text` | `body.summary` |
| `run.tool.driver.name` | `issuer` |
| (constant) | `issuer_type: "tool"` (envelope) |

## 6. CLI Interface

The CLI binary is named `qualifier`. Writes go through four verbs:
`record`, `reply`, `resolve`, and `emit`.

**Locations are relative to the current directory; subjects are stored
relative to the project root** (§10). Every location or artifact argument
— `record <location>`, batch `location`/`reply`/`resolve` values,
`reply`/`resolve` targets, `threads` filters, and the `show`, `praise`, and
`compact` artifacts — is joined to the current directory's path below the
project root and normalized (`.` and `..` folded, `/` separators, the root
itself is `.`). An argument that leaves the project root is an error. Every
write lands in a `.qual` file under the project root, laid out as in
§2.10; an explicit `--file` path stays relative to the current directory.
A write into a `.qual` file that the ignore rules hide is refused unless
`--no-ignore` is given (§10.1).

**Exit codes.** Commands exit 0 on success and 1 on an error, which is
printed on stderr as `qualifier: <message>`. Command-line usage errors
(an unknown flag, an invalid `--format` value) and an unknown
`qualifier agents` topic exit 2. `qualifier diff --fail-on` and
`--fail-on-drift` exit 1 after printing the diff (§6.13).

### 6.1 Core Commands

**Setup and agent guide:**

```
qualifier init [--yes] [--dry-run]              Bootstrap VCS merge config and agent directives
qualifier agents [topic]                        Self-contained guide for AI coding agents
```

**Write commands:**

```
qualifier record <kind> <location> [message]    Record an annotation
qualifier reply <target> <message>              Reply to an existing record
qualifier resolve <target> [message]            Resolve (close) an existing record
qualifier emit <type> <subject> --body '<JSON>' Emit a raw record of any type
```

**Inspect commands:**

```
qualifier show <artifact>                 Show annotations for an artifact
qualifier threads [location...]           List conversation threads
qualifier ls [--kind <k>]                 List subjects by kind
qualifier praise <artifact>               Show who annotated an artifact and why
                                          (also available as the `blame` alias)
qualifier review [subject]                Check freshness of annotations
qualifier diff [ref]                      Records added, changed, resolved, or
                                          drifted since a git ref
```

**Maintain commands:**

```
qualifier compact <artifact> [options]    Compact a .qual file (prune/snapshot)
```

### 6.2 `qualifier record`

The unified annotation-write verb. Replaces the old `attest`, `flag`,
`comment`, `suggest`, `approve`, and `reject` commands with a single
shape: `qualifier record <kind> <location> [message] [flags]`.

```
qualifier record concern src/parser.rs:42:58 "Panics on malformed input" \
  --suggested-fix "Use proper error propagation" \
  --tag robustness \
  --tag error-handling \
  --issuer "mailto:alice@example.com"
```

**Arguments:**

| Argument | Meaning |
|----------|---------|
| `<kind>` | One of `concern`, `comment`, `suggestion`, `pass`, `fail`, `blocker`, `praise`, `waiver`, `resolve`. Custom strings are allowed (per spec §2.7.2). |
| `<location>` | Subject path with optional span — see §6.2.1. |
| `[message]` | One-line summary. Becomes `body.summary`. Required in non-interactive mode unless `--stdin` is set. |

**Flags:** `--detail TEXT`, `--ref REF`, `--tag T1 --tag T2 ...`,
`--suggested-fix TEXT`, `--issuer URI`, `--issuer-type {human|ai|tool|unknown}`,
`--file PATH`, `--span SPEC` (overrides any span in `<location>`),
`--supersedes ID`, `--references ID`, `--stdin` (batch JSONL).

`--supersedes` and `--references` each take the full ID (64 lowercase hex
characters) of a record that exists in the project and is live: not
superseded, and not closed by a `resolve`. A prefix or location is
rejected. A superseded target fails, printing the full ID of the live
record at the tip of its chain; a closed target fails, printing the full ID
of the closing `resolve` record. An ID that matches no record is an
error. A `--supersedes` target must have the same subject as the new
record (§2.9). Full IDs are available from
`qualifier threads --format json` (`root.id`, `closed_by.id`),
`qualifier show --format json`, or the `id:` line that
`record`/`reply`/`resolve` print.

A record of kind `resolve` carries at most one `reason:*` tag, and its
value must be one of the `resolve --reason` values (§6.4). The CLI rejects
anything else on every `resolve` it writes: `record resolve …`, a
`"kind":"resolve"` batch line, `reply --kind resolve`, and `resolve`.

**Defaults:**

- When `--issuer` is omitted, defaults to the VCS user identity (see §8.4).
- When a span is given, `content_hash` is auto-computed if the source file
  is readable.

#### 6.2.1 Location and Span Syntax

The `<location>` argument folds the subject and an optional span into a
single string:

| Form | Meaning |
|------|---------|
| `src/parser.rs` | Whole file |
| `src/parser.rs:42` | Line 42 |
| `src/parser.rs:15:28` | Lines 15 through 28 |

The `--span` flag overrides any span parsed from `<location>` and accepts
the same forms plus column granularity:

| Form | Meaning | Equivalent `span` object |
|------|---------|--------------------------|
| `42` | Line 42 | `{"start":{"line":42},"end":{"line":42}}` |
| `42:58` | Lines 42 through 58 | `{"start":{"line":42},"end":{"line":58}}` |
| `42.5:58.80` | Line 42 col 5 through line 58 col 80 | `{"start":{"line":42,"col":5},"end":{"line":58,"col":80}}` |

#### 6.2.2 Batch Mode

`qualifier record --stdin` reads JSONL from stdin. Each line describes one
new record and is one of:

- An overrides object: `{"kind":"...","location":"...","message":"...", ...}`
  with optional `detail`, `ref`, `tags`, `issuer`, `issuer_type`,
  `span`, `supersedes`, `references`, `suggested_fix`. `location` is
  required. Any other key, or a value of the wrong type, fails the line.
- A complete record (envelope + body), accepted for forward-compat and
  recognized by having both `subject` and `body` keys. Its `subject` is
  relative to the project root and must stay inside it; it is normalized
  like a location.

There are no reply or resolve line shapes. A reply is an overrides line
whose `references` is the target's ID; a resolve is an overrides line with
`"kind":"resolve"` whose `supersedes` is the target's ID:

```
{"kind":"comment","location":"src/auth.rs","references":"<id>","message":"Confirmed"}
{"kind":"resolve","location":"src/auth.rs","supersedes":"<id>","message":"Fixed","tags":["reason:fixed"]}
```

`supersedes` and `references` on either line shape follow the same rule as
the `--supersedes`/`--references` flags: the full ID of a live record,
which may be on disk or on an earlier line of the same batch. Only
complete-envelope lines have IDs known in advance (an overrides line is
stamped with the time it is planned), so in practice an in-batch pointer
names an envelope line. A
`"kind":"resolve"` line follows the `reason:*` tag rule above. `--file` is
rejected with `--stdin`.

Without `--continue-on-error`, batch mode is all-or-nothing with respect to
parse and validation failures: every line is parsed and validated before
any record is written, every failing line is reported, and nothing is
written if any line fails that way. This guarantee does not cover I/O
failures while writing: if appending a planned record to disk fails
partway through (e.g., the filesystem fills up), the lines written before
the failure stay written; the error message reports how many. Pass
`--continue-on-error` to collect every parse/validation error, write the
lines that succeeded, and exit non-zero if any line failed.

### 6.3 `qualifier reply`

```
qualifier reply <target> <message>
```

Sugar over "kind=comment + references=`<target-id>`". The default kind is
`comment`; override with `--kind`.

`<target>` is either:

- An **id-prefix**: 4 or more lowercase hex characters. A prefix
  matching more than one record exits non-zero with a disambiguation
  list, one `[id-prefix] kind location "summary"` line per candidate. A
  hex target that matches no ID is tried as a location, and any other
  target (such as `Makefile` or `README`) is a location; or
- A **`<location>`** (e.g., `src/auth.rs:42`). A location resolves to the
  most-recent active record at that subject and span; a `resolve` record is
  never a location target. If multiple active
  records share the most-recent timestamp, exit non-zero with a
  disambiguation list of `[id-prefix] kind L<line> "summary"`.

A target that has been superseded is rejected; the error prints the full
ID of the live record at the tip of its supersession chain. A target whose
chain ends in a `resolve` is rejected as closed; the error prints the full
ID of the closing `resolve` record. To comment on a closed thread, reply to that `resolve` record: the
reply joins the thread (§6.12), which stays closed. To reopen the thread,
record a new non-reply record on the same subject that supersedes the
`resolve` record; it becomes the thread's root.

Same body flags as `qualifier record`. `--supersedes` (for editing an
earlier reply) takes a full, live record ID, as in §6.2.

### 6.4 `qualifier resolve`

```
qualifier resolve <target> [message]
```

Sugar over "kind=resolve + supersedes=`<target-id>`". `<target>` follows
the same id-prefix-or-location rules as `qualifier reply`. The default
summary is "Resolved" when `[message]` is omitted. Superseded and closed
targets are rejected as for `reply`; resolving an already-closed record
fails, naming the closing record.

`--reason fixed|wontfix|duplicate|invalid|obsolete` adds the tag
`reason:<value>`. A resolve carries at most one `reason:*` tag, and its
value must be one of these; the CLI rejects anything else on every
`resolve` it writes (§6.2). It is a tag convention (§2.12), not a body
field.

### 6.5 `qualifier emit`

```
qualifier emit <type> <subject> --body '<JSON>'
```

A raw, script-oriented write for novel or uncommon record types. The
body's fields and values are kept as given in the record's `body` field,
serialized in canonical key order (§2.8.1). For unknown types the record
round-trips via `Record::Unknown`. When `<type>` is `annotation`, the body
is validated against `AnnotationBody`.

```
qualifier emit license src/lib.rs --body '{"spdx_id":"MIT"}' \
  --issuer "https://ci.example.com"

qualifier emit https://example.com/lint/v1 src/parser.rs \
  --body '{"rule":"no-panic","matches":3}'
```

`--stdin` reads JSONL where each line is a complete record. The
positional `<type>` and `<subject>`, when supplied, become defaults
applied to lines missing those fields.

#### 6.5.1 Example Workflow

```bash
# Record a concern at line 42
qualifier record concern src/parser.rs:42 "Panics on malformed input"

# See the concern
qualifier show src/parser.rs

# Reply to it (using ID prefix or location)
qualifier reply a1b2 "Good catch, fixed in latest commit"
qualifier reply src/parser.rs:42 "Good catch, fixed in latest commit"

# Close it
qualifier resolve a1b2

# The concern now shows as a closed thread, with the resolve as its answer
qualifier show src/parser.rs
```

### 6.6 `qualifier show`

```
qualifier show <artifact> [--all] [--pretty] [--type <TYPE>]
               [--format human|json] [--no-ignore]
```

Shows the records on one artifact, grouped into threads (§7,
`qualifier::threads`) and rendered by the same thread renderer as
`threads` and `praise`:

```
qualifier show src/parser.rs

  src/parser.rs

  Open threads (2):
    [c1acc3f5] praise     src/parser.rs  Excellent property test coverage  (bob, 2026-10-01)

    [e5daa3cd] suggestion src/parser.rs:10:12  Consider fuzzing  (alice, ai, 2026-10-01) — needs decision
        [f9156cb5] comment    Worth it for parse()  (carol, 2026-10-01)

  Closed threads (1):
    [274357ca] concern    src/parser.rs:42  Panics on malformed input  (alice, 2026-10-01) — closed (fixed) by alice: Resolved
```

An open thread prints its root, with its state when it is waiting on or
has reached a decision, then one indented line per live reply. A closed
thread prints one line that carries its closing `resolve` (reason, closer,
and summary). Records that are not annotations are listed under "Other
records". Each line ends with the issuer, the issuer type when it is set
and not `human`, and the date.

- `--all` also shows edit history, superseded replies, and superseded
  records.
- `--pretty` prints the source lines around each span
  (compiler-diagnostic style); with `--format json` it adds a `context`
  field to each record.
- `--type <TYPE>` keeps only records whose envelope `type` matches
  (`annotation`, `epoch`, `dependency`, or a custom type URI).
- `--format json` prints `{subject, records, threads}`: the records, and
  one `{origin, root, state, closed_by}` entry per thread, where `state` is
  the thread state of §6.12.

To see the threads on one line range, use `qualifier threads <path>:<line>`.

An artifact with no records is not an error: `show` prints
`No records found for '<artifact>'.` (or an empty `records` list in JSON)
and exits 0.

### 6.7 `qualifier ls`

```
qualifier ls                    # every subject with live records
qualifier ls --kind blocker     # subjects with a live blocker
```

Lists each subject that has live records, with a count. Superseded
records and `resolve` records are not counted. With `--kind`, only
subjects with a live record of that kind are listed, and the count is the
number of such records. JSON output is an array of
`{subject, annotation_count, kinds}`, where `kinds` lists the kind (or,
for other record types, the envelope type) of each of the subject's live records except `resolve`s.

### 6.8 `qualifier compact`

```
qualifier compact src/parser.rs              # prune superseded records
qualifier compact src/parser.rs --snapshot   # collapse to a single epoch
qualifier compact src/parser.rs --dry-run    # preview without writing
qualifier compact --all                      # compact every .qual file
qualifier compact --all --dry-run            # preview repo-wide compaction
```

Implements §3.3. `qualifier compact <artifact>` compacts only that
artifact's records, in every `.qual` file that holds them; other subjects'
records are written back unchanged. `--all` compacts every discovered
`.qual` file. `--snapshot` prunes first and refuses to fold an open
`blocker` or `concern` thread into an epoch unless `--force` is given.
Each file reports its record count before and after, and with
`--snapshot` the number of epochs written. `compact` re-reads each file
strictly before rewriting it and fails on a malformed line rather than
drop it. It has no `--format` flag.

### 6.9 `qualifier review`

Check the freshness of span-addressed annotations against current file content.

```
qualifier review                          # check all annotations
qualifier review src/parser.rs            # one file, or everything under a directory
qualifier review --format json            # machine-readable output
qualifier review --no-ignore              # bypass ignore rules
```

**Human output:**

```
  FRESH    src/parser.rs:42    concern  "Panics on malformed input"
  DRIFTED  src/auth.rs:10:25   suggestion  "Consider using Result"
  MISSING  src/old.rs:1:20     blocker  "Memory leak"

3 annotations checked: 1 fresh, 1 drifted, 1 missing
```

The subject argument is relative to the current directory and matches that
file or anything under that directory. Only active (non-superseded)
annotations with spans that have a `content_hash` are checked. Annotations
without spans or without `content_hash` are skipped; when there is nothing
to check, `review` says so and exits 0.

**JSON output** includes `status` (`fresh`, `drifted`, `missing`) and `detail`
with expected/actual hashes for drifted annotations or a reason for missing ones:
the file is missing or unreadable, is not UTF-8, the span runs past the end
of the file, or the span ends before it starts.

### 6.10 Configuration

Qualifier uses layered configuration. Precedence (highest wins):

| Priority | Source |
|----------|--------|
| 1 (highest) | CLI flags |
| 2 | Environment variables |
| 3 | Project config (`.qualifier.toml` at the project root) |
| 4 | User config (`~/.config/qualifier/config.toml`) |
| 5 (lowest) | Built-in defaults |

**Configuration keys:**

| Key         | CLI flag       | Env var              | Default |
|-------------|----------------|----------------------|---------|
| `issuer`    | `--issuer`     | `QUALIFIER_ISSUER`   | VCS identity (see 8.4) |
| `format`    | `--format`     | `QUALIFIER_FORMAT`   | `human` |

`issuer` is the default issuer of every write command (§8.4). `format`
(`human` or `json`) is the default `--format` of every command that has
the flag. Environment variables are read as strings, and empty ones count
as unset. A malformed config file, or a `format` value other than `human`
or `json`, fails every command with `qualifier: invalid configuration: …`
(exit 1).

### 6.11 `qualifier praise`

Show who recorded annotations against an artifact and why. Available
under the alias `qualifier blame`; the canonical name is `praise` (the
tool tracks who helped, not who to blame). With `--vcs`, delegates to
the underlying VCS blame command for the subject's `.qual` file.

```
qualifier praise src/parser.rs
qualifier praise src/parser.rs --vcs
```

Without `--vcs`, `praise` lists the artifact's threads with the thread
renderer of §6.6, under a `<subject> — N threads (M open)` header. An
artifact with no records prints `No records found for '<artifact>'.` (or
an empty `records` list in JSON) and exits 0.

### 6.12 `qualifier threads`

```
qualifier threads [LOCATION|ID...] [--all] [--kind K[,K]] [--tag T]
                  [--issuer-type TYPE] [--status needs-decision|decided|deferred]
                  [--changed-since REF] [--summary]
                  [--format human|json] [--no-ignore]
```

Lists threads as defined in §7 (`qualifier::threads`). By default only
open threads and live replies are shown; `--all` adds closed threads and
superseded replies. Each argument filters by location or record ID, and a
thread matching any argument is listed:

- A path or directory matches roots on that subject, below it, or on one
  of its ancestor directories (a directory subject that is a proper,
  `/`-bounded prefix of the path: `src/net` and `src` for
  `src/net/tcp.rs`, never `src/ne`).
- `path:start[:end]` matches roots on that file whose span overlaps, roots
  on that file with no span, and roots on its ancestor directories.
- A glob (`*` does not cross `/`) matches root subjects.
- An argument of four or more hex characters (and so no `/`, `.`, or `:`)
  is an ID prefix, matching threads that contain a record whose ID starts
  with it — root, origin, history, replies, or `closed_by`. Write `./cafe`
  to filter on a directory whose name is all hex.

`--tag` matches tags on the root, a live reply, or — under `--all` — the
`closed_by` resolve; `ns:*` matches a namespace; repeated `--tag` flags
must all match. A `--kind` that is neither built in nor carried by any
record prints `qualifier threads: warning: kind '<k>' matches no known
kind` on stderr.

Each thread has a **state**, shared by `threads`, `show`, and `praise`:
`open`; `needs-decision` (open, latest `status:*` tag is
`status:needs-decision`, optionally with an addressee); `decided` (open,
latest `status:*` tag is `status:decided`); or `closed` (with the closing
resolve's `reason:*` value, and `pending_question` when the thread closed
while its latest `status:*` tag was still `status:needs-decision`). In
human output a closed thread is one line carrying its answer, for
example `[274357ca] concern    src/parser.rs:42  Panics on malformed input
— closed (fixed): Resolved`, so truncated output keeps each outcome; a
closed thread selected by ID (under `--all`) also prints its replies and closing resolve.

JSON output is a single array of `{origin, open, state, root, closed_by,
history, replies: [{active, record}], latest_at}` with full IDs, where
`state` is `{"name": ...}` plus `addressee` for `needs-decision` and
`reason`, `closed_by`, and `pending_question` for `closed`. JSON is the
complete view; a closed thread's answer is its `closed_by` record.

`--status needs-decision|decided|deferred` matches the thread's latest
`status:*` tag by `created_at` (an addressee suffix such as
`status:needs-decision:<issuer>` still matches). `--changed-since REF`
keeps threads whose root subject changed between the merge base of HEAD
and REF and the working tree, including untracked files (git only).
`--summary` prints at most two lines — open blockers and concerns on files
changed since `main` (or `master`, or the `--changed-since` ref), and
threads waiting on a decision — and nothing when both counts are zero.
On the base branch, or outside git, the first line counts project-wide and
ends with `(project-wide)`.

### 6.13 `qualifier diff`

```
qualifier diff [REF] [--from-tip] [--fail-on K[,K]] [--fail-on-drift]
               [--kind K[,K]] [--issuer-type TYPE] [--subjects-only]
               [--format human|json] [--no-ignore]
```

Compares the live annotation records in the working tree against the
`.qual` files committed at a git ref (default `main`). Git only.

**Comparison point.** By default the comparison commit is the merge base
of `HEAD` and `REF`, so records that landed on `REF` after the branch
forked count as old, which is what a pull request introduces. `--from-tip`
compares against the tip of `REF`. When `HEAD` and `REF` share no merge
base, the tip is used and a hint is printed on stderr. The human header
and the JSON `comparison` field name the comparison used: `merge-base`,
`tip`, or `fallback-tip`.

**Buckets.** Only annotation records are reported.

- **Added** — live records whose ID is not present at the ref.
- **Changed** — threads open on both sides whose root was edited or
  re-anchored (superseded without being resolved). Matched by thread
  origin (§7), so several edits in a row pair with the root at the ref.
  Each entry shows the new root and what it was at the ref. These records
  appear here instead of under Added and Resolved.
- **Resolved** — records live at the ref that are no longer live, with
  every record that superseded them (more than one after merging branches
  that each closed it), or marked removed when nothing superseded them.
- **Drifted** — records present on both sides whose span `content_hash`
  no longer matches the file in the working tree. Records added on this
  branch are not checked.

`--kind` filters every bucket (a Changed entry matches on its old or new
kind); `--issuer-type` filters by issuer type; `--subjects-only` prints
only the affected subjects, one per line. A `--kind` or `--fail-on` kind
that is neither built in nor carried by any record on either side prints
`qualifier diff: warning: kind '<k>' matches no known kind` on stderr and
the command still runs.

The working tree's ignore rules (§10.1) apply to both sides, so a `.qual`
file that is ignored now is not read at the ref either; `--no-ignore`
reads every `.qual` file on both sides.

**Exit codes.** The diff is printed first. Then `diff` exits 1 when
`--fail-on` is given and Added holds a record of a listed kind, or a
Changed entry's kind moved into the list (`concern` to `blocker`;
rewording or re-anchoring an existing blocker does not count), or when
`--fail-on-drift` is given and Drifted is non-empty. Otherwise it exits 0.

**JSON output:**

```json
{
  "ref": "main",
  "base": "<full SHA of the comparison commit>",
  "from_tip": false,
  "comparison": "merge-base",
  "added":    [<record>],
  "changed":  [{"record": <new root>, "previous": <root at the ref>}],
  "resolved": [{"record": <record at the ref>, "closer": <newest closer or null>, "closers": [<every closer, oldest first>]}],
  "drifted":  [{"record": <record>, "expected": "<hash>", "actual": "<hash>"}]
}
```

`qualifier agents diff` shows the human layout.

### 6.14 `qualifier init`

Bootstraps a project: configures union merges for `.qual` files (git:
`*.qual merge=union` in `.gitattributes`; other VCSes get the §8.2
instructions) and adds a one-line directive pointing AI coding agents at
`qualifier agents` to the agent-instruction files it finds (`AGENTS.md`,
`CLAUDE.md`, and similar), offering to create `AGENTS.md` when there is
none. Each step is skipped when already configured. `--yes` accepts every
step at its default; `--dry-run` reports what would change.

### 6.15 `qualifier agents`

Prints a self-contained guide for AI coding agents, following
[AGENTS-CLI 0.1](AGENTS-CLI.md). With no argument it prints an orientation
page and a topic index; `qualifier agents <topic>` prints one topic. An
unknown topic prints `qualifier agents: no such topic '<topic>'.
Available: <topics>` on stderr and exits 2.

## 7. Library API

The `qualifier` crate exposes its library API from `src/lib.rs`. Library
consumers add `qualifier = { version = "0.9", default-features = false }` to
avoid pulling in CLI dependencies (keep the version at the current minor
release; pre-1.0, each minor release may change this API).

This section lists the **complete supported library surface**. Items not
listed here, including the `qualifier::cli` module (the binary's
implementation, built with the default `cli` feature), are not part of the
API and may change in any release.

```rust
// qualifier::annotation — record types and core logic

/// A typed qualifier record. Dispatches on the `type` field in JSON.
pub enum Record {
    Annotation(Box<Annotation>),
    Epoch(Epoch),
    Dependency(DependencyRecord),
    Unknown(serde_json::Value),  // forward compatibility; serialized in envelope order
}

impl Record {
    pub fn subject(&self) -> &str;
    pub fn id(&self) -> &str;
    pub fn supersedes(&self) -> Option<&str>;   // Annotation only
    pub fn references(&self) -> Option<&str>;   // Annotation only
    pub fn kind(&self) -> Option<&Kind>;        // Annotation only
    pub fn issuer_type(&self) -> Option<&IssuerType>;
    pub fn as_annotation(&self) -> Option<&Annotation>;
    pub fn as_epoch(&self) -> Option<&Epoch>;
    pub fn record_type(&self) -> &str;          // envelope `type`; "" if absent
}
// Record, Annotation, Epoch, DependencyRecord and the body types implement
// Serialize/Deserialize; a .qual line is `serde_json::from_str::<Record>`.

pub struct Annotation {
    pub metabox: String,                    // always "1"
    pub record_type: String,                // "annotation"
    pub subject: String,
    pub issuer: String,
    pub issuer_type: Option<IssuerType>,
    pub created_at: Timestamp,              // hashed as written (§2.8.1 rule 9)
    pub id: String,
    pub body: AnnotationBody,
}

pub struct AnnotationBody {
    pub detail: Option<String>,
    pub kind: Kind,
    pub r#ref: Option<String>,
    pub references: Option<String>,
    pub span: Option<Span>,
    pub suggested_fix: Option<String>,
    pub summary: String,
    pub supersedes: Option<String>,
    pub tags: Vec<String>,
    pub extra: ExtraFields,             // custom body fields, preserved and hashed
}

/// Body fields a record type does not define, keyed by name.
pub type ExtraFields = BTreeMap<String, serde_json::Value>;

pub struct Epoch {
    pub metabox: String,                    // always "1"
    pub record_type: String,                // "epoch"
    pub subject: String,
    pub issuer: String,
    pub issuer_type: Option<IssuerType>,
    pub created_at: Timestamp,
    pub id: String,
    pub body: EpochBody,
}

pub struct EpochBody {
    pub refs: Vec<String>,
    pub span: Option<Span>,
    pub summary: String,
    pub extra: ExtraFields,
}

pub struct DependencyRecord {
    pub metabox: String,                    // always "1"
    pub record_type: String,                // "dependency"
    pub subject: String,
    pub issuer: String,
    pub issuer_type: Option<IssuerType>,
    pub created_at: Timestamp,
    pub id: String,
    pub body: DependencyBody,
}

pub struct DependencyBody {
    pub depends_on: Vec<String>,
    pub extra: ExtraFields,
}

/// RFC 3339 timestamp that keeps the text it was read from. Derefs to
/// DateTime<Utc>; ordered by instant, then text.
pub struct Timestamp { /* private */ }
impl Timestamp {
    pub fn now() -> Timestamp;                         // canonical form
    pub fn parse(text: &str) -> Result<Timestamp, chrono::ParseError>;
    pub fn canonical(instant: DateTime<Utc>) -> String; // UTC, Z, 0/3/6/9 digits
    pub fn as_str(&self) -> &str;                       // as written
    pub fn instant(&self) -> DateTime<Utc>;
}
impl From<DateTime<Utc>> for Timestamp;                // canonical form
// Timestamp also implements Deref<Target = DateTime<Utc>>, Display and
// FromStr (as written), and Serialize/Deserialize (as written).

pub struct Span {
    pub start: Position,
    pub end: Option<Position>,          // normalized to Some(start) before hashing
    pub content_hash: Option<String>,   // BLAKE3 of spanned lines
}
impl Span {
    pub fn end_or_start(&self) -> &Position;
    pub fn normalize(&mut self);        // materialize end = start
}
/// Parse CLI span syntax: "42", "42:58", "42.5:58.80".
pub fn parse_span(s: &str) -> Result<Span, String>;

pub struct Position {
    pub line: u32,               // 1-indexed
    pub col: Option<u32>,        // 1-indexed, optional
}

pub enum Kind { Pass, Fail, Blocker, Concern, Comment, Resolve, Praise, Suggestion, Waiver, Custom(String) }
impl Kind { pub const BUILT_IN: &'static [Kind]; }   // every variant but Custom
pub enum IssuerType { Human, Ai, Tool, Unknown }
// Kind and IssuerType implement Display and FromStr (snake_case names).

pub fn generate_id(annotation: &Annotation) -> String;
pub fn generate_epoch_id(epoch: &Epoch) -> String;
pub fn generate_dependency_id(dep: &DependencyRecord) -> String;
pub fn generate_unknown_id(value: &serde_json::Value) -> String; // custom record types
pub fn generate_record_id(record: &Record) -> String;
pub fn validate(annotation: &Annotation) -> Vec<String>;
pub fn check_supersession_cycles(records: &[Record]) -> Result<()>;      // Err(Error::Cycle)
pub fn validate_supersession_targets(records: &[Record]) -> Result<()>;  // cross-subject
pub fn finalize(annotation: Annotation) -> Annotation;
pub fn finalize_epoch(epoch: Epoch) -> Epoch;
pub fn finalize_record(record: Record) -> Record;

// qualifier::qual_file
pub struct QualFile { pub path: PathBuf, pub subject: String, pub records: Vec<Record> }
pub fn parse(path: &Path) -> Result<QualFile>;                     // strict: first bad line is an error
pub fn parse_lenient(path: &Path) -> Result<(QualFile, Vec<ParseIssue>)>; // skips bad lines
pub struct ParseIssue { pub path: PathBuf, pub line: usize, pub message: String } // Display: "file:line: message"
pub fn parse_str(content: &str) -> Result<Vec<Record>>;            // strict, in memory
pub fn append(path: &Path, record: &Record) -> Result<()>;
pub fn write_all(path: &Path, records: &[Record]) -> Result<()>;    // rewrite a whole file
pub fn find_project_root(start: &Path) -> Option<PathBuf>;          // nearest VCS root
pub fn discover(root: &Path, respect_ignore: bool) -> Result<Vec<QualFile>>; // lenient; warns on stderr

// qualifier::content_hash — span freshness checking
pub fn compute_span_hash(file_path: &Path, span: &Span) -> Result<String, SpanHashError>;
pub enum SpanHashError { NotFound, Io(String), NotUtf8, OutOfRange { start, end, lines }, Reversed { start, end } } // Display + Error
pub enum FreshnessStatus { Fresh, Drifted { expected, actual }, Missing { reason }, NoHash }
pub fn check_freshness(file_path: &Path, span: &Span) -> FreshnessStatus;

// qualifier::compact
pub struct CompactResult { pub before: usize, pub after: usize, pub pruned: usize, pub epochs: usize }
pub fn filter_superseded(records: &[Record]) -> Vec<&Record>;
pub fn prune(qual_file: &QualFile) -> (QualFile, CompactResult);
pub fn prune_subject(qual_file: &QualFile, subject: &str) -> (QualFile, CompactResult);
pub fn snapshot(qual_file: &QualFile) -> (QualFile, CompactResult);
pub fn snapshot_subject(qual_file: &QualFile, subject: &str) -> (QualFile, CompactResult);

// qualifier (crate root)
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Cycle { context: String, detail: String },
    Validation(String),
    AlreadyReported(i32),   // failure already printed on stderr; the binary exits with this status
}
pub type Result<T> = std::result::Result<T, Error>;

// qualifier::threads — group annotations into conversations
pub struct Thread<'a> {
    pub origin: &'a str,                 // oldest record in the root chain
    pub root: &'a Record,                // open: newest non-resolve tip; closed: closed_by's target, else newest non-resolve
    pub closed_by: Option<&'a Record>,   // the closing resolve, if any
    pub replies: Vec<ThreadEntry<'a>>,   // records outside the root chain, oldest first
    pub history: Vec<&'a Record>,        // superseded root-chain records, oldest first
    pub open: bool,
    pub latest_at: DateTime<Utc>,
}
pub struct ThreadEntry<'a> { pub record: &'a Record, pub active: bool }
pub fn build_threads(records: &[Record]) -> Vec<Thread<'_>>;

impl<'a> Thread<'a> {
    pub fn records(&self) -> impl Iterator<Item = &'a Record>;       // root, history, replies, closed_by
    pub fn live_records(&self) -> impl Iterator<Item = &'a Record>;  // root, live replies, closed_by
    pub fn latest_status(&self) -> Option<(&'a str, Option<&'a str>)>; // newest status:* value, addressee
    pub fn without_superseded_replies(self) -> Self;
    pub fn state(&self) -> ThreadState<'a>;
}

/// Where a thread stands (§6.12).
pub enum ThreadState<'a> {
    Open,
    NeedsDecision { addressee: Option<&'a str> },
    Decided,
    Closed { reason: Option<&'a str>, closer: &'a Record, pending_question: bool },
}
impl ThreadState<'_> {
    pub fn name(&self) -> &'static str;          // "open", "needs-decision", "decided", "closed"
    pub fn to_json(&self) -> serde_json::Value;  // the `state` object of threads JSON
}

/// Threads with any record on `subject`, open first; drops superseded replies unless `all`.
pub fn threads_touching<'a>(records: &'a [Record], subject: &str, all: bool) -> Vec<Thread<'a>>;
/// `<command>: warning: kind 'X' matches no known kind` for each requested
/// custom kind that no record carries.
pub fn unknown_kind_warnings<'r>(command: &str, requested: &[Kind],
    records: impl IntoIterator<Item = &'r Record>) -> Vec<String>;

/// The human thread renderer shared by `threads`, `show`, and `praise`.
pub struct ThreadRenderer<'f> {
    pub all: bool,             // also print edit history and superseded replies
    pub expand_closed: bool,   // print a closed thread's replies and closing resolve
    pub attribution: bool,     // append (issuer, issuer type, date) to each line
    pub continuation: Option<&'f Continuation<'f>>, // extra lines under a record
}
pub type Continuation<'f> = dyn Fn(&Record) -> Vec<String> + 'f;
impl ThreadRenderer<'_> { pub fn render(&self, t: &Thread<'_>) -> Vec<String>; }

pub fn thread_json(t: &Thread<'_>) -> serde_json::Result<serde_json::Value>; // one element of threads JSON
pub fn thread_summary_json(t: &Thread<'_>) -> serde_json::Value;            // {origin, root, state, closed_by}
pub fn kind_label(r: &Record) -> String;   // kind, or envelope type for non-annotations
pub fn location(r: &Record) -> String;     // subject, subject:line, or subject:start:end
pub fn short_id(id: &str) -> &str;         // first 8 characters
pub fn short_issuer(issuer: &str) -> &str; // mailto:alice@example.com -> alice
```

A thread starts at an origin annotation. Records join it through
`references` (replies) or `supersedes` (edits and resolutions). The root
chain is the origin plus the non-reply records that supersede it in turn.
A chain member no other chain member supersedes is a tip (a chain can fork
into more than one). If any tip is not a `resolve`, the thread is open and
its root is the newest such tip; otherwise the thread is closed by the
newest resolve tip, and the root is the non-resolve chain member that
resolve targets, falling back to the newest non-resolve chain member.
Resolving a reply does not close the thread. `history` holds the root
chain's other members, oldest first. Non-annotation records are ignored.

The library is the source of truth. The CLI is a thin wrapper around it.

## 8. VCS Integration

`.qual` files SHOULD be committed to version control. Qualifier is VCS-agnostic
— the append-only JSONL format is friendly to any system that tracks text files.

### 8.1 General Principles

- Append-only JSONL minimizes merge conflicts.
- Pre-compaction history is recoverable from VCS history.
- For collaborative repositories, configure your VCS to use union merges
  on `.qual` files so concurrent appends don't collide.

### 8.2 VCS-Specific Setup

| VCS        | Configuration |
|------------|---------------|
| Git        | Add `*.qual merge=union` to `.gitattributes` |
| Mercurial  | Add `**.qual = :union` under `[merge-patterns]` in `.hg/hgrc` |
| Jujutsu    | No per-path merge configuration; resolve `.qual` conflicts by keeping both sides' lines |
| Other      | Configure equivalent union-merge behaviour for `*.qual` |

Run `qualifier init` to apply the git configuration interactively
(or non-interactively with `--yes`; preview with `--dry-run`). For other
VCSes it prints the relevant row of this table.

### 8.3 `qualifier blame`

Delegates to the underlying VCS blame/annotate command:

- Git: `git blame`
- Mercurial: `hg annotate`
- Fallback: not available (prints guidance)

### 8.4 Issuer Defaults

Each value resolves in order: explicit flag, `QUALIFIER_*` environment
variable, the `issuer` key of the config files (§6.10; issuer only),
detected agent harness, then the fallback below. Empty variables count as
unset. Harness detection never sets the issuer.

| value | flag | variable | config | harness (Claude Code: `CLAUDECODE=1`) | fallback |
|---|---|---|---|---|---|
| issuer | `--issuer` | `QUALIFIER_ISSUER` | `issuer` | — | `git config user.email`, then `hg config ui.username`, then `mailto:$USER@localhost` |
| issuer type | `--issuer-type` | `QUALIFIER_ISSUER_TYPE` | — | `ai` | none |
| session tag | — | `QUALIFIER_SESSION` | — | `claude-code:$CLAUDE_CODE_SESSION_ID` | none |

When a session is known, `record`, `reply`, and `resolve` add the tag
`session:<value>`. `emit` applies the issuer defaults but writes bodies
as given. A human running `qualifier` inside an agent harness is detected
as the agent; pass `--issuer-type human` to override.

## 9. Agent Integration

Qualifier is designed to be used by AI coding agents. Key affordances:

- **Structured output:** `--format json` on every read command and on
  `record`, `reply`, and `resolve`; set `format = "json"` in config or
  `QUALIFIER_FORMAT=json` to make it the default (§6.10).
- **Batch annotation:** `qualifier record --stdin` reads JSONL from stdin
  (overrides objects or full records). For non-annotation record types,
  `qualifier emit --stdin` accepts complete records. A batch reply or
  resolve is a record line whose `references` or `supersedes` is the
  target's full ID (from `qualifier threads --format json`); without
  `--continue-on-error` a batch writes nothing unless every line validates.
- **Suggested fixes:** The `suggested_fix` body field gives agents a concrete
  action to take.
- **Span precision:** The `span` body field lets agents target specific line
  ranges, making annotations actionable without hunting for the relevant code.
- **Filtering by kind:** `qualifier ls --kind blocker --format json` gives
  agents a worklist of issues to address.
- **Continuous interaction:** `qualifier reply <id> <message>` lets agents
  respond to human signals with threaded follow-ups. `qualifier resolve <id>`
  closes a thread; an agent resolves only within close authority
  (`qualifier agents conventions`) and otherwise replies and leaves the
  close to a human.
- **Threading:** The `references` field enables agents to thread follow-up
  observations to prior signals, creating navigable conversation histories.
- **Thread queries:** `qualifier threads --format json` lists every open
  thread with its live replies and its `state`; `--all` adds closed threads,
  whose answer is the `closed_by` record; `--status needs-decision` lists
  threads waiting on a human.
- **Branch review:** `qualifier diff --format json` lists what a branch
  added, changed, resolved, and drifted (§6.13); `--fail-on blocker`
  gates CI.
- **Provenance:** records written inside a detected agent harness default to
  `issuer_type: ai` and carry a `session:` tag (§8.4).
- **Conventions:** `qualifier agents conventions` defines the `status:`,
  `reason:`, `session:`, `revisit:`, and `depends-on:` tag vocabulary and
  close authority.
- **Claude Code plugin:** `plugins/claude-code/` (`qual`) packages lifecycle
  skills and a SessionStart hook on top of these affordances.

## 10. File Discovery

Qualifier discovers `.qual` files by walking the directory tree from the
project root. Each `.qual` file may contain records for multiple subjects
and multiple record types.

The project root is determined by searching upward for VCS markers (`.git`,
`.hg`, `.jj`, `.pijul`, `_FOSSIL_`, `.svn`).

### 10.1 Ignore Rules

By default, qualifier respects ignore rules from three sources during file
discovery, under every VCS (§10), not only in git repositories:

1. **`.gitignore`** — Standard Git ignore files, including:
   - `.gitignore` files at any level of the tree
   - `.git/info/exclude` (per-repo excludes, in git repositories)
   - The global gitignore file (e.g., `~/.config/git/ignore`)
   - `.gitignore` files in parent directories above the project root
     (matching Git's own behavior in monorepos)

2. **`.ignore`** — Generic ignore files in `.gitignore` syntax, as read by
   tools such as ripgrep.

3. **`.qualignore`** — A qualifier-specific ignore file using the same
   syntax as `.gitignore`. Place a `.qualignore` file anywhere in the tree
   to exclude paths from qualifier's discovery walk. Useful for ignoring
   vendored code, generated files, or example directories that have `.qual`
   files you want qualifier to skip without affecting Git.

Paths matched by any source are excluded from every command that discovers
`.qual` files: `show`, `threads`, `ls`, `praise`/`blame`, `review`, `diff`
(on both sides of the comparison), `compact`, and the ID-prefix and
`--supersedes`/`--references` lookups of the write commands.

Writes are checked against the same rules: `record`, `reply`, `resolve`,
and `emit` refuse to write into a `.qual` file that discovery would skip,
naming the rule that hides it, since no command would read the record.
Pass `--no-ignore` to write it anyway.

Discovery reads each `.qual` file leniently: a line that is not a valid
record is skipped with a `warning: skipping <file>:<line>: <reason>` on
stderr, and the other records still load. `compact` re-reads the files it
rewrites strictly and fails on such a line rather than drop it.

### 10.2 `--no-ignore`

Pass `--no-ignore` to any discovery command to bypass all ignore rules.
This forces qualifier to walk every directory except VCS metadata
directories (§10.3) and discover all `.qual` files regardless of
`.gitignore`, `.ignore`, or `.qualignore` entries.

### 10.3 Hidden Directories

VCS metadata directories (`.git`, `.hg`, `.jj`, `.pijul`, `_FOSSIL_`,
`.svn`) are always skipped during discovery, regardless of ignore settings.

Other hidden directories (names starting with `.`, such as `.github`) are
walked like any other directory, so artifacts like
`.github/workflows/ci.yml` can carry records. Tool directories that should
not be walked (`.venv`, `.cache`, and so on) are excluded through
`.gitignore` or `.qualignore`.

Hidden *files* (like `.qual`) are not skipped — the per-directory `.qual`
layout depends on this.

## 11. Crate Structure

A single crate published as `qualifier` on crates.io.

```
qualifier/
├── Cargo.toml
├── SPEC.md                    # This document
├── METABOX.md                 # Metabox envelope specification
├── AGENTS-CLI.md              # AGENTS-CLI protocol (qualifier agents)
└── src/
    ├── lib.rs                 # Public library API (§7)
    ├── annotation.rs          # Record types, body structs, Kind, IssuerType, IDs, validation
    ├── content_hash.rs        # Span content hashing and freshness checking
    ├── qual_file.rs           # .qual file parsing, appending, discovery
    ├── compact.rs             # Compaction: prune and snapshot, supersession filtering
    ├── threads.rs             # Thread assembly, thread state, thread rendering
    ├── bin/
    │   └── qualifier.rs       # Binary entry point
    └── cli/                   # CLI module (behind "cli" feature; not library API)
        ├── mod.rs             # Argument parsing, grouped --help, dispatch
        ├── config.rs          # .qualifier.toml / user config / QUALIFIER_* (§6.10)
        ├── output.rs          # --format human|json
        ├── provenance.rs      # Issuer, issuer type, and session defaults (§8.4)
        ├── span_context.rs    # Source context around spans
        ├── targets.rs         # Location and ID-prefix resolution, write paths
        └── commands/
            ├── mod.rs
            ├── init.rs           # qualifier init
            ├── agents/           # qualifier agents (mod.rs + pages/*.md topics)
            ├── record.rs         # qualifier record (unified annotation write)
            ├── reply.rs          # qualifier reply (id-prefix or location)
            ├── resolve.rs        # qualifier resolve (id-prefix or location)
            ├── emit.rs           # qualifier emit (raw record write)
            ├── show.rs
            ├── threads.rs
            ├── ls.rs
            ├── praise.rs         # qualifier praise (alias: blame)
            ├── freshness.rs      # qualifier review (freshness checking)
            ├── diff.rs           # qualifier diff
            ├── compact.rs
            └── haiku.rs
```

```toml
[features]
default = ["cli"]
cli = ["dep:clap", "dep:comfy-table", "dep:figment", "dep:gix", "dep:globset", "dep:rand", "dep:terminal_size"]
```

## 12. Future Considerations (Out of Scope)

These are explicitly **not** part of the current release but are anticipated:

- **First-class scoring layer:** A built-in implementation of the example
  scoring model in §4 (`qualifier score`, `qualifier check`, dependency
  propagation), gated behind a feature flag.
- **Dependency graph engine:** A built-in graph (`qualifier graph` for
  visualization, plus traversal helpers used by the scoring layer above).
  Dependency *records* (§3.4) remain in the wire format today; the engine
  was yanked alongside scoring.
- **Policy records** (`type: "policy"`): Project-level rules, required kinds,
  and gate criteria — expressed as records in the same stream.
- **Editor plugins:** LSP-based inline display of annotations, with
  span-aware gutter annotations.
- **DSSE signing:** `qualifier sign` to wrap records in DSSE envelopes for
  supply-chain distribution via Sigstore.
- **`qualifier import-sarif`:** First-class SARIF import command.
- **`qualifier rename`:** Automated subject rename with `.qual` file and
  dependency migration.
- **`qualifier watch`:** File-watcher mode for continuous scoring.
- **Remote aggregation:** Qualifier servers for cross-repository views.

---

*The Koalafier has spoken. Now go qualify some code.*
