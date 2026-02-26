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
{"metabox":"1","type":"attestation","subject":"src/parser.rs","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":"a1b2c3d4...","body":{"author_type":"human","kind":"concern","ref":"git:3aba500","score":-30,"summary":"Panics on malformed input"}}
{"metabox":"1","type":"attestation","subject":"src/parser.rs","author":"bob@example.com","created_at":"2026-02-24T11:00:00Z","id":"e5f6a7b8...","body":{"author_type":"human","kind":"praise","score":40,"summary":"Excellent test coverage"}}
```

## Record types

Every record has a `type` field that identifies its schema. Qualifier defines three record types:

| Type          | Description                              |
| ------------- | ---------------------------------------- |
| `attestation` | A quality signal (the primary type)      |
| `epoch`       | A compaction snapshot                    |
| `dependency`  | A dependency edge between subjects       |

When `type` is omitted, it defaults to `"attestation"`. Unknown types are preserved as opaque pass-through data.

## Record envelope

All record types share a common **Metabox envelope** — a fixed set of fields that answer "who said what about which subject, when", plus a type-specific `body` object. The record envelope is an instance of the [Metabox](/metabox/) envelope format.

| Field        | Type    | Required | Description                                      |
| ------------ | ------- | -------- | ------------------------------------------------ |
| `metabox`    | string  | yes      | Envelope version (always `"1"`)                  |
| `type`       | string  | yes*     | Record type identifier. *Defaults to `"attestation"`. |
| `subject`    | string  | yes      | Qualified name of the target artifact            |
| `author`     | string  | yes      | Who or what created this record                  |
| `created_at` | string  | yes      | RFC 3339 timestamp                               |
| `id`         | string  | yes      | Content-addressed BLAKE3 hash                    |
| `body`       | object  | yes      | Type-specific payload                            |

## Attestation schema

Attestations are the primary record type. Envelope fields plus body:

| Field           | Type     | Required | Description                                      |
| --------------- | -------- | -------- | ------------------------------------------------ |
| `author_type`   | enum     | no       | Author classification: human, ai, tool, unknown  |
| `detail`        | string   | no       | Extended description (markdown allowed)          |
| `kind`          | enum     | yes      | Type of attestation (see below)                  |
| `ref`           | string   | no       | VCS ref pin (e.g. "git:3aba500"), opaque string  |
| `score`         | integer  | yes      | Signed quality delta, -100..100                  |
| `span`          | object   | no       | Sub-artifact range (line/col addressing)         |
| `suggested_fix` | string   | no       | Actionable suggestion for improvement            |
| `summary`       | string   | yes      | Human-readable one-liner                         |
| `supersedes`    | string   | no       | ID of a prior attestation this replaces          |
| `tags`          | string[] | no       | Freeform classification tags                     |

Body fields are in alphabetical order (MCF canonical form).

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

## Epoch schema

An **epoch** is a compaction snapshot — a synthetic record that replaces a set of attestations with a single scored record preserving the net score. Envelope fields plus body:

| Field         | Type     | Required | Description                                       |
| ------------- | -------- | -------- | ------------------------------------------------- |
| `author_type` | enum     | no       | Always `"tool"` for epochs                        |
| `refs`        | string[] | yes      | IDs of the compacted records                      |
| `score`       | integer  | yes      | Net score at compaction time                      |
| `span`        | object   | no       | Sub-artifact range                                |
| `summary`     | string   | yes      | `"Compacted from N records"`                      |

Epoch `author` is always `"qualifier/compact"`.

```json
{"metabox":"1","type":"epoch","subject":"src/parser.rs","author":"qualifier/compact","created_at":"2026-02-25T12:00:00Z","id":"f9e8d7c6...","body":{"author_type":"tool","refs":["a1b2...","c3d4..."],"score":10,"summary":"Compacted from 12 records"}}
```

## Dependency schema

A **dependency** record declares directed edges from one subject to others. Envelope fields plus body:

| Field        | Type     | Required | Description                                        |
| ------------ | -------- | -------- | -------------------------------------------------- |
| `depends_on` | string[] | yes      | Subject names this subject depends on              |

```json
{"metabox":"1","type":"dependency","subject":"bin/server","author":"build-system","created_at":"2026-02-25T10:00:00Z","id":"1a2b3c4d...","body":{"depends_on":["lib/auth","lib/http"]}}
```

Dependency records don't carry scores. They feed the propagation engine that computes effective scores.

## Supersession

Attestations are immutable. To "update" a signal, write a new attestation with `body.supersedes` pointing to the prior ID. Only the latest in a chain contributes to scoring.

## Content-addressed IDs

Record IDs are BLAKE3 hashes of the **Metabox Canonical Form (MCF)** — a deterministic JSON serialization with fixed envelope field order, alphabetical body field order, no whitespace, and `id` set to `""` during hashing.

```json
{"metabox":"1","type":"attestation","subject":"src/parser.rs","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":"","body":{"kind":"concern","score":-30,"summary":"Panics on malformed input"}}
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
{"subject":"bin/server","depends_on":["lib/auth","lib/http","lib/db"]}
{"subject":"lib/auth","depends_on":["lib/crypto"]}
{"subject":"lib/http","depends_on":[]}
```

The graph MUST be a DAG. Cycles are rejected.
