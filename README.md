# Qualifier

**Continuous Annotation, stored as files.** Record concerns, suggestions, and
feedback against code the moment you see them — humans and bots writing the
same format. No server, no database, just `.qual` files next to your source.

```bash
cargo install qualifier
qualifier record concern src/auth.rs:42 "SQL injection risk in login handler"
```

The annotation is pinned to that file and line, stored as a content-addressed
JSONL record in `src/.qual`, and `git blame`/`git log`/`git diff` all work on
it. Append-only, so concurrent edits don't conflict structurally.

## What it's for

Quality work is batched behind PRs and gates. You notice a problem on Tuesday
but the PR isn't until Friday. Inline comments disappear into merged PRs.
Three sprints later nobody remembers what was flagged, what was fixed, and
what was quietly ignored.

Qualifier is **ambient review**: annotate any file, any time, whether it
changed today or three years ago. Annotations thread into conversations,
survive merges and rebases, and surface drift automatically when the code
underneath them changes.

## Quick tour

```bash
# Record a concern at a specific line
qualifier record concern src/parser.rs:42 "Panics on malformed input"

# See it
qualifier show src/parser.rs

# Reply (id-prefix or location)
qualifier reply a1b2 "Good catch — fixed in 8f3c2a1"

# Close it
qualifier resolve a1b2

# Find drift in span-bound annotations after the source changed
qualifier review src/parser.rs
```

> **For AI coding agents:** qualifier implements [AGENTS-CLI 0.1](AGENTS-CLI.md). Run `qualifier agents` for a self-contained guide.

## CLI

### For AI agents

| Command | Description |
|---------|-------------|
| `qualifier agents [topic]` | Self-contained guide for AI coding agents (start here) |

### Record observations

| Command | Description |
|---------|-------------|
| `qualifier record <kind> <location> [message]` | Record an annotation |
| `qualifier reply <target> <message>` | Reply to an existing record |
| `qualifier resolve <target> [message]` | Resolve (close) a record |
| `qualifier emit <type> <subject> --body '<JSON>'` | Emit a raw record of any type |

`<kind>` is one of `concern`, `comment`, `suggestion`, `pass`, `fail`,
`blocker`, `praise`, `waiver`, `resolve`, or any custom string. `<location>`
is a path with an optional span (`src/foo.rs:42`, `src/foo.rs:42:58`).
`<target>` is an id-prefix (≥4 chars) or a `<location>`.

### Inspect annotations

| Command | Description |
|---------|-------------|
| `qualifier show <artifact>` | Show annotations for an artifact (threaded) |
| `qualifier ls [--kind K]` | List artifacts (optionally by kind) |
| `qualifier praise <artifact>` | Show who annotated and why (alias: `blame`) |
| `qualifier review [subject]` | Check freshness of span-bound annotations |

### Maintain

| Command | Description |
|---------|-------------|
| `qualifier compact <artifact>` | Prune superseded records or snapshot to an epoch |

All read commands accept `--format json` for machine-readable output.

## Records

Annotations are the primary record type, but `.qual` is a substrate. The
[Metabox](METABOX.md) envelope wraps any type-specific body — annotations,
epochs (compaction snapshots), dependencies, licenses, security advisories,
performance measurements, or any custom URI-typed record. Tools that don't
understand a body type still round-trip the record unchanged.

A two-record `.qual` file:

```jsonl
{"metabox":"1","type":"annotation","subject":"src/auth.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-24T10:00:00Z","id":"a1b2…","body":{"kind":"concern","summary":"SQL injection risk in login handler"}}
{"metabox":"1","type":"annotation","subject":"src/auth.rs","issuer":"urn:anthropic:claude","issuer_type":"ai","created_at":"2026-02-24T11:00:00Z","id":"e5f6…","body":{"kind":"comment","references":"a1b2…","summary":"Switched to parameterized queries in 8f3c2a1"}}
```

Records are immutable and content-addressed (BLAKE3). Updates use
supersession chains rather than mutation. Compaction prunes superseded
records or collapses history into epoch records.

## File layout

`.qual` files can sit per-directory (`src/.qual` — recommended), per-file
(`src/parser.rs.qual`), or per-project (`.qual` at the root). All layouts
coexist. Discovery respects `.gitignore` and `.qualignore`; pass
`--no-ignore` to bypass.

For collaborative repos, configure your VCS to use union merges on `.qual`
files (e.g. `*.qual merge=union` in `.gitattributes` for git) so concurrent
appends don't collide.

## Agent integration

- `--format json` on `show` and `ls` for structured output
- `qualifier record --stdin` and `qualifier emit --stdin` accept JSONL on stdin
- `body.suggested_fix` carries actionable remediation
- `body.span` targets specific line ranges
- `--issuer-type ai` distinguishes agent-authored records in display

## Documentation

- [SPEC.md](SPEC.md) — full format specification
- [METABOX.md](METABOX.md) — the envelope format
- [CHANGELOG.md](CHANGELOG.md) — release notes

## License

MIT OR Apache-2.0
