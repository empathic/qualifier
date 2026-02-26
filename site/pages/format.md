---
layout: base.njk
title: Format
nav: format
permalink: /format/
---

# The .qual format

<p class="subtitle">
Append-only JSONL. One record per line. VCS-native by design.
</p>

A `.qual` file is a UTF-8 encoded file where each line is a complete JSON object representing one record. This is JSONL (JSON Lines).

```jsonl
{"v":3,"type":"attestation","artifact":"src/parser.rs","kind":"concern","score":-30,"summary":"Panics on malformed input","author":"alice@example.com","author_type":"human","created_at":"2026-02-24T10:00:00Z","ref":"git:3aba500","id":"a1b2c3d4..."}
{"v":3,"type":"attestation","artifact":"src/parser.rs","kind":"praise","score":40,"summary":"Excellent test coverage","author":"bob@example.com","author_type":"human","created_at":"2026-02-24T11:00:00Z","id":"e5f6a7b8..."}
```

## Record types

Every record has a `type` field that identifies its schema. Qualifier defines three record types:

| Type          | Description                              |
| ------------- | ---------------------------------------- |
| `attestation` | A quality signal (the primary type)      |
| `epoch`       | A compaction snapshot                    |
| `dependency`  | A dependency edge between artifacts      |

When `type` is omitted, it defaults to `"attestation"`.

## Attestation schema

Each attestation is a JSON object with these fields:

| Field           | Type     | Required | Description                                      |
| --------------- | -------- | -------- | ------------------------------------------------ |
| `v`             | integer  | yes      | Format version (always 3)                        |
| `type`          | string   | yes*     | Record type (`"attestation"`). *May be omitted.  |
| `artifact`      | string   | yes      | Qualified name of the artifact                   |
| `span`          | object   | no       | Sub-artifact range (line/col addressing)         |
| `kind`          | enum     | yes      | Type of attestation (see below)                  |
| `score`         | integer  | yes      | Signed quality delta, -100..100                  |
| `summary`       | string   | yes      | Human-readable one-liner                         |
| `detail`        | string   | no       | Extended description (markdown allowed)          |
| `suggested_fix` | string   | no       | Actionable suggestion for improvement            |
| `tags`          | string[] | no       | Freeform classification tags                     |
| `author`        | string   | yes      | Who or what created this attestation             |
| `author_type`   | enum     | no       | Author classification: human, ai, tool, unknown  |
| `created_at`    | string   | yes      | RFC 3339 timestamp                               |
| `ref`           | string   | no       | VCS ref pin (e.g. "git:3aba500"), opaque string  |
| `supersedes`    | string   | no       | ID of a prior attestation this replaces          |
| `id`            | string   | yes      | Content-addressed BLAKE3 hash                    |

## Attestation kinds

| Kind         | Default Score | Meaning                                        |
| ------------ | ------------- | ---------------------------------------------- |
| `pass`       | +20           | Meets a stated quality bar                     |
| `fail`       | -20           | Does NOT meet a stated quality bar             |
| `blocker`    | -50           | Blocking issue, must resolve before release    |
| `concern`    | -10           | Non-blocking issue worth tracking              |
| `praise`     | +30           | Positive recognition of quality                |
| `suggestion` | -5            | Proposed improvement (often with suggested_fix)|
| `waiver`     | +10           | Acknowledged issue, explicitly accepted        |

When `--score` is omitted from `qualifier attest`, the CLI uses the default score for the given kind.

## Supersession

Attestations are immutable. To "update" a signal, write a new attestation with `supersedes` pointing to the prior ID. Only the latest in a chain contributes to scoring.

## Content-addressed IDs

Record IDs are BLAKE3 hashes of the **Qualifier Canonical Form (QCF)** — a deterministic JSON serialization with fixed field order, no whitespace, and `id` set to `""` during hashing.

```json
{"v":3,"type":"attestation","artifact":"src/parser.rs","kind":"concern","score":-30,"summary":"Panics on malformed input","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":""}
```

Optional fields (`span`, `detail`, `suggested_fix`, `tags`, `author_type`, `ref`, `supersedes`) are omitted from the canonical form when absent — the hash changes only when a field is actually present.

This ensures identical records always produce identical IDs, regardless of implementation language.

## Compaction

Append-only files grow. Compaction reclaims space:

```bash
qualifier compact src/parser.rs              # prune superseded
qualifier compact src/parser.rs --snapshot   # collapse to epoch
qualifier compact src/parser.rs --dry-run    # preview first
```

Compaction MUST NOT change the raw score. If it does, the implementation has a bug.

## File placement

| Strategy         | Example              | Tradeoff                           |
| ---------------- | -------------------- | ---------------------------------- |
| Per-directory    | `src/.qual`          | Clean tree, good merge behavior    |
| Per-file         | `src/parser.rs.qual` | Maximum merge isolation            |
| Per-project      | `.qual` at root      | Simplest setup, more contention    |

The recommended layout is one `.qual` file per directory. `qualifier attest` defaults to this.

<svg class="topo topo-wide" viewBox="0 0 900 40" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <line x1="0" y1="20" x2="900" y2="20" stroke="#818cf8" stroke-width="0.5" opacity="0.1"/>
  <line x1="0" y1="0" x2="0" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="150" y1="10" x2="150" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="300" y1="0" x2="300" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="450" y1="10" x2="450" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="600" y1="0" x2="600" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="750" y1="10" x2="750" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
  <line x1="900" y1="0" x2="900" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.06"/>
</svg>

## Dependency graph

Qualifier consumes a dependency graph as `qualifier.graph.jsonl`:

```jsonl
{"artifact":"bin/server","depends_on":["lib/auth","lib/http","lib/db"]}
{"artifact":"lib/auth","depends_on":["lib/crypto"]}
{"artifact":"lib/http","depends_on":[]}
```

The graph MUST be a DAG. Cycles are rejected.

