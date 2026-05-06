# qualifier — key concepts

## The annotation envelope

Every qualifier record is a single-line JSON object using the **Metabox envelope**.
The envelope fields appear in a fixed order and are identical for every record type:

(formatted for readability; each record is a single line in the .qual file)

```json
{
  "metabox":    "1",
  "type":       "annotation",
  "subject":    "src/auth.rs",
  "issuer":     "mailto:agent@example.com",
  "issuer_type": "ai",
  "created_at": "2026-03-01T10:00:00Z",
  "id":         "<blake3-hex>",
  "body":       { ... }
}
```

Field notes:

- `metabox` — always `"1"`. Validation rejects any other value.
- `type` — `"annotation"` (default, may be omitted in files), `"epoch"`, `"dependency"`, or any non-empty string for custom types (URI recommended).
- `subject` — the artifact being annotated. A path (`src/auth.rs`), module, build target, or package name. Opaque to qualifier.
- `issuer` — who or what wrote the record. Must be a URI (see §Issuer URIs below).
- `issuer_type` — optional; one of `human`, `ai`, `tool`, `unknown`.
- `created_at` — RFC 3339 timestamp.
- `id` — BLAKE3 hash of the canonical form (see §Content addressing). Never hand-edit.
- `body` — type-specific payload; body fields are serialized in alphabetical order.

You do not construct this JSON by hand. Use `qualifier record`, `qualifier reply`,
`qualifier resolve`, or `qualifier emit` — they compute the correct `id` for you.

## Kinds

The `kind` field in an annotation body identifies the nature of the signal.
Choose the most specific kind that fits; downstream filtering and polarity depend on it.

| Kind         | When to use |
|--------------|-------------|
| `concern`    | A non-blocking issue worth tracking (e.g., a code smell, a risk, a TODO that matters). |
| `blocker`    | A blocking issue that must be resolved before release. |
| `fail`       | The artifact does not meet a stated quality bar. |
| `pass`       | The artifact meets a stated quality bar. |
| `comment`    | An observation or discussion point with no polarity impact. |
| `praise`     | Positive recognition — marks intentional design for future readers. |
| `suggestion` | A proposed improvement, typically paired with `--suggested-fix`. |
| `waiver`     | An acknowledged issue explicitly accepted with rationale. |
| `resolve`    | Closes a prior record via supersession. Prefer `qualifier resolve` instead of recording this directly. |

Any other string is valid as a custom kind. The CLI warns if it looks like a
typo of a built-in kind (edit distance ≤ 2).

Polarity summary: `pass`, `praise`, `waiver` are positive; `comment` and `resolve`
are neutral; `concern`, `suggestion`, `fail`, `blocker` are negative.

## Supersession

Records are **immutable once written**. To update or retract a signal, write a
new annotation with `supersedes` pointing to the old record's `id`.

Rules the system enforces:

- The superseding and superseded records **must share the same `subject`**.
  Cross-subject supersession is rejected.
- Supersession chains must be **acyclic**. A → B → A is detected and rejected.
- Only the **tip** of a supersession chain is active. Superseded records are
  hidden by `qualifier show` and `qualifier ls`.
- Dangling `supersedes` references (pointing to IDs not present in the
  current file set) are allowed — the referencing record stays active.

The canonical way to close an issue is `qualifier resolve <id>`, which writes
a `resolve`-kind annotation that supersedes the target. The original record
is no longer surfaced; the resolve record stands as a visible tombstone.

## `.qual` file layout and discovery

A `.qual` file is UTF-8 JSONL (one record per line). Three layouts coexist:

| Layout | Example file | Trade-off |
|--------|-------------|-----------|
| Per-directory (recommended) | `src/.qual` | Clean tree, good merge behavior |
| Per-file | `src/parser.rs.qual` | Maximum merge isolation, noisy tree |
| Per-project | `.qual` at repo root | Simplest, but high merge contention |

When you run `qualifier record concern src/auth.rs ...`, the CLI writes to
`src/.qual` by default (or `src/auth.rs.qual` if that file already exists).
Override with `--file <path>`.

**Discovery** walks from the project root (found by searching upward for
`.git`, `.hg`, `.jj`, `.pijul`, `_FOSSIL_`, or `.svn`). It collects every
file named `.qual` or ending in `.qual`.

**Ignore rules** are applied by default:
- `.gitignore` at any level (including global gitignore and `.git/info/exclude`).
- `.qualignore` — same syntax as `.gitignore`, qualifier-specific. Place
  anywhere in the tree to exclude vendored code, generated files, or examples.

Pass `--no-ignore` to bypass all ignore rules on any discovery command
(`show`, `ls`, `compact`, `review`, `praise`).

Hidden directories (e.g., `.git`, `.vscode`) are always skipped. Hidden
*files* like `.qual` are not skipped.

## Issuer URIs and `issuer_type`

The `issuer` field identifies who or what created a record. It must be a URI
— specifically, it must contain `:`. Validation rejects bare strings without a colon.

Common forms:

```
mailto:agent@example.com      # typical for an AI agent or human
https://ci.example.com        # CI job or tool with a URL
urn:qualifier:compact         # reserved for the compact command
```

When `--issuer` is omitted, the CLI detects the VCS user identity:
- Git: `git config user.email` → wrapped as `mailto:<email>`
- Mercurial: `hg config ui.username` → wrapped as `mailto:<username>`
- Fallback: `mailto:$USER@localhost`

As an agent, always pass `--issuer "mailto:your-agent-id@example.com"` and
`--issuer-type ai` so records are traceable back to you.

The `issuer_type` field is optional but strongly recommended:

| Value     | Use when |
|-----------|----------|
| `human`   | A person wrote this record |
| `ai`      | An AI agent wrote this record |
| `tool`    | An automated scanner or CI job wrote this record |
| `unknown` | Origin is unclear |

## Content addressing (BLAKE3)

A record's `id` is the BLAKE3 hash of its **Metabox Canonical Form (MCF)**:
the record serialized with envelope fields in fixed order, body fields in
alphabetical order, all optional absent fields omitted, and `id` set to `""`
during hashing.

Implications:

- Identical inputs always produce the same `id`, on every machine and implementation.
- Changing any canonical field (summary, kind, span, issuer, timestamp, ...) changes the `id`.
- **Do not hand-edit `.qual` files.** Editing a field invalidates the `id`,
  and `qualifier` will reject the record. Use `qualifier record`, `qualifier emit`,
  or the compactor to write records.
- Span normalization is part of MCF: a span with no `end` has `end` set equal
  to `start` before hashing, so `{"start":{"line":42}}` and
  `{"start":{"line":42},"end":{"line":42}}` produce the same `id`.

The `id` is a stable handle you can use to target a record with
`qualifier reply <id-prefix>` or `qualifier resolve <id-prefix>`.
Four characters of prefix are the minimum; use more if the file has many records.
