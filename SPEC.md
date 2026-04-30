# Qualifier Specification

**Version:** 0.3.0-draft
**Status:** Draft
**Authors:** Alex Kesling

---

## Abstract

Qualifier is a deterministic system for recording, propagating, and querying
typed metadata records against software artifacts. It provides a VCS-friendly
file format (`.qual`), a Rust library (`libqualifier`), and a CLI binary
(`qualifier`) that together enable humans and agents to annotate code with
structured quality signals — scored or unscored — without waiting for a
formal process. Scored signals (concerns, passes, blockers) carry numeric
deltas that propagate through dependency graphs. Unscored signals (comments,
observations) capture knowledge as first-class records. Both thread, persist,
and compose through the same model.

Records use the [Metabox](METABOX.md) envelope format: a fixed envelope
(`metabox`, `type`, `subject`, `issuer`, `issuer_type`, `created_at`, `id`)
wrapping a type-specific `body` object. Records are content-addressed, append-only, and
human-writable. No server, no database, no PKI required.

## 1. Design Principles

1. **Files are the API.** The `.qual` format is the primary interface. Every
   tool — CLI, editor plugin, CI bot, coding agent — reads and writes the same
   files. No server, no database, no lock-in.

2. **VCS-native.** `.qual` files are append-only JSONL. They merge cleanly,
   diff readably, and blame usefully. Conflicts are structurally impossible
   under normal workflows (append-only + file-per-artifact).

3. **Deterministic scoring.** Given identical `.qual` files and an identical
   dependency graph, every implementation MUST produce identical quality scores.
   No floating point, no random weights — just deterministic integer arithmetic.

4. **Propagation through the graph.** Quality is more than local. Software has
   dependencies. An artifact's *effective* quality is a function of its own
   annotations AND the effective quality of everything it depends on. A
   pristine binary that links a cursed library inherits the curse.

5. **Human-first, agent-friendly.** The CLI is designed for humans at a
   terminal. The JSONL format and library API are designed for agents and
   tooling. Both are first-class.

6. **Composable.** The record format uses the Metabox envelope — a uniform
   frame (who said something about which subject) wrapping typed payloads
   (what they said). New record types extend the system without changing the
   envelope. Unknown types pass through harmlessly.

7. **Interoperable.** Qualifier records project losslessly into in-toto
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
| `created_at`   | string   | yes      | RFC 3339 timestamp |
| `id`           | string   | yes      | Content-addressed BLAKE3 hash (see 2.8) |
| `body`         | object   | yes      | Type-specific payload (see 2.6, 3.2, 3.4) |

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
| `content_hash` | string | no       | BLAKE3 hash of the spanned lines (see 2.4.4) |

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

#### 2.4.4 Content Hashing

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
addressed annotations (via `flag`, `suggest`, `comment`, `approve`, `reject`,
`attest --span`, etc.) if the subject file exists and the span is within
bounds. If the file does not exist or the span extends beyond EOF, `content_hash`
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

#### 2.4.3 Span Scoring

Span-addressed records contribute to the score of their parent **subject**.
An annotation about `src/parser.rs` at span `{start: {line: 42}, end: {line: 58}}`
contributes to the raw score of `src/parser.rs`, not to a separate span-level
score.

Spans are addressing granularity, not scoring granularity. They tell you
*where* within the subject a signal applies but do not create separate scoring
targets.

> **Rationale.** Span-level scoring would be extremely noisy for most
> workflows. Subject-level aggregation is the right default. Future
> extensions MAY introduce opt-in span-level scoring.

### 2.5 Record Types

The `type` field is a string that identifies the body schema. Implementations
MUST support the following types:

| Type            | Description |
|-----------------|-------------|
| `annotation`   | A quality signal (see 2.6) |
| `epoch`         | A compaction snapshot (see 3.2) |
| `dependency`    | A dependency edge (see 3.4) |

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
| `references`    | string   | no       | ID of a related record (see 2.11). No scoring impact. |
| `score`         | integer  | no       | Signed quality delta, -100..100. Present on scored kinds (concern, pass, etc.); absent on unscored kinds (comment). The CLI uses the kind's default score when `--score` is omitted. |
| `span`          | object   | no       | Sub-artifact range (see 2.4) |
| `suggested_fix` | string   | no       | Actionable suggestion for improvement |
| `summary`       | string   | yes      | Human-readable one-liner |
| `supersedes`    | string   | no       | ID of a prior record this replaces (see 2.9) |
| `tags`          | string[] | no       | Freeform classification tags |

Body fields are listed in alphabetical order, which matches the Metabox
Canonical Form (MCF) serialization order.

**Example:**

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-25T10:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","ref":"git:3aba500","score":-10,"span":{"start":{"line":42},"end":{"line":58}},"suggested_fix":"Use the ? operator instead of unwrap()","summary":"Panics on malformed input","tags":["robustness"]}}
```

**Shorthand (equivalent):** Since `type` defaults to `"annotation"`, it may
be omitted:

```json
{"metabox":"1","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-02-25T10:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","score":-10,"summary":"Panics on malformed input"}}
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

#### 2.7.1 Recommended Score Ranges

The following table provides RECOMMENDED default scores for each kind.
Implementations and users MAY deviate, but SHOULD maintain sign consistency:
positive kinds SHOULD have positive scores, and negative kinds SHOULD have
negative scores.

| Kind          | Default Score | Recommended Range | Sign |
|---------------|---------------|-------------------|------|
| `pass`        | +20           | +10 to +50        | positive |
| `fail`        | -20           | -10 to -50        | negative |
| `blocker`     | -50           | -30 to -100       | negative |
| `concern`     | -10           | -5 to -30         | negative |
| `comment`     | absent        | N/A               | neutral |
| `praise`      | +30           | +10 to +50        | positive |
| `resolve`     | 0             | 0                 | neutral |
| `suggestion`  | -5            | -5 to -15         | negative |
| `waiver`      | +10           | 0 to +30          | positive |

When `--score` is omitted from `qualifier attest`, the CLI SHOULD use the
default score for the given kind. `--score` always takes precedence.

These are guidance, not constraints. Implementations MUST NOT reject an
annotation solely because its score falls outside the recommended range.

#### 2.7.2 Custom Kinds

Any string is a valid `kind`. Implementations SHOULD detect likely typos
(edit distance <= 2 from a built-in kind) and warn the user.

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

3. **Body field order.** Body fields MUST appear in lexicographic
   (alphabetical) order. Nested objects (like `span`) also have their fields
   in lexicographic order.

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

See the [Metabox specification](METABOX.md) for the full MCF definition.

#### 2.8.2 Example

Given an annotation with no optional body fields, the MCF is:

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-02-24T10:00:00Z","id":"","body":{"kind":"concern","score":-30,"summary":"Panics on malformed input"}}
```

With a span and issuer_type:

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-24T10:00:00Z","id":"","body":{"kind":"concern","score":-30,"span":{"start":{"line":42},"end":{"line":42}},"summary":"Panics on malformed input"}}
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
- When computing scores, a superseded record MUST be excluded. Only the tip
  of each chain contributes.
- Dangling `supersedes` references (pointing to IDs not present in the current
  file set) are allowed. The referencing record remains active.

**Resolve pattern:** A `resolve`-kind annotation supersedes its target,
withdrawing the target's score from the raw total. This is the canonical way
to close an issue — the resolve record carries a score of 0 and the
superseded record is excluded from scoring, so the net effect is removal of
the original signal.

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
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-24T10:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","ref":"git:3aba500","score":-30,"span":{"start":{"line":42},"end":{"line":58}},"suggested_fix":"Replace .unwrap() with proper error propagation","summary":"Panics on malformed UTF-8 input","tags":["robustness","error-handling"]}}
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:bob@example.com","issuer_type":"human","created_at":"2026-02-24T11:00:00Z","id":"e5f6a7b8...","body":{"kind":"praise","score":40,"summary":"Excellent property-based test coverage","tags":["testing"]}}
```

### 2.11 References

The `references` body field provides a lightweight "re:" pointer from one
annotation to another. Unlike `supersedes` (which removes the referenced
record from scoring), `references` is purely informational — both the
original and the referencing record contribute independently to scores.

**Semantics:**

- A `references` value is a single record ID string.
- The referenced record is NOT filtered from scoring. Both records remain
  active and contribute their scores independently.
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
Implementations SHOULD display these as threaded conversations with
tree-drawing characters (`├──`, `└──`). Reply depth is unbounded — a reply
to a reply is a valid thread.

**Example:**

```json
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:bob@example.com","created_at":"2026-03-01T10:00:00Z","id":"b2c3d4e5...","body":{"kind":"comment","references":"a1b2c3d4...","score":0,"summary":"This was addressed in the latest refactor"}}
```

**Full lifecycle example (flag → reply → resolve):**

```jsonl
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-03-01T09:00:00Z","id":"a1b2c3d4...","body":{"kind":"concern","score":-10,"span":{"start":{"line":42}},"summary":"Panics on malformed input"}}
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:bob@example.com","created_at":"2026-03-01T10:00:00Z","id":"b2c3d4e5...","body":{"kind":"comment","references":"a1b2c3d4...","summary":"Good catch — fixed in latest commit"}}
{"metabox":"1","type":"annotation","subject":"src/parser.rs","issuer":"mailto:alice@example.com","created_at":"2026-03-01T11:00:00Z","id":"c3d4e5f6...","body":{"kind":"resolve","score":0,"summary":"Resolved","supersedes":"a1b2c3d4..."}}
```

After the resolve, the original concern's `-10` is withdrawn from scoring.
The reply remains visible in the thread for context.

## 3. Record Type Specifications

### 3.1 Annotation (`type: "annotation"`)

Defined in section 2.6. This is the primary record type.

### 3.2 Epoch (`type: "epoch"`)

An **epoch** is a synthetic compaction summary produced by the compactor. It
replaces a set of annotations with a single record that preserves the net
score.

Body fields (alphabetical):

| Field         | Type     | Required | Description |
|---------------|----------|----------|-------------|
| `refs`        | string[] | yes      | IDs of the compacted records |
| `score`       | integer  | yes      | Raw score at compaction time |
| `span`        | object   | no       | Sub-artifact range |
| `summary`     | string   | yes      | `"Compacted from N records"` |

Epoch records MUST set `issuer` to `"urn:qualifier:compact"` and
`issuer_type` to `"tool"` (in the envelope).

**Example:**

```json
{"metabox":"1","type":"epoch","subject":"src/parser.rs","issuer":"urn:qualifier:compact","issuer_type":"tool","created_at":"2026-02-25T12:00:00Z","id":"f9e8d7c6...","body":{"refs":["a1b2...","c3d4..."],"score":10,"summary":"Compacted from 12 records"}}
```

Epoch records are treated as normal scored records by the scoring engine. The
`refs` field exists solely for auditability — it lets you trace back (via VCS
history) to the individual records that were folded in.

### 3.3 Compaction

Append-only files grow without bound. **Compaction** is the mechanism for
reclaiming space while preserving scoring correctness.

A compaction rewrites a `.qual` file by:

1. **Pruning** all superseded records. If record B supersedes A, only B is
   retained. The entire chain collapses to its tip.
2. **Optionally snapshotting.** When `--snapshot` is passed, all surviving
   records for each subject are replaced by a single epoch record.

#### 3.3.1 Compaction Rules

- Compaction MUST NOT change the raw score of any subject. This is the
  invariant. If compaction changes a score, the implementation has a bug.
- Compaction MUST be explicit and user-initiated — never automatic or silent.
- Compaction MUST preserve records of unrecognized types (they are opaque
  pass-through).
- After compaction, the file is a valid `.qual` file. No special reader
  support is needed.
- `qualifier compact --dry-run` MUST be supported.

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

The dependency graph MUST be a DAG. Implementations MUST detect and reject
cycles.

#### 3.4.1 Dependency Graph Sources

Qualifier accepts dependency information from two sources:

1. **Dependency records in `.qual` files** — as defined above.
2. **Legacy graph file** — a standalone JSONL file (conventionally
   `qualifier.graph.jsonl`) with simplified dependency declarations:

```jsonl
{"subject":"bin/server","depends_on":["lib/auth","lib/http","lib/db"]}
{"subject":"lib/auth","depends_on":["lib/crypto"]}
```

Both sources are merged when computing effective scores. When both declare
edges for the same subject, the union of all `depends_on` arrays is used.

### 3.5 Defining New Record Types

New record types are identified by a string value in the `type` field. Types
defined outside this spec SHOULD use a URI to avoid collisions:

```json
{"metabox":"1","type":"https://example.com/qualifier/license/v1","subject":"src/parser.rs","issuer":"https://license-scanner.example.com","created_at":"...","id":"...","body":{"license":"MIT"}}
```

Types defined in this spec use short aliases (`annotation`, `epoch`,
`dependency`). The spec reserves all unqualified type names (strings that
do not contain `:` or `/`) for future standardization.

A record type specification MUST define:

1. The body fields, their types, and which are required.
2. How the type interacts with scoring (if at all).

Body fields are always serialized in lexicographic order per MCF.

## 4. Scoring

### 4.1 Raw Score

The **raw score** of a subject is the sum of the `score` fields of all
non-superseded annotation and epoch records for that subject, clamped to
`[-100, 100]`.

```
raw_score(A) = clamp(-100, 100, sum(record.body.score for active scored records of A))
```

A subject with no scored records has a raw score of **0** (unqualified).

Only records of types that carry a `score` field (`annotation`, `epoch`)
contribute to scoring. Dependency records and unknown types do not.

### 4.2 Effective Score

The **effective score** of a subject is a function of its raw score and the
effective scores of its dependencies:

```
effective_score(A) = min(raw_score(A), min(effective_score(D) for D in deps(A)))
```

Your effective score can never be higher than your worst dependency's effective
score. Quality flows downhill.

If a subject has no dependencies, its effective score equals its raw score.

If a subject has no scored records but has dependencies, its effective score
is the minimum effective score of its dependencies (the "inherited floor").

### 4.3 Span Scoring

Span-addressed records contribute to the raw score of their `subject`. The
span is informational — it identifies where a signal applies within the
subject, but scoring aggregates at the subject level.

This means `qualifier score src/parser.rs` reports one score for the file,
even if individual annotations target different line ranges.

Implementations MAY offer span-level filtering for display (e.g.,
`qualifier show src/parser.rs --line 42` shows only annotations whose spans
overlap line 42), but this is a presentation concern, not a scoring concern.

### 4.4 Score Status

Implementations SHOULD report a human-readable status for each subject:

| Condition | Status |
|-----------|--------|
| effective < 0 | `blocker` |
| effective = 0, limited by dependency | `unqualified (limited)` |
| effective = 0 | `unqualified` |
| effective >= 60, limited by dependency | `healthy (limited)` |
| effective >= 60 | `healthy` |
| limited by dependency | `ok (limited)` |
| otherwise | `ok` |

A subject is "limited" when its effective score is lower than its raw score
due to a dependency constraint.

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
    "score": -10,
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
| `result.level` | `body.score` (see mapping below) |
| `result.message.text` | `body.summary` |
| `run.tool.driver.name` | `issuer` |
| (constant) | `issuer_type: "tool"` (envelope) |

**Level-to-score mapping:**

| SARIF level | Default score |
|-------------|---------------|
| `error` | -20 |
| `warning` | -10 |
| `note` | -5 |
| `none` | 0 |

Implementations providing SARIF import SHOULD allow users to override these
defaults.

## 6. CLI Interface

The CLI binary is named `qualifier`.

### 6.1 Core Commands

**Signal commands:**

```
qualifier comment <location> <message>    Add a comment
qualifier flag <location> <message>       Flag a concern
qualifier suggest <location> <message>    Suggest a change
qualifier approve <location> <message>    Approve an artifact
qualifier reject <location> <message>     Reject an artifact
qualifier reply <id-prefix> <message>     Reply to a record
qualifier resolve <id-prefix> [message]   Resolve a record
```

**Analysis commands:**

```
qualifier show <artifact>                 Show annotations and scores
qualifier score [artifact...]             Compute and display scores
qualifier ls [--below <n>] [--kind <k>]   List subjects by score/kind
qualifier check [--min-score <n>]          CI gate: exit non-zero if below threshold
qualifier review [subject]                Check freshness of annotations
```

**Management commands:**

```
qualifier attest <artifact> [options]     Add an annotation (low-level)
qualifier compact <artifact> [options]     Compact a .qual file (prune/snapshot)
qualifier graph [--format dot|json]        Visualize the dependency graph
qualifier init                             Initialize qualifier in a repo
qualifier blame <artifact>                 Per-line VCS attribution for a .qual file
```

### 6.2 `qualifier attest`

Interactive and non-interactive annotation creation.

```
qualifier attest src/parser.rs \
  --kind concern \
  --score -30 \
  --summary "Panics on malformed input" \
  --suggested-fix "Use proper error propagation" \
  --tag robustness \
  --tag error-handling \
  --issuer "mailto:alice@example.com" \
  --span 42:58
```

#### 6.2.1 Span Syntax

The `--span` flag accepts the following forms:

| Form | Meaning | Equivalent `span` object |
|------|---------|--------------------------|
| `42` | Line 42 | `{"start":{"line":42},"end":{"line":42}}` |
| `42:58` | Lines 42 through 58 | `{"start":{"line":42},"end":{"line":58}}` |
| `42.5:58.80` | Line 42 col 5 through line 58 col 80 | `{"start":{"line":42,"col":5},"end":{"line":58,"col":80}}` |

When `--span` is omitted, no span is set (the annotation addresses the whole
subject).

#### 6.2.2 Other Flags

`--summary` is required in non-interactive mode.

When `--score` is omitted, the CLI uses the recommended default score for the
given kind (see section 2.7.1).

`--file <path>` writes the annotation to a specific `.qual` file instead
of using the default layout resolution.

When `--issuer` is omitted, defaults to the VCS user identity (see 8.4).

### 6.3 Signal Commands

The signal commands are thin wrappers around `qualifier attest` that provide
ergonomic, zero-ceremony entry points for recording quality signals.

#### 6.3.1 Location Syntax

Signal commands accept a `<location>` argument:

| Form | Meaning |
|------|---------|
| `src/parser.rs` | Whole file |
| `src/parser.rs:42` | Line 42 |
| `src/parser.rs:15:28` | Lines 15 through 28 |

#### 6.3.2 Signal Command Details

**`qualifier comment <location> <message>`** — Creates an annotation with
`kind: "comment"`. An unscored signal — observations, discussion points, questions.

**`qualifier flag <location> <message>`** — Creates an annotation with
`kind: "concern"` and the default concern score (-10).

**`qualifier suggest <location> <message>`** — Creates an annotation with
`kind: "suggestion"` and the default suggestion score (-5). Use `--suggested-fix`
for actionable remediation text.

**`qualifier approve <location> <message>`** — Creates an annotation with
`kind: "pass"` and the default pass score (+20).

**`qualifier reject <location> <message>`** — Creates an annotation with
`kind: "fail"` and the default fail score (-20).

**`qualifier reply <id-prefix> <message>`** — Creates an annotation with
`kind: "comment"` and `references` pointing to the target record's ID. The
ID prefix must be at least 4 characters and must resolve unambiguously within
the subject's `.qual` file.

**`qualifier resolve <id-prefix> [message]`** — Creates an annotation with
`kind: "resolve"`, `supersedes` pointing to the target record, and a default
summary of "Resolved". Withdraws the target's score from the raw total.

#### 6.3.3 Example Workflow

```bash
# Flag a concern at line 42
qualifier flag src/parser.rs:42 "Panics on malformed input"

# See the flag
qualifier show src/parser.rs

# Reply to it (using ID prefix)
qualifier reply a1b2 "Good catch, fixed in latest commit"

# Close it
qualifier resolve a1b2

# Negative score is gone
qualifier score
```

### 6.4 `qualifier show`

```
qualifier show src/parser.rs

  src/parser.rs
  Raw score:       10
  Effective score: -20 (limited by lib/crypto)

  Records (4):
    [-30] concern  L42–58 "Panics on malformed input"    alice  2026-02-24  a1b2c3d4
    ├── [ 0] comment       "Good catch, fixed"           bob    2026-02-25  b2c3d4e5
    └── [ 0] resolve       "Resolved"                    alice  2026-02-25  c3d4e5f6
    [+40] praise          "Excellent property test coverage"  bob  2026-02-24  e5f6a7b8
```

When annotations have spans, the line range is displayed. Use
`--line <n>` to filter to annotations overlapping a specific line.

`--all` shows all records including resolved/superseded ones (default hides
them). `--pretty` forces colored output when piped.

### 6.5 `qualifier score`

```
qualifier score

  SUBJECT               RAW    EFF   STATUS
  lib/crypto            -20    -20   ██░░░░░░░░  blocker
  lib/auth               60    -20   ██░░░░░░░░  blocker
  lib/http               80     80   ████████░░  healthy
  bin/server             45    -20   ██░░░░░░░░  blocker
```

### 6.6 `qualifier check`

Returns exit code 0 if all subjects meet the threshold, non-zero otherwise.

```
qualifier check --min-score 0
```

### 6.7 `qualifier ls`

```
qualifier ls --below 0
qualifier ls --kind blocker
qualifier ls --unqualified
```

### 6.8 `qualifier compact`

```
qualifier compact src/parser.rs              # prune superseded records
qualifier compact src/parser.rs --snapshot   # collapse to a single epoch
qualifier compact src/parser.rs --dry-run    # preview without writing
qualifier compact --all                      # compact every .qual file
qualifier compact --all --dry-run            # preview repo-wide compaction
```

### 6.9 `qualifier review`

Check the freshness of span-addressed annotations against current file content.

```
qualifier review                          # check all annotations
qualifier review src/parser.rs            # check annotations for one subject
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

Only active (non-superseded) annotations with spans that have a `content_hash`
are checked. Annotations without spans or without `content_hash` are skipped.

**JSON output** includes `status` (`fresh`, `drifted`, `missing`) and `detail`
with expected/actual hashes for drifted annotations or a reason for missing ones.

### 6.10 `qualifier init`

```
qualifier init
  Created qualifier.graph.jsonl
  Detected VCS: git
  Added *.qual merge=union to .gitattributes
```

### 6.11 Configuration

Qualifier uses layered configuration. Precedence (highest wins):

| Priority | Source |
|----------|--------|
| 1 (highest) | CLI flags |
| 2 | Environment variables |
| 3 | Project config (`.qualifier.toml`) |
| 4 | User config (`~/.config/qualifier/config.toml`) |
| 5 (lowest) | Built-in defaults |

**Configuration keys:**

| Key         | CLI flag       | Env var              | Default |
|-------------|----------------|----------------------|---------|
| `graph`     | `--graph`      | `QUALIFIER_GRAPH`    | `qualifier.graph.jsonl` |
| `issuer`    | `--issuer`     | `QUALIFIER_ISSUER`   | VCS identity (see 8.4) |
| `format`    | `--format`     | `QUALIFIER_FORMAT`   | `human` |
| `min_score` | `--min-score`  | `QUALIFIER_MIN_SCORE`| `0` |

### 6.12 `qualifier blame`

Delegates to the underlying VCS blame command for the subject's `.qual` file.

```
qualifier blame src/parser.rs
```

## 7. Library API

The `qualifier` crate exposes its library API from `src/lib.rs`. Library
consumers add `qualifier = { version = "0.3", default-features = false }` to
avoid pulling in CLI dependencies.

```rust
// qualifier::annotation — record types and core logic

/// A typed qualifier record. Dispatches on the `type` field in JSON.
pub enum Record {
    Annotation(Box<Annotation>),
    Epoch(Epoch),
    Dependency(DependencyRecord),
    Unknown(serde_json::Value),  // forward compatibility
}

impl Record {
    pub fn subject(&self) -> &str;
    pub fn id(&self) -> &str;
    pub fn score(&self) -> Option<i32>;         // Annotation | Epoch
    pub fn supersedes(&self) -> Option<&str>;   // Annotation only
    pub fn references(&self) -> Option<&str>;   // Annotation only
    pub fn kind(&self) -> Option<&Kind>;        // Annotation only
    pub fn issuer_type(&self) -> Option<&IssuerType>;
    pub fn as_annotation(&self) -> Option<&Annotation>;
    pub fn as_epoch(&self) -> Option<&Epoch>;
    pub fn is_scored(&self) -> bool;            // Annotation | Epoch
}

pub struct Annotation {
    pub metabox: String,                    // always "1"
    pub record_type: String,                // "annotation"
    pub subject: String,
    pub issuer: String,
    pub issuer_type: Option<IssuerType>,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub body: AnnotationBody,
}

pub struct AnnotationBody {
    pub detail: Option<String>,
    pub kind: Kind,
    pub r#ref: Option<String>,
    pub references: Option<String>,
    pub score: Option<i32>,
    pub span: Option<Span>,
    pub suggested_fix: Option<String>,
    pub summary: String,
    pub supersedes: Option<String>,
    pub tags: Vec<String>,
}

pub struct Epoch {
    pub metabox: String,                    // always "1"
    pub record_type: String,                // "epoch"
    pub subject: String,
    pub issuer: String,
    pub issuer_type: Option<IssuerType>,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub body: EpochBody,
}

pub struct EpochBody {
    pub refs: Vec<String>,
    pub score: i32,
    pub span: Option<Span>,
    pub summary: String,
}

pub struct DependencyRecord {
    pub metabox: String,                    // always "1"
    pub record_type: String,                // "dependency"
    pub subject: String,
    pub issuer: String,
    pub issuer_type: Option<IssuerType>,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub body: DependencyBody,
}

pub struct DependencyBody {
    pub depends_on: Vec<String>,
}

pub struct Span {
    pub start: Position,
    pub end: Option<Position>,          // normalized to Some(start) before hashing
    pub content_hash: Option<String>,   // BLAKE3 of spanned lines
}

pub struct Position {
    pub line: u32,               // 1-indexed
    pub col: Option<u32>,        // 1-indexed, optional
}

pub enum Kind { Pass, Fail, Blocker, Concern, Comment, Resolve, Praise, Suggestion, Waiver, Custom(String) }
pub enum IssuerType { Human, Ai, Tool, Unknown }

pub fn generate_id(annotation: &Annotation) -> String;
pub fn generate_epoch_id(epoch: &Epoch) -> String;
pub fn generate_dependency_id(dep: &DependencyRecord) -> String;
pub fn generate_record_id(record: &Record) -> String;
pub fn validate(annotation: &Annotation) -> Vec<String>;
pub fn finalize(annotation: Annotation) -> Annotation;
pub fn finalize_epoch(epoch: Epoch) -> Epoch;
pub fn finalize_record(record: Record) -> Record;

// qualifier::qual_file
pub struct QualFile { pub path: PathBuf, pub subject: String, pub records: Vec<Record> }
pub fn parse(path: &Path) -> Result<QualFile>;
pub fn append(path: &Path, record: &Record) -> Result<()>;
pub fn discover(root: &Path, respect_ignore: bool) -> Result<Vec<QualFile>>;

// qualifier::scoring
pub struct ScoreReport { pub raw: i32, pub effective: i32, pub limiting_path: Option<Vec<String>> }
pub fn raw_score(records: &[Record]) -> i32;
pub fn effective_scores(graph: &DependencyGraph, qual_files: &[QualFile]) -> HashMap<String, ScoreReport>;

// qualifier::content_hash — span freshness checking
pub fn compute_span_hash(file_path: &Path, span: &Span) -> Option<String>;
pub enum FreshnessStatus { Fresh, Drifted { expected, actual }, Missing { reason }, NoHash }
pub fn check_freshness(file_path: &Path, span: &Span) -> FreshnessStatus;

// qualifier::compact
pub struct CompactResult { pub before: usize, pub after: usize, pub pruned: usize }
pub fn prune(qual_file: &QualFile) -> (QualFile, CompactResult);
pub fn snapshot(qual_file: &QualFile) -> (QualFile, CompactResult);
```

The library is the source of truth. The CLI is a thin wrapper around it.

## 8. VCS Integration

`.qual` files SHOULD be committed to version control. Qualifier is VCS-agnostic
— the append-only JSONL format is friendly to any system that tracks text files.

### 8.1 General Principles

- Append-only JSONL minimizes merge conflicts.
- Pre-compaction history is recoverable from VCS history.
- `qualifier init` detects the active VCS and applies appropriate configuration.

### 8.2 VCS-Specific Setup

| VCS        | Action |
|------------|--------|
| Git        | Adds `*.qual merge=union` to `.gitattributes` |
| Mercurial  | Adds `**.qual = union` merge pattern to `.hgrc` |
| Other      | Prints guidance for manual merge configuration |

### 8.3 `qualifier blame`

Delegates to the underlying VCS blame/annotate command:

- Git: `git blame`
- Mercurial: `hg annotate`
- Fallback: not available (prints guidance)

### 8.4 Issuer Defaults

When `--issuer` is omitted:

- Git: `git config user.email`
- Mercurial: `hg config ui.username`
- Fallback: `mailto:$USER@localhost`

## 9. Agent Integration

Qualifier is designed to be used by AI coding agents. Key affordances:

- **Structured output:** `--format json` on `score`, `show`, and `ls` commands.
- **Batch annotation:** `qualifier attest --stdin` reads JSONL from stdin.
- **Suggested fixes:** The `suggested_fix` body field gives agents a concrete
  action to take.
- **Span precision:** The `span` body field lets agents target specific line
  ranges, making annotations actionable without hunting for the relevant code.
- **Priority ordering:** `qualifier ls --below 0 --format json` gives agents a
  prioritized worklist.
- **Continuous interaction:** `qualifier reply <id> <message>` lets agents
  respond to human signals with threaded follow-ups. `qualifier resolve <id>`
  lets agents close issues after fixes are applied.
- **Threading:** The `references` field enables agents to thread follow-up
  observations to prior signals, creating navigable conversation histories.

## 10. File Discovery

Qualifier discovers `.qual` files by walking the directory tree from the
project root. Each `.qual` file may contain records for multiple subjects
and multiple record types.

The project root is determined by searching upward for VCS markers (`.git`,
`.hg`, `.jj`, `.pijul`, `_FOSSIL_`, `.svn`) or a `qualifier.graph.jsonl`
file, whichever is found first.

### 10.1 Ignore Rules

By default, qualifier respects ignore rules from two sources during file
discovery:

1. **`.gitignore`** — Standard Git ignore files, including:
   - `.gitignore` files at any level of the tree
   - `.git/info/exclude` (per-repo excludes)
   - The global gitignore file (e.g., `~/.config/git/ignore`)
   - `.gitignore` files in parent directories above the project root
     (matching Git's own behavior in monorepos)

2. **`.qualignore`** — A qualifier-specific ignore file using the same
   syntax as `.gitignore`. Place a `.qualignore` file anywhere in the tree
   to exclude paths from qualifier's discovery walk. Useful for ignoring
   vendored code, generated files, or example directories that have `.qual`
   files you want qualifier to skip without affecting Git.

Paths matched by either source are excluded from all discovery commands:
`score`, `show`, `check`, `ls`, `compact`, and `praise`/`blame`.

### 10.2 `--no-ignore`

Pass `--no-ignore` to any discovery command to bypass all ignore rules.
This forces qualifier to walk every non-hidden directory and discover all
`.qual` files regardless of `.gitignore` or `.qualignore` entries.

### 10.3 Hidden Directories

Hidden directories (names starting with `.`) are always skipped during
discovery, regardless of ignore settings. This prevents qualifier from
descending into `.git`, `.vscode`, `.idea`, and similar tool directories.

Hidden *files* (like `.qual`) are not skipped — the per-directory `.qual`
layout depends on this.

## 11. Crate Structure

A single crate published as `qualifier` on crates.io.

```
qualifier/
├── Cargo.toml
├── SPEC.md                    # This document
├── METABOX.md                 # Metabox envelope specification
├── qualifier.graph.jsonl      # Example / self-hosted graph
└── src/
    ├── lib.rs                 # Public library API
    ├── annotation.rs         # Record types, body structs, Kind, IssuerType, validation
    ├── content_hash.rs        # Span content hashing and freshness checking
    ├── qual_file.rs           # .qual file parsing, appending, discovery
    ├── graph.rs               # Dependency graph loading, cycle detection
    ├── scoring.rs             # Raw + effective score computation
    ├── compact.rs             # Compaction: prune and snapshot
    ├── bin/
    │   └── qualifier.rs       # Binary entry point
    └── cli/                   # CLI module (behind "cli" feature)
        ├── mod.rs
        ├── config.rs
        ├── output.rs
        └── commands/
            ├── mod.rs
            ├── attest.rs
            ├── review.rs         # Shared review command logic
            ├── comment.rs        # qualifier comment
            ├── flag.rs           # qualifier flag
            ├── suggest.rs        # qualifier suggest
            ├── approve.rs        # qualifier approve
            ├── reject.rs         # qualifier reject
            ├── reply.rs          # qualifier reply
            ├── resolve.rs        # qualifier resolve
            ├── freshness.rs      # qualifier review (freshness checking)
            ├── show.rs
            ├── score.rs
            ├── ls.rs
            ├── check.rs
            ├── compact.rs
            ├── graph_cmd.rs
            ├── init.rs
            └── blame.rs
```

```toml
[features]
default = ["cli"]
cli = ["dep:clap", "dep:comfy-table", "dep:figment"]
```

## 12. Future Considerations (Out of Scope)

These are explicitly **not** part of v0.3 but are anticipated:

- **Policy records** (`type: "policy"`): Project-level scoring rules, required
  kinds, and gate criteria — expressed as records in the same stream.
- **Span-level scoring:** Opt-in scoring at sub-artifact granularity.
- **Editor plugins:** LSP-based inline display of scores and annotations,
  with span-aware gutter annotations.
- **DSSE signing:** `qualifier sign` to wrap records in DSSE envelopes for
  supply-chain distribution via Sigstore.
- **Decay:** Time-based score decay to encourage re-qualification.
- **`qualifier import-sarif`:** First-class SARIF import command.
- **`qualifier rename`:** Automated subject rename with `.qual` file and
  dependency migration.
- **`qualifier watch`:** File-watcher mode for continuous scoring.
- **Remote aggregation:** Qualifier servers for cross-repository views.

---

*The Koalafier has spoken. Now go qualify some code.*
