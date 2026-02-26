# Qualifier Specification

**Version:** 0.2.0-draft
**Status:** Draft
**Authors:** Alex Kesling

---

## Abstract

Qualifier is a deterministic system for recording, propagating, and querying
typed metadata records against software artifacts. It provides a VCS-friendly
file format (`.qual`), a Rust library (`libqualifier`), and a CLI binary
(`qualifier`) that together enable humans and agents to annotate code with
structured quality signals — and to compute aggregate quality scores that
propagate through dependency graphs.

Records are content-addressed, append-only, and human-writable. No server, no
database, no PKI required.

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
   attestations AND the effective quality of everything it depends on. A
   pristine binary that links a cursed library inherits the curse.

5. **Human-first, agent-friendly.** The CLI is designed for humans at a
   terminal. The JSONL format and library API are designed for agents and
   tooling. Both are first-class.

6. **Composable.** The record format separates a uniform frame (who said
   something about which artifact) from typed payloads (what they said). New
   record types extend the system without changing the frame. Unknown types
   pass through harmlessly.

7. **Interoperable.** Qualifier records project losslessly into in-toto
   attestation predicates. SARIF results import into qualifier attestations.
   The format bridges the gap between supply-chain attestation frameworks and
   human-scale quality tracking.

## 2. Record Model

### 2.1 Records

A **record** is a single, immutable, content-addressed JSON object that says
something about a software artifact. Records are the atoms of the system.

Every record has a **frame** — a fixed set of fields that identify *who* said
*what kind of thing* about *which artifact* and *when* — plus **body fields**
defined by the record's type.

Body fields are inlined at the top level of the JSON object alongside frame
fields. There is no nested `body` wrapper. The `type` field tells the parser
which fields are frame fields and which are body fields.

### 2.2 Record Frame

Every record has these frame fields:

| Field        | Type     | Required | Description |
|--------------|----------|----------|-------------|
| `v`          | integer  | yes      | Frame version. MUST be `3`. |
| `type`       | string   | yes*     | Record type identifier (see 2.5). *May be omitted in `.qual` files; defaults to `"attestation"`. |
| `artifact`   | string   | yes      | Qualified name of the target artifact |
| `span`       | object   | no       | Sub-artifact range (see 2.4) |
| `author`     | string   | yes      | Who or what created this record |
| `created_at` | string   | yes      | RFC 3339 timestamp |
| `id`         | string   | yes      | Content-addressed BLAKE3 hash (see 2.8) |

These seven fields form the **uniform interface**. They are the same for every
record type, they are stable across spec revisions, and they are sufficient
to answer the questions "who said what kind of thing about what, where, and
when?" without understanding the body.

### 2.3 Artifact Names

An **artifact** is any addressable unit of software that can be qualified.
Artifacts are identified by a **qualified name** (a string), which SHOULD
correspond to a logical unit in the codebase:

- A file path: `src/parser.rs`
- A module: `crate::parser`
- A build target: `//services/auth:lib`
- A package: `pkg:npm/lodash@4.17.21`

Qualifier does not enforce a naming scheme. The names are opaque strings.
Conventions are a project-level decision.

#### 2.3.1 Artifact Renames

Qualifier identifies artifacts by their qualified name. Renaming an artifact
(e.g., `src/parser.rs` to `src/ast_parser.rs`) requires the following steps:

1. Rename the `.qual` file to match the new artifact name.
2. Update dependency records to reference the new name wherever the old name
   appeared (both as `artifact` and in `depends_on` arrays).
3. **Note:** Existing records inside the renamed `.qual` file still contain the
   old `artifact` field in their JSON. Since record IDs are content-addressed,
   changing the `artifact` field would change the ID, breaking supersession
   chains.

The RECOMMENDED workflow after a rename is:

1. Rename the `.qual` file and update dependency records.
2. Run `qualifier compact <new-name> --snapshot` to collapse history into a
   fresh epoch under the new name.
3. Commit the rename and compacted file together.

### 2.4 Spans

A **span** identifies a sub-range within an artifact. When present, the record
addresses a specific region rather than the whole artifact.

```json
"span": {
  "start": { "line": 42 },
  "end": { "line": 58 }
}
```

A span is an object with two position fields:

| Field   | Type   | Required | Description |
|---------|--------|----------|-------------|
| `start` | object | yes      | Start of the range (inclusive) |
| `end`   | object | no       | End of the range (inclusive). Defaults to `start`. |

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

After normalization, `{"start":{"line":42}}` and
`{"start":{"line":42},"end":{"line":42}}` produce identical canonical forms
and therefore identical record IDs.

#### 2.4.3 Span Scoring

Span-addressed records contribute to the score of their parent **artifact**.
An attestation about `src/parser.rs` at span `{start: {line: 42}, end: {line: 58}}`
contributes to the raw score of `src/parser.rs`, not to a separate span-level
score.

Spans are addressing granularity, not scoring granularity. They tell you
*where* within the artifact a signal applies but do not create separate scoring
targets.

> **Rationale.** Span-level scoring would be extremely noisy for most
> workflows. Artifact-level aggregation is the right default. Future
> extensions MAY introduce opt-in span-level scoring.

### 2.5 Record Types

The `type` field is a string that identifies the body schema. Implementations
MUST support the following types:

| Type            | Description |
|-----------------|-------------|
| `attestation`   | A quality signal (see 3.1) |
| `epoch`         | A compaction snapshot (see 3.2) |
| `dependency`    | A dependency edge (see 3.4) |

Implementations MUST ignore records with unrecognized types (forward
compatibility). Unrecognized records MUST be preserved during file operations
(compaction, rewriting) — they are opaque pass-through data.

When `type` is omitted in a `.qual` file, it defaults to `"attestation"`.
In canonical form (for hashing), `type` is always materialized.

### 2.6 Attestation Records

An **attestation** is a quality signal about an artifact. It is the primary
record type and the reason qualifier exists.

Frame fields (section 2.2) plus these body fields:

| Field           | Type     | Required | Description |
|-----------------|----------|----------|-------------|
| `kind`          | string   | yes      | The type of attestation (see 2.7) |
| `score`         | integer  | yes      | Signed quality delta, -100..100 |
| `summary`       | string   | yes      | Human-readable one-liner |
| `detail`        | string   | no       | Extended description, markdown allowed |
| `suggested_fix` | string   | no       | Actionable suggestion for improvement |
| `tags`          | string[] | no       | Freeform classification tags |
| `author_type`   | string   | no       | Author classification: `human`, `ai`, `tool`, `unknown` |
| `ref`           | string   | no       | VCS reference pin (e.g., `"git:3aba500"`). Opaque to qualifier. |
| `supersedes`    | string   | no       | ID of a prior record this replaces (see 2.9) |

**Example:**

```json
{"v":3,"type":"attestation","artifact":"src/parser.rs","span":{"start":{"line":42},"end":{"line":58}},"kind":"concern","score":-10,"summary":"Panics on malformed input","suggested_fix":"Use the ? operator instead of unwrap()","tags":["robustness"],"author":"alice@example.com","author_type":"human","created_at":"2026-02-25T10:00:00Z","ref":"git:3aba500","id":"a1b2c3d4..."}
```

**Shorthand (equivalent):** Since `type` defaults to `"attestation"`, it may
be omitted:

```json
{"v":3,"artifact":"src/parser.rs","span":{"start":{"line":42},"end":{"line":58}},"kind":"concern","score":-10,"summary":"Panics on malformed input","suggested_fix":"Use the ? operator instead of unwrap()","tags":["robustness"],"author":"alice@example.com","author_type":"human","created_at":"2026-02-25T10:00:00Z","ref":"git:3aba500","id":"a1b2c3d4..."}
```

### 2.7 Attestation Kinds

The `kind` field is an open enum. The following kinds are defined by the spec;
implementations MUST support them and MAY define additional kinds.

| Kind          | Meaning |
|---------------|---------|
| `pass`        | The artifact meets a stated quality bar |
| `fail`        | The artifact does NOT meet a stated quality bar |
| `blocker`     | A blocking issue that must be resolved before release |
| `concern`     | A non-blocking issue worth tracking |
| `praise`      | Positive recognition of quality |
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
| `praise`      | +30           | +10 to +50        | positive |
| `suggestion`  | -5            | -5 to -15         | negative |
| `waiver`      | +10           | 0 to +30          | positive |

When `--score` is omitted from `qualifier attest`, the CLI SHOULD use the
default score for the given kind. `--score` always takes precedence.

These are guidance, not constraints. Implementations MUST NOT reject an
attestation solely because its score falls outside the recommended range.

#### 2.7.2 Custom Kinds

Any string is a valid `kind`. Implementations SHOULD detect likely typos
(edit distance <= 2 from a built-in kind) and warn the user.

### 2.8 Record IDs & Canonical Form

A record ID is a lowercase hex-encoded BLAKE3 hash of the **Qualifier
Canonical Form (QCF)** of the record, with the `id` field set to the empty
string `""` during hashing. This makes IDs deterministic and
content-addressed.

#### 2.8.1 Qualifier Canonical Form (QCF)

To ensure that every implementation — regardless of language or JSON library —
produces identical bytes for the same record, the canonical serialization MUST
obey the following rules:

1. **Normalization.** Before serialization:
   - `type` MUST be materialized. If absent, set to `"attestation"`.
   - `span.end` MUST be materialized. If absent, set equal to `span.start`.
   - `id` MUST be set to `""` (the empty string).

2. **Field order.** Fields MUST appear in the order defined for the record's
   type. The order for each type is specified below.

3. **Absent optional fields.** Optional fields whose value is absent (null,
   None, etc.) MUST be omitted entirely. `tags` MUST be omitted when the
   array is empty. The `id` field is the sole exception — it is always
   present (set to `""`).

4. **Whitespace.** No whitespace between tokens. No space after `:` or `,`.
   No trailing newline. The output is a single compact JSON line.

5. **No trailing commas.** Standard JSON — no trailing commas.

6. **String encoding.** Standard JSON escaping (RFC 8259 Section 7).
   Implementations MUST NOT add escapes beyond what JSON requires.

7. **Number encoding.** Integers serialize as bare decimal with no leading
   zeros, no decimal point, no exponent. Negative values use a leading `-`.

#### 2.8.2 Canonical Field Orders

**Attestation** (`type: "attestation"`):

```
v, type, artifact, span, kind, score, summary, detail, suggested_fix, tags,
author, author_type, created_at, ref, supersedes, id
```

**Epoch** (`type: "epoch"`):

```
v, type, artifact, span, score, summary, refs,
author, author_type, created_at, id
```

**Dependency** (`type: "dependency"`):

```
v, type, artifact, depends_on, author, created_at, id
```

New record types MUST define their canonical field order in their specification.

#### 2.8.3 Example

Given an attestation with `span: None`, `detail: None`, `suggested_fix: None`,
`tags: []`, `author_type: None`, `ref: None`, `supersedes: None`, the QCF is:

```json
{"v":3,"type":"attestation","artifact":"src/parser.rs","kind":"concern","score":-30,"summary":"Panics on malformed input","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":""}
```

With a span:

```json
{"v":3,"type":"attestation","artifact":"src/parser.rs","span":{"start":{"line":42},"end":{"line":42}},"kind":"concern","score":-30,"summary":"Panics on malformed input","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":""}
```

Note that `span.end` has been materialized (it was omitted in the input,
defaulting to `start`).

> **Rationale.** This scheme matches the behavior of serde_json with
> `#[serde(skip_serializing_if)]` annotations. It is simpler than RFC 8785
> (JSON Canonicalization Scheme) and sufficient for qualifier's needs.

### 2.9 Supersession

Records are immutable once written. To "update" a signal, you write a new
attestation with a `supersedes` field pointing to the prior record's `id`.

**Constraints:**

- The superseding and superseded records MUST have the same `artifact` field.
  Cross-artifact supersession is forbidden. Implementations MUST reject it.
- The `span` field MAY differ between superseder and superseded. (The
  problematic code may have moved.)
- Supersession chains MUST be acyclic. Implementations MUST detect and reject
  cycles.
- When computing scores, a superseded record MUST be excluded. Only the tip
  of each chain contributes.
- Dangling `supersedes` references (pointing to IDs not present in the current
  file set) are allowed. The referencing record remains active.

### 2.10 The `.qual` File Format

A `.qual` file is a UTF-8 encoded file where each line is a complete JSON
object representing one record. This is JSONL (JSON Lines).

**Placement:** A `.qual` file can contain records for any artifacts in its
directory or subdirectories. The `artifact` field in each record is the
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
{"v":3,"artifact":"src/parser.rs","kind":"concern","score":-30,"summary":"Panics on malformed UTF-8 input","span":{"start":{"line":42},"end":{"line":58}},"suggested_fix":"Replace .unwrap() with proper error propagation","tags":["robustness","error-handling"],"author":"alice@example.com","author_type":"human","created_at":"2026-02-24T10:00:00Z","ref":"git:3aba500","id":"a1b2c3d4..."}
{"v":3,"artifact":"src/parser.rs","kind":"praise","score":40,"summary":"Excellent property-based test coverage","tags":["testing"],"author":"bob@example.com","author_type":"human","created_at":"2026-02-24T11:00:00Z","id":"e5f6a7b8..."}
```

## 3. Record Type Specifications

### 3.1 Attestation (`type: "attestation"`)

Defined in section 2.6. This is the primary record type.

### 3.2 Epoch (`type: "epoch"`)

An **epoch** is a synthetic compaction summary produced by the compactor. It
replaces a set of attestations with a single record that preserves the net
score.

Body fields:

| Field         | Type     | Required | Description |
|---------------|----------|----------|-------------|
| `score`       | integer  | yes      | Raw score at compaction time |
| `summary`     | string   | yes      | `"Compacted from N attestations"` |
| `refs`        | string[] | yes      | IDs of the compacted records |
| `author_type` | string   | no       | Always `"tool"` for epochs |

Epoch records MUST set `author` to `"qualifier/compact"`.

**Example:**

```json
{"v":3,"type":"epoch","artifact":"src/parser.rs","score":10,"summary":"Compacted from 12 attestations","refs":["a1b2...","c3d4..."],"author":"qualifier/compact","author_type":"tool","created_at":"2026-02-25T12:00:00Z","id":"f9e8d7c6..."}
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
   records for each artifact are replaced by a single epoch record.

#### 3.3.1 Compaction Rules

- Compaction MUST NOT change the raw score of any artifact. This is the
  invariant. If compaction changes a score, the implementation has a bug.
- Compaction MUST be explicit and user-initiated — never automatic or silent.
- Compaction MUST preserve records of unrecognized types (they are opaque
  pass-through).
- After compaction, the file is a valid `.qual` file. No special reader
  support is needed.
- `qualifier compact --dry-run` MUST be supported.

### 3.4 Dependency (`type: "dependency"`)

A **dependency** record declares directed dependency edges from one artifact
to others.

Body fields:

| Field        | Type     | Required | Description |
|--------------|----------|----------|-------------|
| `depends_on` | string[] | yes      | Artifact names this artifact depends on |

**Example:**

```json
{"v":3,"type":"dependency","artifact":"bin/server","depends_on":["lib/auth","lib/http","lib/db"],"author":"build-system","created_at":"2026-02-25T10:00:00Z","id":"1a2b3c4d..."}
```

The dependency graph MUST be a DAG. Implementations MUST detect and reject
cycles.

#### 3.4.1 Dependency Graph Sources

Qualifier accepts dependency information from two sources:

1. **Dependency records in `.qual` files** — as defined above.
2. **Legacy graph file** — a standalone JSONL file (conventionally
   `qualifier.graph.jsonl`) with simplified dependency declarations:

```jsonl
{"artifact":"bin/server","depends_on":["lib/auth","lib/http","lib/db"]}
{"artifact":"lib/auth","depends_on":["lib/crypto"]}
```

Both sources are merged when computing effective scores. When both declare
edges for the same artifact, the union of all `depends_on` arrays is used.

### 3.5 Defining New Record Types

New record types are identified by a string value in the `type` field. Types
defined outside this spec SHOULD use a URI to avoid collisions:

```json
{"v":3,"type":"https://example.com/qualifier/license/v1","artifact":"src/parser.rs","license":"MIT","author":"license-scanner","created_at":"...","id":"..."}
```

Types defined in this spec use short aliases (`attestation`, `epoch`,
`dependency`). The spec reserves all unqualified type names (strings that
do not contain `:` or `/`) for future standardization.

A record type specification MUST define:

1. The body fields, their types, and which are required.
2. The canonical field order (frame fields + body fields + `id`).
3. How the type interacts with scoring (if at all).

## 4. Scoring

### 4.1 Raw Score

The **raw score** of an artifact is the sum of the `score` fields of all
non-superseded attestation and epoch records for that artifact, clamped to
`[-100, 100]`.

```
raw_score(A) = clamp(-100, 100, sum(record.score for active scored records of A))
```

An artifact with no scored records has a raw score of **0** (unqualified).

Only records of types that carry a `score` field (`attestation`, `epoch`)
contribute to scoring. Dependency records and unknown types do not.

### 4.2 Effective Score

The **effective score** of an artifact is a function of its raw score and the
effective scores of its dependencies:

```
effective_score(A) = min(raw_score(A), min(effective_score(D) for D in deps(A)))
```

Your effective score can never be higher than your worst dependency's effective
score. Quality flows downhill.

If an artifact has no dependencies, its effective score equals its raw score.

If an artifact has no scored records but has dependencies, its effective score
is the minimum effective score of its dependencies (the "inherited floor").

### 4.3 Span Scoring

Span-addressed records contribute to the raw score of their `artifact`. The
span is informational — it identifies where a signal applies within the
artifact, but scoring aggregates at the artifact level.

This means `qualifier score src/parser.rs` reports one score for the file,
even if individual attestations target different line ranges.

Implementations MAY offer span-level filtering for display (e.g.,
`qualifier show src/parser.rs --line 42` shows only attestations whose spans
overlap line 42), but this is a presentation concern, not a scoring concern.

### 4.4 Score Status

Implementations SHOULD report a human-readable status for each artifact:

| Condition | Status |
|-----------|--------|
| effective < 0 | `blocker` |
| effective = 0, limited by dependency | `unqualified (limited)` |
| effective = 0 | `unqualified` |
| effective >= 60, limited by dependency | `healthy (limited)` |
| effective >= 60 | `healthy` |
| limited by dependency | `ok (limited)` |
| otherwise | `ok` |

An artifact is "limited" when its effective score is lower than its raw score
due to a dependency constraint.

## 5. Interoperability

### 5.1 in-toto Predicate Projection

Qualifier records project losslessly into [in-toto v1 Statement](https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md)
predicates for use with DSSE signing and Sigstore distribution.

**Mapping (attestation):**

```json
{
  "_type": "https://in-toto.io/Statement/v1",
  "subject": [
    {
      "name": "src/parser.rs",
      "digest": {"blake3": "<artifact-content-hash>"}
    }
  ],
  "predicateType": "https://qualifier.dev/attestation/v1",
  "predicate": {
    "qualifier_id": "a1b2c3d4...",
    "span": {"start": {"line": 42}, "end": {"line": 58}},
    "kind": "concern",
    "score": -10,
    "summary": "Panics on malformed input",
    "tags": ["robustness"],
    "author": "alice@example.com",
    "author_type": "human",
    "created_at": "2026-02-25T10:00:00Z",
    "ref": "git:3aba500",
    "supersedes": null
  }
}
```

**Field mapping:**

| Qualifier field | in-toto location |
|----------------|------------------|
| `artifact` | `subject[0].name` |
| `span` | `predicate.span` |
| `id` | `predicate.qualifier_id` |
| `author` | `predicate.author` (also DSSE signer) |
| All body fields | `predicate.*` |

The in-toto `subject[0].digest` contains the content hash of the artifact
file. This is populated by the signing tool, not by qualifier itself.
Qualifier's `id` is the hash of the *record*, not the *artifact*.

**Predicate type URIs:**

| Qualifier type | Predicate type URI |
|---------------|-------------------|
| `attestation` | `https://qualifier.dev/attestation/v1` |
| `epoch` | `https://qualifier.dev/epoch/v1` |
| `dependency` | `https://qualifier.dev/dependency/v1` |

### 5.2 SARIF Import

SARIF v2.1.0 results can be converted to qualifier attestations:

| SARIF field | Qualifier field |
|-------------|----------------|
| `result.locations[0].physicalLocation.artifactLocation.uri` | `artifact` |
| `result.locations[0].physicalLocation.region.startLine` | `span.start.line` |
| `result.locations[0].physicalLocation.region.startColumn` | `span.start.col` |
| `result.locations[0].physicalLocation.region.endLine` | `span.end.line` |
| `result.locations[0].physicalLocation.region.endColumn` | `span.end.col` |
| `result.ruleId` | `kind` (as custom kind) |
| `result.level` | `score` (see mapping below) |
| `result.message.text` | `summary` |
| `run.tool.driver.name` | `author` |
| (constant) | `author_type: "tool"` |

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

```
qualifier attest <artifact> [options]     Add an attestation
qualifier show <artifact>                 Show attestations and scores
qualifier score [artifact...]             Compute and display scores
qualifier ls [--below <n>] [--kind <k>]   List artifacts by score/kind
qualifier graph [--format dot|json]        Visualize the dependency graph
qualifier check [--min-score <n>]          CI gate: exit non-zero if below threshold
qualifier compact <artifact> [options]     Compact a .qual file (prune/snapshot)
qualifier init                             Initialize qualifier in a repo
qualifier blame <artifact>                 Per-line VCS attribution for a .qual file
```

### 6.2 `qualifier attest`

Interactive and non-interactive attestation creation.

```
qualifier attest src/parser.rs \
  --kind concern \
  --score -30 \
  --summary "Panics on malformed input" \
  --suggested-fix "Use proper error propagation" \
  --tag robustness \
  --tag error-handling \
  --author "alice@example.com" \
  --span 42:58
```

#### 6.2.1 Span Syntax

The `--span` flag accepts the following forms:

| Form | Meaning | Equivalent `span` object |
|------|---------|--------------------------|
| `42` | Line 42 | `{"start":{"line":42},"end":{"line":42}}` |
| `42:58` | Lines 42 through 58 | `{"start":{"line":42},"end":{"line":58}}` |
| `42.5:58.80` | Line 42 col 5 through line 58 col 80 | `{"start":{"line":42,"col":5},"end":{"line":58,"col":80}}` |

When `--span` is omitted, no span is set (the attestation addresses the whole
artifact).

#### 6.2.2 Other Flags

`--summary` is required in non-interactive mode.

When `--score` is omitted, the CLI uses the recommended default score for the
given kind (see section 2.7.1).

`--file <path>` writes the attestation to a specific `.qual` file instead
of using the default layout resolution.

When `--author` is omitted, defaults to the VCS user identity (see 8.4).

### 6.3 `qualifier show`

```
qualifier show src/parser.rs

  src/parser.rs
  Raw score:       10
  Effective score: -20 (limited by lib/crypto)

  Attestations (2):
    [-30] concern  L42–58 "Panics on malformed input"    alice  2026-02-24
    [+40] praise          "Excellent property test coverage"  bob  2026-02-24
```

When attestations have spans, the line range is displayed. Use
`--line <n>` to filter to attestations overlapping a specific line.

### 6.4 `qualifier score`

```
qualifier score

  ARTIFACT              RAW    EFF   STATUS
  lib/crypto            -20    -20   ██░░░░░░░░  blocker
  lib/auth               60    -20   ██░░░░░░░░  blocker
  lib/http               80     80   ████████░░  healthy
  bin/server             45    -20   ██░░░░░░░░  blocker
```

### 6.5 `qualifier check`

Returns exit code 0 if all artifacts meet the threshold, non-zero otherwise.

```
qualifier check --min-score 0
```

### 6.6 `qualifier ls`

```
qualifier ls --below 0
qualifier ls --kind blocker
qualifier ls --unqualified
```

### 6.7 `qualifier compact`

```
qualifier compact src/parser.rs              # prune superseded records
qualifier compact src/parser.rs --snapshot   # collapse to a single epoch
qualifier compact src/parser.rs --dry-run    # preview without writing
qualifier compact --all                      # compact every .qual file
qualifier compact --all --dry-run            # preview repo-wide compaction
```

### 6.8 `qualifier init`

```
qualifier init
  Created qualifier.graph.jsonl
  Detected VCS: git
  Added *.qual merge=union to .gitattributes
```

### 6.9 Configuration

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
| `author`    | `--author`     | `QUALIFIER_AUTHOR`   | VCS identity (see 8.4) |
| `format`    | `--format`     | `QUALIFIER_FORMAT`   | `human` |
| `min_score` | `--min-score`  | `QUALIFIER_MIN_SCORE`| `0` |

### 6.10 `qualifier blame`

Delegates to the underlying VCS blame command for the artifact's `.qual` file.

```
qualifier blame src/parser.rs
```

## 7. Library API

The `qualifier` crate exposes its library API from `src/lib.rs`. Library
consumers add `qualifier = { version = "0.2", default-features = false }` to
avoid pulling in CLI dependencies.

```rust
// qualifier::attestation — record types and core logic

/// A typed qualifier record. Dispatches on the `type` field in JSON.
pub enum Record {
    Attestation(Attestation),
    Epoch(Epoch),
    Dependency(DependencyRecord),
    Unknown(serde_json::Value),  // forward compatibility
}

impl Record {
    pub fn artifact(&self) -> &str;
    pub fn id(&self) -> &str;
    pub fn score(&self) -> Option<i32>;         // Attestation | Epoch
    pub fn supersedes(&self) -> Option<&str>;   // Attestation only
    pub fn kind(&self) -> Option<&Kind>;        // Attestation only
    pub fn as_attestation(&self) -> Option<&Attestation>;
    pub fn as_epoch(&self) -> Option<&Epoch>;
    pub fn is_scored(&self) -> bool;            // Attestation | Epoch
}

pub struct Attestation {
    pub v: u8,                              // always 3
    pub record_type: String,                // "attestation"
    pub artifact: String,
    pub span: Option<Span>,
    pub kind: Kind,
    pub score: i32,
    pub summary: String,
    pub detail: Option<String>,
    pub suggested_fix: Option<String>,
    pub tags: Vec<String>,
    pub author: String,
    pub author_type: Option<AuthorType>,
    pub created_at: DateTime<Utc>,
    pub r#ref: Option<String>,
    pub supersedes: Option<String>,
    pub id: String,
}

pub struct Epoch {
    pub v: u8,                              // always 3
    pub record_type: String,                // "epoch"
    pub artifact: String,
    pub span: Option<Span>,
    pub score: i32,
    pub summary: String,
    pub refs: Vec<String>,
    pub author: String,
    pub author_type: Option<AuthorType>,
    pub created_at: DateTime<Utc>,
    pub id: String,
}

pub struct DependencyRecord {
    pub v: u8,                              // always 3
    pub record_type: String,                // "dependency"
    pub artifact: String,
    pub depends_on: Vec<String>,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub id: String,
}

pub struct Span {
    pub start: Position,
    pub end: Option<Position>,   // normalized to Some(start) before hashing
}

pub struct Position {
    pub line: u32,               // 1-indexed
    pub col: Option<u32>,        // 1-indexed, optional
}

pub enum Kind { Pass, Fail, Blocker, Concern, Praise, Suggestion, Waiver, Custom(String) }
pub enum AuthorType { Human, Ai, Tool, Unknown }

pub fn generate_id(attestation: &Attestation) -> String;
pub fn generate_epoch_id(epoch: &Epoch) -> String;
pub fn generate_dependency_id(dep: &DependencyRecord) -> String;
pub fn generate_record_id(record: &Record) -> String;
pub fn validate(attestation: &Attestation) -> Vec<String>;
pub fn finalize(attestation: Attestation) -> Attestation;
pub fn finalize_epoch(epoch: Epoch) -> Epoch;
pub fn finalize_record(record: Record) -> Record;

// qualifier::qual_file
pub struct QualFile { pub path: PathBuf, pub artifact: String, pub records: Vec<Record> }
pub fn parse(path: &Path) -> Result<QualFile>;
pub fn append(path: &Path, record: &Record) -> Result<()>;
pub fn discover(root: &Path) -> Result<Vec<QualFile>>;

// qualifier::scoring
pub struct ScoreReport { pub raw: i32, pub effective: i32, pub limiting_path: Option<Vec<String>> }
pub fn raw_score(records: &[Record]) -> i32;
pub fn effective_scores(graph: &DependencyGraph, qual_files: &[QualFile]) -> HashMap<String, ScoreReport>;

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

### 8.4 Author Defaults

When `--author` is omitted:

- Git: `git config user.email`
- Mercurial: `hg config ui.username`
- Fallback: `$USER@localhost`

## 9. Agent Integration

Qualifier is designed to be used by AI coding agents. Key affordances:

- **Structured output:** `--format json` on `score`, `show`, and `ls` commands.
- **Batch attestation:** `qualifier attest --stdin` reads JSONL from stdin.
- **Suggested fixes:** The `suggested_fix` field gives agents a concrete
  action to take.
- **Span precision:** The `span` field lets agents target specific line ranges,
  making attestations actionable without hunting for the relevant code.
- **Priority ordering:** `qualifier ls --below 0 --format json` gives agents a
  prioritized worklist.

## 10. File Discovery

Qualifier discovers `.qual` files by walking the directory tree from the
project root. Each `.qual` file may contain records for multiple artifacts
and multiple record types.

The project root is determined by searching upward for VCS markers (`.git`,
`.hg`, `.jj`, `.pijul`, `_FOSSIL_`, `.svn`) or a `qualifier.graph.jsonl`
file, whichever is found first.

## 11. Crate Structure

A single crate published as `qualifier` on crates.io.

```
qualifier/
├── Cargo.toml
├── SPEC.md                    # This document
├── qualifier.graph.jsonl      # Example / self-hosted graph
└── src/
    ├── lib.rs                 # Public library API
    ├── record.rs              # Record frame, Span, RecordBody enum
    ├── attestation.rs         # AttestationBody, Kind, AuthorType, validation
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

These are explicitly **not** part of v0.2 but are anticipated:

- **Policy records** (`type: "policy"`): Project-level scoring rules, required
  kinds, and gate criteria — expressed as records in the same stream.
- **Span-level scoring:** Opt-in scoring at sub-artifact granularity.
- **Editor plugins:** LSP-based inline display of scores and attestations,
  with span-aware gutter annotations.
- **DSSE signing:** `qualifier sign` to wrap records in DSSE envelopes for
  supply-chain distribution via Sigstore.
- **Decay:** Time-based score decay to encourage re-qualification.
- **`qualifier import-sarif`:** First-class SARIF import command.
- **`qualifier rename`:** Automated artifact rename with `.qual` file and
  dependency migration.
- **`qualifier watch`:** File-watcher mode for continuous scoring.
- **Remote aggregation:** Qualifier servers for cross-repository views.

---

*The Koalafier has spoken. Now go qualify some code.*
