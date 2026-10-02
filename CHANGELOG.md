# Changelog

All notable changes to this project are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) (with
the pre-1.0 caveat that any breaking change bumps the minor version).

## [Unreleased] — Claude Code plugin 0.2.0

Changes to the `qual` plugin (`plugins/claude-code/`). The plugin is
versioned separately from the crate. 0.2.0 still pins qualifier 0.8.0;
the plugin release that pins 0.9.0 follows the 0.9.0 tag.

### Added

- **Getting started message.** The first session after the plugin is
  installed or upgraded, in any directory (including one without `.qual`
  files), shows a short message once per plugin version: what the plugin
  does, prompts to try, and how the pinned binary is downloaded and
  checksum-verified. The plugin README gains a "Getting started" section.
- The wrapper falls back to a source install with cargo when the verified
  prebuilt binary does not run on the system (for example a glibc build on
  a musl system).
- CI fails a pull request that changes the plugin without bumping the
  version in `plugin.json` (`scripts/test-plugin.sh --check-version-bump`).
- `scripts/test-plugin.sh` requires the binary that runs the skill
  examples to report the pinned version (`ALLOW_UNPINNED_SKILLS=1` makes a
  mismatch a warning), and checks every eval case's `scaffold.sh` against
  the shared fixture.

### Changed

- Skills no longer pre-approve `Bash(qualifier:*)`, which matched a
  `qualifier` on `PATH` rather than the pinned binary.
- The wrapper prints its call line in the form the skills' `allowed-tools`
  approve, and the SessionStart hook says only what holds.
- Skills read threads as JSON (the complete view) and take a closed
  thread's answer from its closing resolve (`closed_by`).
- Release builds for Linux aarch64 are static musl binaries
  (`aarch64-unknown-linux-musl`), like x86_64, so they also run on musl
  systems such as Alpine. The wrapper's target mapping and checksums move
  to the new target with the first release built this way.

## Claude Code plugin 0.1.0

### Added

- **Claude Code plugin `qual`** (`plugins/claude-code/`, plugin version
  0.1.0): lifecycle skills and a SessionStart hook; install with
  `/plugin marketplace add empathic/qualifier`. The plugin runs the
  qualifier release it pins (0.8.0), which it downloads, verifies against
  embedded checksums, and keeps under
  `~/.local/share/qualifier/plugin/<version>/`; a plugin update that pins
  a newer release installs it on next use. A `qualifier` on `PATH` is
  neither used nor modified. `QUALIFIER_BIN` selects a development build;
  `QUALIFIER_PLUGIN_HOME` relocates the installs.

## [0.9.0] — unreleased

Breaking changes are marked **Breaking**.

### Added

- **`qualifier diff` reports a Changed bucket**: threads open on both sides
  whose root was edited or re-anchored are listed once under "Changed on
  this branch" (JSON `changed: [{record, previous}]`), matched by thread
  origin, instead of as an Added record plus a Resolved one.
- `qualifier diff` names the comparison it used in its header and in a
  JSON `comparison` field: `merge-base`, `tip` (`--from-tip`), or
  `fallback-tip` (no merge base, so the ref's tip was used).
- `qualifier diff` lists every closer of a resolved record (JSON
  `closers`, oldest first, alongside `closer`).
- `threads --kind`, `diff --kind`, and `diff --fail-on` warn on stderr when
  a kind is neither built in nor carried by any record
  (`warning: kind 'blockers' matches no known kind`).
- **Thread state.** `threads`, `show`, and `praise` share one thread-state
  model: open, needs decision, decided, or closed (with the closing
  resolve's reason, and whether a decision was still pending). Threads
  JSON gains a `state` object; `show`/`praise` JSON gains a `threads` list
  of `{origin, root, state, closed_by}`.
- **Config file defaults take effect.** `issuer` and `format` from
  `.qualifier.toml`, `~/.config/qualifier/config.toml`, `QUALIFIER_ISSUER`,
  and `QUALIFIER_FORMAT` now set the default issuer of write commands and
  the default `--format` of every command that has the flag. Flags still
  win; environment variables are read as strings.
- **Custom body fields are preserved.** Annotation, epoch, and dependency
  bodies keep fields the type does not define when a file is rewritten
  (`compact`), and those fields are part of the record ID.
- Library: `annotation::Timestamp`, `ExtraFields` and the `extra` field on
  every body struct, `Kind::BUILT_IN`, `generate_unknown_id`,
  `qual_file::parse_lenient` and `ParseIssue`, `compact::prune_subject`,
  `compact::snapshot_subject`, `CompactResult::epochs`,
  `content_hash::SpanHashError`, `Error::AlreadyReported`, and the
  `threads` items `ThreadState`, `Thread::state`, `ThreadRenderer`,
  `thread_json`, and related helpers (SPEC §7).

### Changed

- **Breaking: `created_at` is hashed exactly as written.** Records keep
  the RFC 3339 text they were read with, IDs hash that text, and rewrites
  write it back unchanged; any valid RFC 3339 timestamp is accepted.
  Records qualifier creates use the canonical form (UTC, `Z`, 0/3/6/9
  fractional digits), as before. A record whose timestamp was written in
  another form (an offset, other fractional digits) now has the ID that
  hashing its text gives, instead of the ID of a re-encoded timestamp.
- **Breaking: `show` and `praise` render threads.** Both use the thread
  renderer of `threads`: each record line starts with its short ID, an open
  thread lists its live replies, and a closed thread is one line carrying
  its closing resolve (`— closed (fixed) by alice: Resolved`).
- **Breaking: `show` and `praise` exit 0 for an artifact with no
  records**, printing `No records found for '<artifact>'.` (or an empty
  `records` list in JSON). They used to exit 1.
- **Breaking: `qualifier agents <unknown topic>` exits 2** and prints
  `qualifier agents: no such topic '<topic>'. Available: …`, as AGENTS-CLI
  rule 4 requires. It used to exit 1.
- **Breaking: `--format` accepts only `human` or `json`** on every command;
  any other value is a usage error (exit 2). It used to fall back to human
  output.
- **Breaking: writes into ignored `.qual` files are refused.** `record`,
  `reply`, `resolve`, and `emit` fail when the target `.qual` file is
  hidden by `.gitignore`, `.ignore`, or `.qualignore` (no command would
  read the record), naming the rule; `--no-ignore` writes it anyway.
- **Breaking: `record --stdin` lines are checked like single records.**
  Overrides lines reject unknown keys and mistyped values. Complete-record
  lines have their `subject` normalized relative to the project root (and
  refused outside it), and their `supersedes`/`references` must name a live
  record, as on overrides lines.
- **Breaking: supersession is checked against the target record.** On
  every write path a `supersedes` target must exist and have the same
  subject as the new record; an unknown target is an error.
- **Breaking: `diff --fail-on` ignores edits.** Re-anchoring or rewording a
  record of a listed kind no longer trips it; a Changed entry trips it only
  when its kind moved into the list (e.g. `concern` to `blocker`).
- `compact` keeps thread structure: a superseded record that a kept record
  references is kept, with every record that supersedes it, so a resolved
  thread stays closed and its replies do not become threads of their own.
- `compact <artifact>` compacts only that artifact's records, in every
  `.qual` file that holds them, instead of every record in the files.
- `compact --snapshot` prunes first, so epoch `refs` and counts describe
  the surviving records, and refuses to fold an open `blocker` or
  `concern` thread into an epoch unless `--force` is given. Its success
  line reports the real epoch count.
- **Discovery skips malformed lines.** A line that is not a valid record
  is skipped with `warning: skipping <file>:<line>: <reason>` instead of
  failing every read command; `compact` still fails on it rather than drop
  it.
- `.gitignore` rules apply during discovery under every VCS (Mercurial,
  Jujutsu, ...), not only in git repositories.
- `diff` applies the working tree's ignore rules to `.qual` files read from
  the ref, so a newly ignored path is not reported as removed, and reads
  only `.qual` blobs from the ref's tree.
- `ls` counts only live records, leaves out `resolve` records, and with
  `--kind` counts only records of that kind.
- `review <subject>` resolves the subject relative to the current
  directory and matches everything under a directory.
- `review` reports why a span cannot be checked (file missing or
  unreadable, not UTF-8, span past the end of the file, span reversed)
  instead of calling every case "beyond file length".
- Records of custom types (written by `emit`) serialize their envelope in
  Metabox order and get a real BLAKE3 ID instead of `""`.
- `emit` resolves its subject like `record` does (relative to the current
  directory, stored relative to the project root) and validates a whole
  `--stdin` batch before writing.
- Only lowercase-hex targets of 4 or more characters are ID prefixes for
  `reply`/`resolve`; an extensionless file at the root (`Makefile`) is a
  location.
- **Breaking (library):** `created_at` fields are `Timestamp` instead of
  `DateTime<Utc>`; body structs have an `extra` field;
  `compute_span_hash` returns `Result<String, SpanHashError>`. The library
  surface is narrowed to what SPEC §7 lists: `qual_file::{resolve_qual_path,
  find_records_for, subject_name, detect_vcs}` and
  `annotation::parse_location` are no longer public,
  `qual_file::{find_qual_file_for, find_annotations_for}` are removed, and
  the `cli` module is hidden. Pin `qualifier = "0.9"`.

### Fixed

- A reversed span (`record x.rs:5:2`) is rejected with a message instead of
  panicking.
- `resovle` and other near-misses of `resolve` are reported as likely
  typos like the other built-in kinds.
- `record --stdin` writes the whole batch before printing, so a closed
  stdout cannot interrupt the writes, and exits cleanly on a broken pipe.
- Absolute locations that reach the project through a symlink resolve.
- `show --pretty` and `diff` label source context with the root-relative
  subject instead of an absolute path.
- `diff` uses the terminal width when `COLUMNS` is not exported.
- Supersession cycle checking takes linear time.
- Help text and module docs describe the issuer chain the code implements.

### Removed

- **Breaking: `ls --unqualified`**, which was never implemented.

## [0.8.0] — 2026-09-25

### Added

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
- `qualifier agents conventions` and `qualifier agents batch`.

### Fixed

- `record --stdin` detects the VCS issuer once per run instead of once per
  line.

### Changed

- `show` marks records whose issuer type is not `human`, e.g. `alex (ai)`.
- **`reply` and `resolve` refuse superseded and closed targets.** A target
  that has been superseded fails, naming the live record at the tip of its
  chain. A target whose chain ends in a `resolve` fails as closed, naming
  the closing record: reply to that record to comment on the closed
  thread, or record a new record that supersedes it to reopen the thread.
- `record --stdin` without `--continue-on-error` is all-or-nothing: every
  line is validated first, every failing line is reported, and nothing is
  written if any line fails. Previously lines before the first failure
  were written.
- **`supersedes`/`references` pointers must name a live record by full
  ID.** `record --supersedes`/`--references`, `reply --supersedes`, and
  the `supersedes`/`references` keys on `record --stdin` overrides lines
  take the full 64-character ID of a record that exists (for batch lines,
  on disk or on an earlier line) and is neither superseded nor closed.
  Previously the value was stored verbatim.
- Every `resolve` the CLI writes (`record resolve`, a `kind: "resolve"`
  line in `record --stdin`, `reply --kind resolve`, `resolve`) carries at
  most one `reason:*` tag, from the same vocabulary as `resolve --reason`.
- Commands now discover the whole project when run from a subdirectory,
  not just that subdirectory's `.qual` files.
- **Locations are relative to the current directory; subjects are stored
  relative to the project root.** `record` (single and `--stdin`
  `location` values), `reply`/`resolve` location targets, `threads`
  filters, and the `show`, `praise`, and `compact`
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
- `record --stdin` rejects `--file` instead of silently ignoring it.
- An ambiguous ID prefix lists the candidates, one
  `[id8] kind location "summary"` line each.

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

- **`qualifier diff [ref]`** — records added, resolved, or drifted since a
  git ref, compared against the merge base of `HEAD` and the ref by default
  (`--from-tip` for the ref's tip). `--fail-on KIND` and `--fail-on-drift`
  exit non-zero for CI gating; `--kind`, `--issuer-type`, and
  `--subjects-only` filter the output; `--format json` gives a
  machine-readable summary.
- **`qualifier record --stdin`** as the first-class batch path: JSONL
  overrides lines (`{"kind", "location", "message", ...}`) or complete
  records, one record per line.
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
