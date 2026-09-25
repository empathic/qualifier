# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) (with
the pre-1.0 caveat that any breaking change bumps the minor version).

## [0.8.0] — unreleased

### Added

- `--supersedes` and `--references` accept ID prefixes (≥ 4 characters),
  resolved like `reply`/`resolve` targets. Editing a reply is
  `qualifier reply <target> "…" --supersedes <prefix>`.
- Write commands read `QUALIFIER_ISSUER`, `QUALIFIER_ISSUER_TYPE`, and
  `QUALIFIER_SESSION`, and detect Claude Code (`CLAUDECODE=1`): records
  written there default to `issuer_type: ai` and carry the tag
  `session:claude-code:<session id>`. Explicit flags still win.
- `resolve --reason <fixed|wontfix|duplicate|invalid|obsolete>` adds the
  tag `reason:<value>`.
- **`qualifier threads`** lists conversations across the project (root,
  live replies, open/closed) with location, glob, span, record-ID, kind,
  tag, and issuer-type filters, and JSON output. Path and span filters
  also match threads on the path's ancestor directories, and a span filter
  matches span-less threads on the same file; an ID prefix selects the
  thread containing that record in any role; under `--all`, `--tag` also
  matches the closing resolve.
- `threads --status`, `--changed-since <ref>`, and `--summary` (a
  two-line digest for session-start hooks).
- `record --stdin` accepts `{"reply": "<target>", …}` and
  `{"resolve": "<target>", …}` lines with the same target resolution as
  the single commands, including records created earlier in the batch.
- `qualifier agents conventions` and `qualifier agents batch`.

### Fixed

- `record --stdin` detects the VCS issuer once per run instead of once per
  line.

### Changed

- `show` marks records whose issuer type is not `human`, e.g. `alex (ai)`.
- **`reply` and `resolve` refuse superseded targets.** An ID prefix that
  matches a superseded record now fails, naming the live record at the tip
  of its chain, or reporting the record as closed when the chain ends in a
  `resolve`. `--allow-superseded` restores the old behavior for deliberate
  annotation of history.
- `record --stdin` without `--continue-on-error` is all-or-nothing: every
  line is resolved and validated first, every failing line is reported,
  and nothing is written if any line fails. Previously lines before the
  first failure were written.
- `record --stdin` overrides lines (`supersedes`/`references`) now resolve
  ID prefixes and require a live, existing target, matching the non-batch
  `--supersedes`/`--references` flags. Previously these fields were stored
  verbatim with no resolution or liveness check.
- Commands now discover the whole project when run from a subdirectory,
  not just that subdirectory's `.qual` files.
- **Locations are relative to the current directory; subjects are stored
  relative to the project root.** `record` (single and `--stdin`
  `location` values), `reply`/`resolve` location targets (single and
  batch), `threads` filters, and the `show`, `praise`, and `compact`
  artifacts join the argument to the current directory's path below the
  project root and normalize it; an argument that leaves the project root
  is an error. Every write lands in a `.qual` file under the project root;
  `--file` still resolves relative to the current directory. From `src/`,
  `record concern foo.rs …` now stores subject `src/foo.rs` in
  `src/.qual` (previously `foo.rs` in `src/.qual`), and a repo-relative
  path given from a subdirectory no longer creates a nested `.qual` tree.
- A location target for `reply`/`resolve` never resolves to a `resolve`
  record.
- `record --stdin --dry-run` no longer creates directories.

## [0.7.0] — unreleased

### Added

- **`qualifier init`** — interactive, idempotent bootstrap command.
  Configures `*.qual merge=union` in `.gitattributes` for git repos
  (hints for hg/jj/pijul/fossil/svn), and appends a one-line directive
  pointing AI coding agents at `qualifier agents` to any discovered
  agent-instruction file (`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`,
  `CONVENTIONS.md`, `.cursorrules`, `.windsurfrules`, `.clinerules` (file
  or directory), `.github/copilot-instructions.md`,
  `.github/instructions/*.instructions.md`, `.junie/guidelines.md`, or
  `*.md`/`*.mdc` files directly under `.cursor/rules/`). Symlinked
  duplicates are patched once, and existing CRLF line endings are kept.
  If no agent file exists, offers to create `AGENTS.md`. Supports `--yes`
  for non-interactive use and `--dry-run` to preview changes. Already-
  configured steps are skipped, including `.gitattributes` rules that
  already union-merge `*.qual` under git's last-match-wins rules. End of
  input at a prompt aborts instead of accepting the default.

### Fixed

- SPEC §8.2: the Mercurial setup is `**.qual = :union` under
  `[merge-patterns]` in `.hg/hgrc` (was `**.qual = union`, which names a
  nonexistent merge tool). Added a Jujutsu row.

## [0.6.2] — unreleased

### Fixed

- **Discovery walks hidden directories other than VCS metadata.** Records
  under directories such as `.github/` were invisible to `show`, `ls`,
  `reply`/`resolve` by ID prefix, `compact`, and `praise`, because
  every directory starting with `.` was skipped. Only `.git`, `.hg`,
  `.jj`, `.pijul`, `_FOSSIL_`, and `.svn` are now always skipped; other
  hidden directories are subject to `.gitignore`/`.qualignore` like any
  other directory (SPEC §10.2–10.3).

## [0.5.1] — unreleased

### Added

- **AGENTS-CLI 0.1 protocol document** (`AGENTS-CLI.md`) — a draft
  cross-tool convention for CLI tools that self-describe to AI coding
  agents. qualifier is named as the reference implementation. The
  protocol defines five MUST rules (`agents` subcommand, bare
  orientation, topic dispatch, exit-2 unknown-topic, `--help`
  discoverability) and four SHOULD recommendations.

### Changed

- **Internal:** the `qualifier agents` page registry is now generated
  at build time from TOML frontmatter on each `pages/*.md` file
  instead of being a hand-coded `&[Page]` array in `mod.rs`. New
  per-page fields: `name`, `summary`, `sees_also`, `since`.
  User-visible CLI behavior is unchanged.
- **Topic display order:** the topic index in `qualifier agents` and the
  available-topics list in unknown-topic errors are now in lexicographic
  order (file-sorted), where they were previously hand-ordered. This is
  cosmetic; the same set of topics is exposed.

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
