# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) (with
the pre-1.0 caveat that any breaking change bumps the minor version).

## [0.5.0] — unreleased

### Added

- **`qualifier agents`** — self-contained guide for AI coding agents.
  Bare `qualifier agents` prints an orientation page with an index of
  topics; `qualifier agents <topic>` (e.g. `concepts`, `workflows`,
  `record`) drills into per-topic detail. The agent group also appears
  at the top of `qualifier --help` so models reading the help text
  discover the entry point on their own.

## [0.4.0] — unreleased

This release is a substantial reshape: the CLI surface narrowed, scoring
left the format, and project bootstrap was retired. Earlier history lives
in git; this is the first changelog entry.

### Removed

- **`qualifier score` and `qualifier check`** — the scoring engine and
  CI gating commands. The format no longer carries a `score` body field
  on annotations or epochs. Scoring is reframed in SPEC §4 as one
  optional layer a tool may add via custom body fields, with a polarity
  table in §2.7.1 as the only stable hook.
- **`qualifier init` and `qualifier graph`** — project bootstrap and
  dependency-graph visualization. The graph engine (`src/graph.rs`) is
  gone. `dependency` records and the `Record::Dependency` variant
  remain in the wire format and round-trip unchanged.
- Per-kind verbs **`qualifier {comment, flag, suggest, approve, reject,
  attest}`** — collapsed into the unified `qualifier record <kind>`.

### Added

- **`qualifier record <kind> <location> [message]`** — the unified
  annotation-write verb. Accepts the built-in kinds plus any custom
  string. `<location>` carries an optional span (`src/foo.rs:42` or
  `src/foo.rs:42:58`).
- **`qualifier emit <type> <subject> --body '<JSON>'`** — low-level
  passthrough for any record type. Bodies for non-`annotation` types
  round-trip via `Record::Unknown`. Supports `--stdin` batch input.
- **`qualifier reply <target> <message>`** and **`qualifier resolve
  <target> [message]`** — both accept an id-prefix or a `<location>`,
  with disambiguation when multiple active records share a location.
- **`qualifier review`** — span-bound annotation freshness check
  (`fresh` / `drifted` / `missing`) using BLAKE3 content hashes
  computed at write time.
- **`qualifier praise`** (alias: `blame`) — record-based attribution
  for an artifact; `--vcs` delegates to git/hg.
- **Threaded `qualifier show` output** with tree-drawing characters
  for replies and resolves under their parents.
- **`--type <TYPE>` filter on `qualifier show`** for filtering by
  envelope record type, including custom URI types.
- **Open record types in the wire format**: `license`,
  `security-advisory`, `perf-measurement`, plus arbitrary URI-typed
  records preserved through `Record::Unknown`.
- **`.qualignore` file discovery** alongside `.gitignore`, and a
  `--no-ignore` flag to bypass both.
- **Grouped `qualifier --help` output** rendered via a custom clap
  `help_template` (Record observations / Inspect annotations /
  Maintain / Other).

### Changed

- **Metabox envelope is now the wire format.** Records carry a fixed
  envelope (`metabox`, `type`, `subject`, `issuer`, `issuer_type`,
  `created_at`, `id`) wrapping a type-specific `body`. Body fields
  serialize in alphabetical order (Metabox Canonical Form). The
  envelope is specced separately in `METABOX.md` so other tools can
  adopt the same shape.
- **`issuer` is a URI** (`mailto:`, `https:`, `urn:…`). `issuer_type`
  is an optional envelope field with values `human`, `ai`, `tool`,
  `unknown`.
- **"Annotation" terminology** replaces "attestation" throughout code,
  spec, and docs.
- **Project root detection** searches upward for VCS markers only
  (`.git`, `.hg`, `.jj`, `.pijul`, `_FOSSIL_`, `.svn`). The
  `qualifier.graph.jsonl` marker was removed when the graph engine
  was yanked.

### Internal

- Restructured the crate around `annotation`, `compact`, `content_hash`,
  `qual_file` as the library core; the `cli` feature gates clap,
  comfy-table, figment, and rand.
- `compact::filter_superseded` is now the canonical "active records"
  helper, used by `show`, `ls`, `praise`, `reply`, and `freshness`.

[0.4.0]: https://github.com/empathic/qualifier
