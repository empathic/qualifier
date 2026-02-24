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

### 2.4 Supersession

Attestations are immutable once written. To "update" a signal, you write a new
attestation with a `supersedes` field pointing to the prior attestation's `id`.

When computing scores, a superseded attestation MUST be excluded from the
calculation. Only the latest attestation in a supersession chain contributes.

Supersession chains MUST be acyclic. Implementations MUST detect and reject
cycles.

### 2.5 Attestation IDs

An attestation ID is a lowercase hex-encoded BLAKE3 hash of the canonical
serialization of the attestation (with the `id` field set to the empty string
during hashing). This makes IDs deterministic and content-addressed.

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

**Placement:** `.qual` files live alongside the artifacts they describe.
A file `src/parser.rs` has its qualifications in `src/parser.rs.qual`.
A directory-level qualification file (e.g. `src/.qual`) applies to the
directory as a whole, treated as its own artifact.

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

The CLI binary is named `qualifier` (or `qual` as a short alias).

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

When run without `--summary`, opens `$EDITOR` for interactive entry.

When `--author` is omitted, defaults to the VCS user identity (see 7.4).

### 4.3 `qualifier show`

```
qualifier show src/parser.rs

  src/parser.rs
  Raw score:       10
  Effective score: -20 (limited by lib/crypto @ -20)

  Attestations (2):
    [-30] concern  "Panics on malformed input"        alice  2026-02-24
    [+40] praise   "Excellent property test coverage"  bob    2026-02-24
```

### 4.4 `qualifier score`

```
qualifier score

  ARTIFACT              RAW    EFF   STATUS
  lib/crypto            -20    -20   ██░░░░░░░░  blocker
  lib/auth               60    -20   ██░░░░░░░░  limited by lib/crypto
  lib/http               80     80   ████████░░  healthy
  bin/server             45    -20   ██░░░░░░░░  limited by lib/crypto
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

## 5. Library API

The `qualifier` crate exposes its library API from `src/lib.rs`. Library
consumers add `qualifier = { version = "0.1", default-features = false }` to
avoid pulling in CLI dependencies.

```rust
// qualifier::attestation
pub struct Attestation { /* fields per spec section 2.2 */ }
pub enum Kind { Pass, Fail, Blocker, Concern, Praise, Suggestion, Waiver, Custom(String) }

// qualifier::qual_file
pub struct QualFile { pub path: PathBuf, pub attestations: Vec<Attestation> }
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
system that tracks text files (Git, Mercurial, Jess, Pijul, Fossil,
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
- Fallback: `$USER@$(hostname)` or prompts

## 8. File Discovery

Qualifier discovers `.qual` files by walking the directory tree from the
project root. It associates each `.qual` file with its artifact by stripping
the `.qual` suffix:

- `src/parser.rs.qual` -> artifact `src/parser.rs`
- `src/.qual` -> artifact `src/` (directory-level)

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
cli = ["dep:clap", "dep:comfy-table"]
```

```
qualifier/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public library API — re-exports core modules
│   ├── attestation.rs      # Attestation type, ID generation, validation
│   ├── qual_file.rs        # .qual file parsing, appending, discovery
│   ├── graph.rs            # Dependency graph loading, cycle detection
│   ├── scoring.rs          # Raw + effective score computation
│   └── bin/                # CLI binary (behind "cli" feature)
│       ├── main.rs
│       ├── commands/       # One module per subcommand
│       │   ├── attest.rs
│       │   ├── show.rs
│       │   ├── score.rs
│       │   ├── ls.rs
│       │   ├── check.rs
│       │   ├── graph.rs
│       │   └── init.rs
│       └── output.rs       # Human + JSON output formatting
├── qualifier.graph.jsonl   # Example / self-hosted graph
└── SPEC.md                 # This document
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
| `comfy-table` or `tabled` | Terminal table output |

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

---

*The Koalafier has spoken. Now go qualify some code.*
