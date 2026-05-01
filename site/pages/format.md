---
layout: base.njk
title: Format
nav: format
prose: true
permalink: /format/
---

# The .qual format

<p class="subtitle">
A friendly tour of the .qual file format.
</p>

A `.qual` file is how Qualifier records structured observations about code.
Concerns, suggestions, anything worth keeping is stored in plain UTF-8 encoded
JSONL. Append-only, merge friendly, one record per line, sitting next to your
source. This page is the orientation: enough to read one fluently and write one
by hand. The full reference is the [spec](/spec/).

Files live next to your source. An annotation about `src/parser.rs`
typically lives in `src/.qual` (one file per directory) or
`src/parser.rs.qual` (one file per source file). Either works, and `git`
treats them like any other text file. `git blame`, `git log`, `git diff` all
do the right thing.

A two-record `.qual` file looks like this:

```jsonl
{"metabox":"1","type":"annotation","subject":"src/auth.rs","issuer":"mailto:alice@example.com","issuer_type":"human","created_at":"2026-02-24T10:00:00Z","id":"a1b2...","body":{"kind":"concern","summary":"SQL injection risk in login handler"}}
{"metabox":"1","type":"annotation","subject":"src/auth.rs","issuer":"urn:anthropic:claude","issuer_type":"ai","created_at":"2026-02-24T11:00:00Z","id":"e5f6...","body":{"kind":"comment","references":"a1b2...","summary":"Switched the handler to parameterized queries in 8f3c2a1"}}
```

A human reviewer flags a concern; an AI agent replies with a fix, threaded
to the original via `references`.

## Anatomy of a record

Those two records, expanded side by side:

{% codecompare "json",
  "Comment (Record 1)",
'{
  "metabox": "1",
  "type": "annotation",
  "subject": "src/auth.rs",
  "issuer": "mailto:alice@example.com",
  "issuer_type": "human",
  "created_at": "2026-02-24T10:00:00Z",
  "id": "a1b2...",
  "body": {
    "kind": "concern",
    "summary": "SQL injection risk in login handler"
  }
}',
  "Response (Record 2)",
'{
  "metabox": "1",
  "type": "annotation",
  "subject": "src/auth.rs",
  "issuer": "urn:anthropic:claude",
  "issuer_type": "ai",
  "created_at": "2026-02-24T11:00:00Z",
  "id": "e5f6...",
  "body": {
    "kind": "comment",
    "references": "a1b2...",
    "summary": "Switched the handler to parameterized queries in 8f3c2a1"
  }
}'
%}

There are two halves. The **envelope** (everything outside `body`) is
who-said-what-about-which-subject-when. The **body** is what they actually
said. Every record in a `.qual` file has the same envelope shape — that's
why the two records above look nearly identical at the top. The body varies
by record type.

That split is intentional: tools that don't understand a particular body
schema can still read the envelope and route the record sensibly.

## The envelope

Every record carries the same eight fields. One sentence each:

- `metabox` — envelope version, always `"1"` for now.
- `type` — what kind of record this is (`annotation`, `epoch`, `dependency`, ...). Defaults to `"annotation"` when absent.
- `subject` — the artifact this record is about, usually a path like `src/parser.rs`.
- `issuer` — who or what wrote it, as a URI (`mailto:`, `https:`, or `urn:`).
- `issuer_type` — optional; `human`, `ai`, `tool`, or `unknown`.
- `created_at` — RFC 3339 timestamp.
- `id` — a BLAKE3 hash of the record itself, so identical records always get identical IDs.
- `body` — the type-specific payload.

That's the [Metabox envelope](/metabox/), specced separately so other tools
can adopt the same shape. If you want field-level depth (validation rules,
URI schemes, canonical ordering), the [spec](/spec/#22-metabox-envelope)
has it.

## What kinds of records?

Annotations are the primary record type, but `.qual` is a substrate. The
format supports any structured record type that fits the envelope, and
tools are required to preserve records they don't understand. That means
a `.qual` file can carry ecosystem signals from many sources without
the format itself needing to grow.

The types defined in the spec today:

- `annotation` — a quality signal (concern, praise, blocker, comment, ...). The one you'll write most often.
- `epoch` — a compaction snapshot. Synthesizes a chunk of history into one scored record.
- `dependency` — declares that one subject depends on others, so scores can propagate.
- `license` — a license declaration for a subject.
- `security-advisory` — a known vulnerability or weakness.
- `perf-measurement` — a performance measurement against a baseline.

The first three ship in the CLI today. The rest are spec-level: the format
defines them so adopters can produce them, and any tool that round-trips a
`.qual` file will preserve them whether or not it knows how to interpret them.

For full schemas, see [section 3 of the spec](/spec/#3-record-type-specifications).

## Threads and resolution

Two body fields turn a flat list of records into a conversation.

`references` is a lightweight "re:" link. Bob sees Alice's concern, replies
with a comment, and points `body.references` at Alice's record ID. Both
records stay active in scoring; the link is purely for threading.

`supersedes` is stronger. A new record with `body.supersedes` set to a prior
record's ID withdraws the prior record from scoring. That's how you "edit"
something in an immutable, append-only file: write a new record that replaces
the old one. The `resolve` annotation kind is the canonical way to close
something out, withdrawing the score of whatever it supersedes.

```jsonl
{
  "metabox": "1",
  "type": "annotation",
  "subject": "src/parser.rs",
  "issuer": "mailto:bob@example.com",
  "created_at": "2026-03-01T10:00:00Z",
  "id": "b2c3...",
  "body": {
    "kind": "comment",
    "references": "a1b2...",
    "summary": "Good catch, fixed in 8f3c2a1"
  }
}
```

Tools render threads with tree-drawing characters so the conversation reads
naturally in a terminal.

## Scoring, briefly

Annotations carry an integer `score` in the range -100 to +100. A `concern`
is negative, a `praise` is positive, a `comment` is unscored. The CLI picks
sensible defaults per kind so you rarely set the number by hand.

Scoring is deterministic: given the same set of records, every implementation
computes the same raw score for every subject. When you have a dependency
graph, scores propagate across edges so a problem in `lib/auth` shows up in
`bin/server` too. The full rules (raw vs. effective scores, propagation,
status thresholds) live in [section 4 of the spec](/spec/#4-scoring).

## Why JSONL?

Three reasons, all about the format being boring on purpose.

**Append-only means clean merges.** Two people writing annotations to the
same file at the same time produce two new lines at the end. Git merges them
trivially because there's no editing in place. You can push straight to main.

**Content-addressed IDs are stable.** Each record's `id` is a hash of its
canonical form. Identical records produce identical IDs across machines,
languages, and time. That's what makes `references` and `supersedes` work
without a coordinating server.

**Human-writable means anyone can produce it.** A reviewer with a text editor,
a CI script with `jq`, an AI agent with a function call. Same format, same
file, no special tooling required to participate. The CLI is a convenience,
not a gatekeeper.

## Where next?

- [Spec](/spec/) — the canonical reference, including every field, every kind, and every rule.
- [CLI](/cli/) — the command reference for the `qualifier` binary.
- [Metabox](/metabox/) — the envelope format on its own, in case you want to use it elsewhere.
