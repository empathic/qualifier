# Qualifier Specification

**Version:** 0.1.0-draft
**Status:** Draft
**Authors:** Alex Kesling

---

## Abstract

Qualifier is a deterministic system for recording, propagating, and querying
quality attestations against software artifacts. It provides a VCS-friendly
file format (`.qual`), a Rust library (`libqualifier`), and a CLI binary
(`qualifier`) that together enable humans and agents to annotate code with
structured quality signals — and to compute aggregate quality scores that
propagate through dependency graphs.

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

## 2. Concepts

### 2.1 Artifact

An **artifact** is any addressable unit of software that can be qualified.
Artifacts are identified by a **qualified name** (a string), which SHOULD
correspond to a logical unit in the codebase:

- A file path: `src/parser.rs`
- A module: `crate::parser`
- A build target: `//services/auth:lib`
- A test suite: `tests/integration/auth_test.rs`

Qualifier does not enforce a naming scheme. The names are opaque strings.
Conventions are a project-level decision.

#### 2.1.1 Artifact Renames

Qualifier identifies artifacts by their qualified name. Renaming an artifact
(e.g., `src/parser.rs` to `src/ast_parser.rs`) requires the following steps:

1. Rename the `.qual` file to match the new artifact name
   (`src/parser.rs.qual` -> `src/ast_parser.rs.qual`).
2. Update the `qualifier.graph.jsonl` to reference the new name wherever the
   old name appeared (both as `artifact` and in `depends_on` arrays).
3. **Note:** Attestations inside the renamed `.qual` file still contain
   `"artifact": "src/parser.rs"` in their JSON. Since attestation IDs are
   content-addressed, changing the `artifact` field would change the ID,
   breaking supersession chains.

The RECOMMENDED workflow after a rename is:

1. Rename the `.qual` file and update the graph file.
2. Run `qualifier compact <new-name> --snapshot` to collapse history into a
   fresh epoch under the new name.
3. Commit the rename, graph update, and compacted file together.

Alternatively, re-attest the artifact under its new name and let the old
attestations age out through compaction.

> **Note:** Tooling to automate artifact renames is out of scope for v0.1 but
> is anticipated as a future `qualifier rename` command.

### 2.2 Attestation

An **attestation** is a single, immutable quality signal about an artifact.
Attestations are the atoms of the system. Each attestation records:

| Field         | Type       | Required | Description |
|---------------|------------|----------|-------------|
| `artifact`    | string     | yes      | Qualified name of the artifact |
| `kind`        | enum       | yes      | The type of attestation (see 2.3) |
| `score`       | integer    | yes      | Signed quality delta, -100..100 |
| `summary`     | string     | yes      | Human-readable one-liner |
| `detail`      | string     | no       | Extended description, markdown allowed |
| `suggested_fix` | string   | no       | Actionable suggestion for improvement |
| `tags`        | string[]   | no       | Freeform classification tags |
| `author`      | string     | yes      | Who or what created this attestation |
| `created_at`  | string     | yes      | RFC 3339 timestamp |
| `supersedes`  | string     | no       | ID of a prior attestation this replaces |
| `epoch_refs`  | string[]   | no       | IDs of compacted attestations (epoch only; see 2.6.1) |
| `id`          | string     | yes      | Unique attestation ID (see 2.5) |

### 2.3 Attestation Kinds

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
| `epoch`       | Synthetic compaction summary (see 2.6.1) |

#### 2.3.1 Recommended Score Ranges

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
| `epoch`       | (computed)    | n/a               | any |

When the `--score` flag is omitted from `qualifier attest`, the CLI SHOULD
use the default score for the given kind. The `--score` flag always takes
precedence over defaults.

These are guidance, not constraints. Implementations MUST NOT reject an
attestation solely because its score falls outside the recommended range.

### 2.4 Supersession

Attestations are immutable once written. To "update" a signal, you write a new
attestation with a `supersedes` field pointing to the prior attestation's `id`.

The superseding and superseded attestations MUST refer to the same artifact.
An attestation for artifact A MUST NOT supersede an attestation for artifact B.
Implementations MUST reject attestations that violate this constraint during
validation.

When computing scores, a superseded attestation MUST be excluded from the
calculation. Only the latest attestation in a supersession chain contributes.

Supersession chains MUST be acyclic. Implementations MUST detect and reject
cycles.

### 2.5 Attestation IDs

An attestation ID is a lowercase hex-encoded BLAKE3 hash of the **Qualifier
Canonical Form (QCF)** of the attestation, with the `id` field set to the
empty string `""` during hashing. This makes IDs deterministic and
content-addressed.

#### 2.5.1 Qualifier Canonical Form (QCF)

To ensure that every implementation — regardless of language or JSON library —
produces identical bytes for the same attestation, the canonical serialization
MUST obey the following rules:

1. **Field order.** Fields MUST appear in exactly this order:

   `artifact`, `kind`, `score`, `summary`, `detail`, `suggested_fix`, `tags`,
   `author`, `created_at`, `supersedes`, `epoch_refs`, `id`

2. **Absent optional fields.** Optional fields whose value is absent (null,
   None, etc.) MUST be omitted from the serialization entirely. Likewise,
   `tags` MUST be omitted when the array is empty. The omittable fields are:
   `detail`, `suggested_fix`, `tags` (when empty), `supersedes`, and
   `epoch_refs`.

3. **Whitespace.** No whitespace between tokens. No space after `:` or `,`.
   No trailing newline. The output is a single compact JSON line.

4. **No trailing commas.** Standard JSON — no trailing commas in objects or
   arrays.

5. **String encoding.** Strings use JSON's standard escaping (RFC 8259
   Section 7). Implementations MUST NOT add escapes beyond what JSON
   requires (e.g., do not escape `/`).

6. **Number encoding.** `score` is serialized as a bare integer with no
   leading zeros, no decimal point, and no exponent notation. Negative
   values use a leading `-`.

7. **`id` field.** During hashing, `id` MUST be set to `""` (the empty
   string), not omitted. This is the sole exception to the omission rule:
   `id` is always present.

**Example.** Given an attestation with `detail: None`, `suggested_fix: None`,
`tags: []`, `supersedes: None`, `epoch_refs: None`, the QCF is:

```json
{"artifact":"src/parser.rs","kind":"concern","score":-30,"summary":"Panics on malformed input","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":""}
```

Note that `detail`, `suggested_fix`, `tags`, `supersedes`, and `epoch_refs`
are omitted because they are absent/empty.

> **Rationale.** This scheme matches the behavior of the Rust reference
> implementation's `serde_json::to_string` with `#[serde(skip_serializing_if)]`
> annotations. It is simpler than RFC 8785 (JSON Canonicalization Scheme) and
> sufficient for Qualifier's needs. Implementations in other languages MUST
> replicate this exact byte sequence.

### 2.6 Compaction

Append-only files grow without bound. **Compaction** is the mechanism for
reclaiming space while preserving scoring correctness. Since `.qual` files
live in VCS, the full pre-compaction history is always recoverable from
VCS history.

A compaction rewrites a `.qual` file by:

1. **Pruning** all superseded attestations. If attestation B supersedes A,
   only B is retained. The entire supersession chain collapses to its tip.
2. **Optionally snapshotting.** When `--snapshot` is passed, all surviving
   attestations are replaced by a single **epoch attestation** that captures
   the net state. This is the most aggressive form of compaction.

#### 2.6.1 Epoch Attestations

An epoch attestation is a synthetic attestation written by the compactor:

| Field         | Value |
|---------------|-------|
| `kind`        | `epoch` |
| `score`       | The raw score at compaction time |
| `summary`     | `"Compacted from N attestations"` |
| `detail`      | Optional: summary of what was folded in |
| `author`      | `"qualifier/compact"` |
| `tags`        | `["epoch"]` |
| `supersedes`  | Not set (epoch attestations do not form chains) |
| `epoch_refs`  | Array of IDs of the attestations that were compacted |

The `epoch_refs` field is unique to epoch attestations. It exists solely for
auditability — it lets you trace back (via VCS history) to the individual
attestations that were folded in.

`epoch` is added to the attestation kinds table. Implementations MUST treat
it as a normal attestation for scoring purposes.

#### 2.6.2 Compaction Rules

- Compaction MUST NOT change the raw score of the artifact. This is the
  invariant. If compaction changes the score, the implementation has a bug.
- Compaction MUST be an explicit, committed operation — never automatic or
  silent. It produces a visible diff in VCS.
- After compaction, the file is a valid `.qual` file. No special reader
  support is needed; older tooling that doesn't know about `epoch` treats
  it as a `Custom("epoch")` kind, which scores normally.
- `qualifier compact --dry-run` MUST be supported so users can preview the
  result before committing.

#### 2.6.3 Recommended Workflow

```
qualifier compact src/parser.rs --dry-run   # preview
qualifier compact src/parser.rs             # prune superseded attestations
# review and commit the compaction using your VCS of choice
```

For aggressive compaction:

```
qualifier compact src/parser.rs --snapshot  # collapse to single epoch
```

### 2.7 The `.qual` File Format

A `.qual` file is a UTF-8 encoded file where each line is a complete JSON
object representing one attestation. This is JSONL (JSON Lines).

**Placement:** A `.qual` file can contain attestations for any artifacts
in its directory or subdirectories. The `artifact` field in each JSON
attestation line is the authoritative identifier — not the filename.

**Layout strategies:**

| Strategy | Example | Pros | Cons |
|----------|---------|------|------|
| **Per-directory** (recommended) | `src/.qual` for files in `src/` | Clean tree, fewer files, good merge behavior | Slightly more merge contention than 1:1 |
| Per-file | `src/parser.rs.qual` | Maximum merge isolation | Noisy file tree, many small files |
| Per-project | `.qual` at repo root | Simplest setup | High merge contention in teams |

The **recommended** layout is one `.qual` file per directory, containing
attestations for files directly in that directory. `qualifier attest`
defaults to this layout, writing to `{dir}/.qual`. If a 1:1 file
(`{artifact}.qual`) already exists, `qualifier attest` writes there
instead for backwards compatibility. The `--file` flag overrides the
target file explicitly.

All layouts are backwards-compatible and can coexist in the same project.
Discovery scans all `.qual` files regardless of layout, and scoring uses
the `artifact` field from each attestation line to associate scores.

**Rules:**
- Each line MUST be a valid JSON object conforming to the attestation schema.
- Lines MUST be separated by a single `\n` (LF).
- The file MUST end with a trailing `\n`.
- Empty lines and lines starting with `//` are ignored (comments).
- Implementations MUST preserve ordering; older attestations come first.
- New attestations MUST be appended, never inserted.
- The sole exception to append-only is **compaction** (see 2.6), which
  rewrites the file. Compaction is always an explicit, user-initiated action.

**Example:**

```jsonl
{"artifact":"src/parser.rs","kind":"concern","score":-30,"summary":"Panics on malformed UTF-8 input instead of returning an error","suggested_fix":"Replace .unwrap() on line 42 with proper error propagation","tags":["robustness","error-handling"],"author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":"a1b2c3d4..."}
{"artifact":"src/parser.rs","kind":"praise","score":40,"summary":"Excellent property-based test coverage","tags":["testing"],"author":"bob@example.com","created_at":"2026-02-24T11:00:00Z","id":"e5f6a7b8..."}
```

## 3. Scoring

### 3.1 Raw Score

The **raw score** of an artifact is the sum of the `score` fields of all
non-superseded attestations for that artifact, clamped to the range
`[-100, 100]`.

```
raw_score(A) = clamp(-100, 100, sum(attestation.score for non-superseded attestations of A))
```

An artifact with no attestations has a raw score of **0** (unqualified).

### 3.2 Effective Score

The **effective score** of an artifact is a function of its raw score and the
effective scores of its dependencies:

```
effective_score(A) = min(raw_score(A), min(effective_score(D) for D in deps(A)))
```

In plain English: your effective score can never be higher than your worst
dependency's effective score. Quality flows downhill.

If an artifact has no dependencies, its effective score equals its raw score.

If an artifact has no attestations but has dependencies, its effective score is
the minimum effective score of its dependencies (the "inherited floor").

### 3.3 Dependency Graph Input

Qualifier does not analyze source code. It consumes a **dependency graph** as
an input file. The graph format is a simple adjacency list in JSONL:

```jsonl
{"artifact":"bin/server","depends_on":["lib/auth","lib/http","lib/db"]}
{"artifact":"lib/auth","depends_on":["lib/crypto"]}
{"artifact":"lib/http","depends_on":[]}
```

This file is conventionally named `qualifier.graph.jsonl` and placed at the
repository root. It can be generated from build tools (Bazel, Cargo, etc.)
or maintained by hand.

The dependency graph MUST be a DAG (directed acyclic graph). Implementations
MUST detect and reject cycles.

## 4. CLI Interface

The CLI binary is named `qualifier`.

> **Tip:** Users who want a shorter command can create a shell alias
> (e.g., `alias qual=qualifier`) or a symlink. A future packaging enhancement
> may ship `qual` as a built-in alias.

### 4.1 Core Commands

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

### 4.2 `qualifier attest`

Interactive and non-interactive attestation creation.

```
qualifier attest src/parser.rs \
  --kind concern \
  --score -30 \
  --summary "Panics on malformed input" \
  --suggested-fix "Use proper error propagation" \
  --tag robustness \
  --tag error-handling \
  --author "alice@example.com"
```

`--summary` is required in non-interactive mode. When omitted, the command
returns an error.

> **Future:** A planned enhancement will open `$EDITOR` for interactive entry
> when `--summary` is omitted and stdin is a TTY. This is not yet implemented
> in the reference implementation.

When `--score` is omitted, the CLI uses the recommended default score for the
given kind (see section 2.3.1). For example, `--kind blocker` without
`--score` defaults to -50.

`--file <path>` writes the attestation to a specific `.qual` file instead
of using the default layout resolution (see 2.7).

When `--author` is omitted, defaults to the VCS user identity (see 7.4).

### 4.3 `qualifier show`

```
qualifier show src/parser.rs

  src/parser.rs
  Raw score:       10
  Effective score: -20 (limited by lib/crypto)

  Attestations (2):
    [-30] concern  "Panics on malformed input"        alice  2026-02-24
    [+40] praise   "Excellent property test coverage"  bob    2026-02-24
```

### 4.4 `qualifier score`

```
qualifier score

  ARTIFACT              RAW    EFF   STATUS
  lib/crypto            -20    -20   ██░░░░░░░░  blocker
  lib/auth               60    -20   ██░░░░░░░░  blocker
  lib/http               80     80   ████████░░  healthy
  bin/server             45    -20   ██░░░░░░░░  blocker
```

### 4.5 `qualifier check`

Designed for CI pipelines. Returns exit code 0 if all artifacts meet the
threshold, non-zero otherwise.

```
qualifier check --min-score 0
```

Outputs failing artifacts to stderr so CI logs are actionable.

### 4.6 `qualifier ls`

```
qualifier ls --below 0
qualifier ls --kind blocker
qualifier ls --unqualified         # artifacts with no attestations
```

### 4.7 `qualifier compact`

Compacts a `.qual` file by pruning superseded attestations. See section 2.6.

```
qualifier compact src/parser.rs              # prune superseded attestations
qualifier compact src/parser.rs --snapshot   # collapse to a single epoch attestation
qualifier compact src/parser.rs --dry-run    # preview without writing
qualifier compact --all                      # compact every .qual file in the repo
qualifier compact --all --dry-run            # preview repo-wide compaction
```

Output:

```
qualifier compact src/parser.rs
  src/parser.rs.qual: 47 -> 12 attestations (35 superseded, pruned)

qualifier compact src/parser.rs --snapshot
  src/parser.rs.qual: 47 -> 1 attestation (epoch, raw score: 10)
```

### 4.8 `qualifier init`

Sets up a repo for qualifier:

```
qualifier init
  Created qualifier.graph.jsonl (empty — populate with your dependency graph)
  Detected VCS: git
  Added *.qual merge=union to .gitattributes
```

When no VCS is detected:

```
qualifier init
  Created qualifier.graph.jsonl (empty — populate with your dependency graph)
  No VCS detected — skipping merge configuration (see SPEC.md section 7)
```

### 4.9 Configuration

Qualifier uses layered configuration. Each layer overrides the one below it.
Precedence (highest wins):

| Priority | Source | Example |
|----------|--------|---------|
| 1 (highest) | CLI flags | `--graph path/to/graph.jsonl` |
| 2 | Environment variables | `QUALIFIER_GRAPH`, `QUALIFIER_AUTHOR`, `QUALIFIER_FORMAT`, `QUALIFIER_MIN_SCORE` |
| 3 | Project config | `.qualifier.toml` in the project root |
| 4 | User config | `~/.config/qualifier/config.toml` |
| 5 (lowest) | Built-in defaults | See below |

**Configuration keys:**

| Key         | CLI flag       | Env var              | Default |
|-------------|----------------|----------------------|---------|
| `graph`     | `--graph`      | `QUALIFIER_GRAPH`    | `qualifier.graph.jsonl` |
| `author`    | `--author`     | `QUALIFIER_AUTHOR`   | VCS identity (see 7.4) |
| `format`    | `--format`     | `QUALIFIER_FORMAT`   | `human` |
| `min_score` | `--min-score`  | `QUALIFIER_MIN_SCORE`| `0` |

**Example `.qualifier.toml`:**

```toml
graph = "build/deps.graph.jsonl"
author = "ci-bot@example.com"
format = "human"
min_score = 0
```

### 4.10 `qualifier blame`

Delegates to the underlying VCS blame/annotate command for the artifact's
`.qual` file. This shows who added each attestation and when.

```
qualifier blame src/parser.rs
```

VCS support:
- **Git:** delegates to `git blame src/parser.rs.qual`
- **Mercurial:** delegates to `hg annotate src/parser.rs.qual`
- **Other:** prints guidance to run the VCS blame command manually

See also Section 7.3.

## 5. Library API

The `qualifier` crate exposes its library API from `src/lib.rs`. Library
consumers add `qualifier = { version = "0.1", default-features = false }` to
avoid pulling in CLI dependencies.

```rust
// qualifier::attestation
pub struct Attestation { /* fields per spec section 2.2 */ }
pub enum Kind { Pass, Fail, Blocker, Concern, Praise, Suggestion, Waiver, Epoch, Custom(String) }
pub fn generate_id(attestation: &Attestation) -> String;
pub fn validate(attestation: &Attestation) -> Vec<String>;
pub fn validate_supersession_targets(attestations: &[Attestation]) -> Vec<String>;
pub fn finalize(attestation: Attestation) -> Attestation;

// qualifier::qual_file
pub struct QualFile { pub path: PathBuf, pub artifact: String, pub attestations: Vec<Attestation> }
pub fn parse(path: &Path) -> Result<QualFile>;
pub fn append(path: &Path, attestation: &Attestation) -> Result<()>;
pub fn discover(root: &Path) -> Result<Vec<QualFile>>;

// qualifier::graph
pub struct DependencyGraph { /* adjacency list */ }
pub fn load(path: &Path) -> Result<DependencyGraph>;

// qualifier::scoring
pub struct ScoreReport { pub raw: i32, pub effective: i32, pub limiting_path: Option<Vec<String>> }
pub fn raw_score(attestations: &[Attestation]) -> i32;
pub fn effective_scores(graph: &DependencyGraph, qual_files: &[QualFile]) -> HashMap<String, ScoreReport>;

// qualifier::compact
pub struct CompactResult { pub before: usize, pub after: usize, pub pruned: usize }
pub fn prune(qual_file: &QualFile) -> (QualFile, CompactResult);
pub fn snapshot(qual_file: &QualFile) -> (QualFile, CompactResult);
```

The library is the source of truth. The CLI is a thin wrapper around it.

## 6. Agent Integration

Qualifier is designed to be used by AI coding agents. Key affordances:

- **Structured output:** `qualifier score --format json` and
  `qualifier show --format json` emit machine-readable JSON for agent
  consumption.
- **Batch attestation:** `qualifier attest --stdin` reads JSONL attestations
  from stdin, allowing agents to qualify many artifacts in one pass.
- **Suggested fixes:** The `suggested_fix` field gives agents (and humans) a
  concrete action to take. A qualifying agent can read these and attempt fixes.
- **Priority ordering:** `qualifier ls --below 0 --format json` gives agents a
  prioritized worklist of what to fix next.

## 7. VCS Integration

`.qual` files SHOULD be committed to version control. Qualifier is
VCS-agnostic by design — the append-only JSONL format is friendly to any
system that tracks text files (Git, Mercurial, Jujutsu, Pijul, Fossil,
Subversion, etc.).

### 7.1 General Principles

- The `.qual` format's append-only nature minimizes merge conflicts in any
  VCS. Concurrent appends to different lines are structurally conflict-free
  in most merge algorithms.
- Pre-compaction history is recoverable from VCS history regardless of the
  VCS used. Compaction (section 2.6) is designed around this assumption.
- `qualifier init` detects the active VCS and applies appropriate
  configuration (see below). When no VCS is detected, it skips
  VCS-specific setup.

### 7.2 VCS-Specific Setup (`qualifier init`)

| VCS        | Action |
|------------|--------|
| Git        | Adds `*.qual merge=union` to `.gitattributes` |
| Mercurial  | Adds `**.qual = union` merge pattern to `.hgrc` |
| Other      | Prints guidance for manual merge configuration |

### 7.3 `qualifier blame`

`qualifier blame <artifact>` shows per-line attribution for a `.qual` file.
It delegates to the underlying VCS blame/annotate command:

- Git: `git blame`
- Mercurial: `hg annotate`
- Fallback: not available (prints guidance)

### 7.4 Author Defaults

When `--author` is omitted from `qualifier attest`, the CLI infers the
author from VCS configuration:

- Git: `git config user.email`
- Mercurial: `hg config ui.username`
- Fallback: `$USER@localhost`

## 8. File Discovery

Qualifier discovers `.qual` files by walking the directory tree from the
project root. Each `.qual` file may contain attestations for multiple
artifacts. The `artifact` field in each attestation JSON line is the
authoritative identifier — the file path is not used to determine which
artifact an attestation belongs to.

File path conventions for reference:

- `src/parser.rs.qual` — 1:1 layout (one file per artifact)
- `src/.qual` — per-directory layout (one file for all artifacts in `src/`)
- `.qual` — per-project layout (one file at the repo root)

The project root is determined by searching upward for VCS markers (`.git`,
`.hg`, `.jj`, `.pijul`, `_FOSSIL_`, `.svn`) or a `qualifier.graph.jsonl`
file, whichever is found first.

## 9. Crate Structure

A single crate published as `qualifier` on crates.io. Add it as a library
dependency (`qualifier = "0.1"`) or install the binary
(`cargo install qualifier`).

The `Cargo.toml` declares both a `[lib]` and a `[[bin]]` target. Library
consumers only pull in the library code; the binary depends on `clap` and
friends behind a default `cli` feature so they don't pollute library builds.

```toml
[features]
default = ["cli"]
cli = ["dep:clap", "dep:comfy-table", "dep:figment"]
```

```
qualifier/
├── Cargo.toml
├── SPEC.md                    # This document
├── qualifier.graph.jsonl      # Example / self-hosted graph
└── src/
    ├── lib.rs                 # Public library API — re-exports core modules
    ├── attestation.rs         # Attestation type, ID generation, validation
    ├── qual_file.rs           # .qual file parsing, appending, discovery
    ├── graph.rs               # Dependency graph loading, cycle detection
    ├── scoring.rs             # Raw + effective score computation
    ├── compact.rs             # Compaction: prune and snapshot operations
    ├── bin/
    │   └── qualifier.rs       # Binary entry point (calls cli::run)
    └── cli/                   # CLI module (behind "cli" feature)
        ├── mod.rs             # Clap parser, command dispatch
        ├── config.rs          # Configuration loading (figment)
        ├── output.rs          # Human + JSON output formatting
        └── commands/          # One module per subcommand
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

Library consumers disable the default feature to avoid CLI dependencies:

```toml
[dependencies]
qualifier = { version = "0.1", default-features = false }
```

## 10. Dependencies (Recommended)

| Crate       | Purpose |
|-------------|---------|
| `serde`     | Serialization of attestations and graph |
| `serde_json`| JSON parsing and emission |
| `clap`      | CLI argument parsing (derive API) |
| `blake3`    | Attestation ID hashing |
| `chrono`    | RFC 3339 timestamp handling |
| `petgraph`  | Dependency graph representation and cycle detection |
| `comfy-table` | Terminal table output |
| `figment`   | Layered configuration (TOML + env vars) |

## 11. Future Considerations (Out of Scope for v0.1)

These are explicitly **not** part of v0.1 but are anticipated:

- **Qualifier policies:** Project-level config for custom scoring weights,
  required attestation kinds before merge, etc.
- **Editor plugins:** LSP-based inline display of scores and attestations.
- **`qualifier watch`:** File-watcher mode for continuous scoring.
- **Attestation signatures:** Cryptographic signing of attestations for
  supply-chain integrity.
- **Remote qualifier servers:** Aggregation across multiple repositories.
- **Decay:** Time-based score decay to encourage re-qualification of stale
  attestations.
- **`qualifier rename`:** Automated artifact rename with `.qual` file and
  graph migration (see section 2.1.1).
- **`$EDITOR` interactive mode:** Open an editor for attestation creation
  when `--summary` is omitted (see section 4.2).

---

*The Koalafier has spoken. Now go qualify some code.*
