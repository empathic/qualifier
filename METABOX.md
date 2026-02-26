# Metabox

**Version:** 1.0.0-draft
**Status:** Draft
**Authors:** Alex Kesling

---

## Abstract

Metabox is a minimal envelope format for content-addressed records. It defines
seven fixed fields that answer "who said what, when" plus a `body` object for
domain-specific payload. Records are JSONL, IDs are BLAKE3 hashes of a
canonical form.

The format is designed for append-only, VCS-friendly record streams — quality
attestations, dependency declarations, audit logs, or any structured signal
that benefits from content addressing and a uniform envelope.

## 1. Envelope Fields

Every Metabox record is a JSON object with exactly seven top-level fields, in
this canonical order:

| #   | Field        | Type   | Required | Description                                    |
| --- | ------------ | ------ | -------- | ---------------------------------------------- |
| 1   | `metabox`    | string | yes      | Envelope version. Always `"1"`.                |
| 2   | `type`       | string | yes      | Body schema identifier.                        |
| 3   | `subject`    | string | yes      | What this record is about.                     |
| 4   | `author`     | string | yes      | Who or what created this record.               |
| 5   | `created_at` | string | yes      | RFC 3339 timestamp.                            |
| 6   | `id`         | string | yes      | Content-addressed BLAKE3 hash (see section 3). |
| 7   | `body`       | object | yes      | Type-specific payload.                         |

All seven fields are required. All seven are present in every record.

### 1.1 `metabox`

Always the string `"1"`. This field is self-identifying — any JSON object with
a `metabox` field is a Metabox record. Parsers SHOULD use this field to detect
Metabox records in mixed streams.

The value is a string, not an integer, to allow future non-numeric versioning
schemes and to avoid ambiguity with other version fields (like Qualifier's
`"v": 3`).

### 1.2 `type`

A string that identifies the schema of the `body` object. Metabox itself does
not define any types — they are the domain of consuming projects.

- **Short strings** for project-local types: `"attestation"`, `"epoch"`,
  `"dependency"`, `"ping"`.
- **URIs** for cross-project interoperability:
  `"https://qualifier.dev/attestation"`, `"https://example.com/audit/v1"`.

The `type` field is opaque to Metabox. It carries no semantics at the envelope
level beyond identifying which body fields to expect.

### 1.3 `subject`

What this record is about — a file path, URL, package name, service endpoint,
or any other addressable thing. The string is opaque to Metabox. Naming
conventions are a project-level decision.

Examples: `"src/parser.rs"`, `"pkg:npm/lodash@4.17.21"`, `"service/health"`,
`"https://example.com/api/v2"`.

### 1.4 `author`

Who or what created this record. Typically an email address, tool identifier,
or service account name. The string is opaque to Metabox.

### 1.5 `created_at`

An RFC 3339 timestamp indicating when the record was created.

### 1.6 `id`

A lowercase hex-encoded BLAKE3 hash of the record's Metabox Canonical Form
(section 3), 64 characters. Content-addressed: the same record always produces
the same ID.

### 1.7 `body`

A JSON object containing the type-specific payload. The body is always present.
Types with no fields use an empty object `{}`.

The envelope never looks inside the body. Generic Metabox tooling (indexers,
replicators, filters) can operate on the six envelope fields without
understanding or parsing the body.

## 2. File Format

Metabox records are stored as **JSONL** — one JSON object per line, UTF-8
encoded.

**Rules:**

- Each line MUST be a valid JSON object conforming to the Metabox envelope.
- Lines MUST be separated by a single `\n` (LF).
- The file MUST end with a trailing `\n`.
- Empty lines and lines starting with `//` are comments (ignored by parsers).
- Files are append-only by convention. New records are appended, never
  inserted. The sole exception is **compaction**, which rewrites the file.
- Implementations MUST preserve record ordering; older records come first.

**File extensions** are a project-level decision. Metabox does not mandate an
extension. (Qualifier uses `.qual`; other projects may use `.jsonl`,
`.metabox`, or whatever fits their ecosystem.)

## 3. Metabox Canonical Form (MCF)

To ensure that every implementation — regardless of language or JSON library —
produces identical bytes for the same record, the canonical serialization MUST
obey the following rules:

### 3.1 Normalization

Before serialization:

- `id` MUST be set to `""` (the empty string).
- All seven envelope fields MUST be present.
- `body` MUST be present (empty `{}` if the type has no fields).

### 3.2 Field Order

1. **Envelope fields** appear in the fixed order defined in section 1:
   `metabox`, `type`, `subject`, `author`, `created_at`, `id`, `body`.
2. **Body fields** are sorted lexicographically by key. Sorting is recursive:
   nested objects also have their keys sorted lexicographically.

### 3.3 Absent Optional Body Fields

Optional body fields whose value is absent (null, None, etc.) MUST be omitted
entirely from the body. Array-valued fields MUST be omitted when the array is
empty. The hash changes only when a field is actually present.

### 3.4 Whitespace

No whitespace between tokens. No space after `:` or `,`. No trailing newline.
The output is a single compact JSON line.

### 3.5 String Encoding

Standard JSON escaping (RFC 8259 Section 7). Implementations MUST NOT add
escapes beyond what JSON requires.

### 3.6 Number Encoding

Integers serialize as bare decimal with no leading zeros, no decimal point, no
exponent. Negative values use a leading `-`.

## 4. Content Addressing

The record ID is computed as:

1. Serialize the record in Metabox Canonical Form (section 3), with `id` set
   to `""`.
2. Compute the BLAKE3 hash of the MCF UTF-8 bytes.
3. Hex-encode the hash, lowercase. The result is 64 characters.

This makes IDs deterministic and content-addressed. The same logical record
always produces the same ID, regardless of which implementation generates it.

## 5. Type System

Types are opaque strings. Metabox does not define any types, validate body
schemas, or constrain what goes in `body`. That is the domain of consuming
projects.

**Conventions:**

- Short strings for project-local types: `"attestation"`, `"epoch"`,
  `"ping"`, `"audit"`.
- URIs for cross-project interoperability:
  `"https://qualifier.dev/attestation"`,
  `"https://example.com/audit/v1"`.

**Forward compatibility:** Implementations MUST preserve records with
unrecognized types. Unknown records are opaque pass-through data — they MUST
NOT be dropped, rewritten, or rejected.

A type specification (defined by the consuming project, not by Metabox) SHOULD
define:

1. The body fields, their types, and which are required.
2. How the type interacts with any domain-specific logic (scoring, etc.).

## 6. Examples

A Qualifier attestation in Metabox format:

```json
{"metabox":"1","type":"attestation","subject":"src/parser.rs","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":"a1b2c3d4e5f6a7b8a1b2c3d4e5f6a7b8a1b2c3d4e5f6a7b8a1b2c3d4e5f6a7b8","body":{"author_type":"human","kind":"concern","ref":"git:3aba500","score":-30,"summary":"Panics on malformed input"}}
```

Note that body fields are sorted lexicographically: `author_type`, `kind`,
`ref`, `score`, `summary`.

A minimal record with an empty body:

```json
{"metabox":"1","type":"ping","subject":"service/health","author":"monitor","created_at":"2026-02-25T12:00:00Z","id":"f9e8d7c6b5a4f9e8d7c6b5a4f9e8d7c6b5a4f9e8d7c6b5a4f9e8d7c6b5a4f9e8","body":{}}
```

A dependency declaration:

```json
{"metabox":"1","type":"dependency","subject":"bin/server","author":"build-system","created_at":"2026-02-25T10:00:00Z","id":"1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b","body":{"depends_on":["lib/auth","lib/http","lib/db"]}}
```

## 7. Qualifier Mapping

Qualifier v3 records map to Metabox as follows:

### 7.1 Frame → Envelope

| Qualifier v3       | Metabox           | Notes                            |
| ------------------ | ----------------- | -------------------------------- |
| `v: 3`             | `metabox: "1"`    | Version field changes name/value |
| `type`             | `type`            | Unchanged                        |
| `artifact`         | `subject`         | Renamed for generality           |
| `author`           | `author`          | Unchanged                        |
| `created_at`       | `created_at`      | Unchanged                        |
| `id`               | `id`              | Unchanged                        |

### 7.2 Body Fields → `body`

All non-frame fields move into the `body` object:

**Attestation** (`type: "attestation"`):

`span`, `kind`, `score`, `summary`, `detail`, `suggested_fix`, `tags`,
`author_type`, `ref`, `supersedes` → `body.*`

**Epoch** (`type: "epoch"`):

`span`, `score`, `summary`, `refs`, `author_type` → `body.*`

**Dependency** (`type: "dependency"`):

`depends_on` → `body.*`

### 7.3 Canonical Form Differences

| Qualifier (QCF)                       | Metabox (MCF)                        |
| ------------------------------------- | ------------------------------------ |
| Per-type field order for body fields  | Lexicographic body field order       |
| Body fields inlined at top level      | Body fields nested in `body` object  |
| `v: 3` in position 1                 | `metabox: "1"` in position 1        |
| `artifact` in position 3             | `subject` in position 3             |

## 8. Design Rationale

**Why `body`, not inlined?** Separation of concerns. The envelope answers "who
said what, when" — generic questions that every tool can answer. The body
answers domain-specific questions that only type-aware tools understand.
Inlining conflates the two, which means generic tooling has to know which
fields are "envelope" and which are "body" for each type. A nested `body`
makes the boundary explicit and mechanical.

**Why `metabox`, not `v`?** Self-identification. Any JSON object with a
`metabox` field is a Metabox record. This doesn't collide with existing version
fields (Qualifier's `v`, npm's `version`, etc.) and lets parsers detect Metabox
records in mixed or unknown streams without prior knowledge of the schema.

**Why `subject`, not `artifact`?** Generality. Not all records describe code
artifacts. A health check pings a service endpoint. An audit record references
a user action. `subject` is the most neutral term for "the thing this record
is about."

**Why lexicographic body ordering?** Simplicity. Per-type canonical field
orders (as in Qualifier's QCF) require every implementation to know the field
order for every type. Lexicographic ordering is universal — a generic
implementation can canonicalize any body without type-specific knowledge. This
is the key enabler for generic Metabox tooling.

**Why BLAKE3?** Fast, secure, widely available, and already proven in
Qualifier. The 256-bit output (64 hex chars) provides strong collision
resistance while keeping IDs compact enough to display and copy.

**Why JSONL?** Append-only JSONL is the simplest format that is
human-readable, VCS-friendly (clean diffs, useful blame), and trivial to parse
in any language. It is the right format for a record stream that grows over
time.

**Why comments?** Lines starting with `//` are ignored by parsers. This lets
humans annotate record files with context, section headers, or notes without
affecting the data. Comments are stripped during compaction.

---

*Metabox: put your records in a box.*
