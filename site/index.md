---
layout: base.njk
title: Qualifier
nav: home
---

<div class="hero">
  <div class="hero-content">
    <h1>Qualifier</h1>
    <p class="tagline">
      <strong>Know your code.</strong> A deterministic system for recording quality attestations
      and blockers against software artifacts. Quality scores that propagate through your
      dependency graph &mdash; no server, no database, just files.
    </p>
    <div class="hero-install">
      <span class="prompt">$ </span>cargo install qualifier
      <button id="try-it-btn" class="try-it-btn">Try in browser</button>
    </div>
  </div>
  <svg class="topo topo-hero" viewBox="0 0 340 320" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
    <!-- Blueprint grid — quality score coordinate system -->
    <!-- Major grid -->
    <line x1="0" y1="0" x2="340" y2="0" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="0" y1="64" x2="340" y2="64" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="0" y1="128" x2="340" y2="128" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="0" y1="192" x2="340" y2="192" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="0" y1="256" x2="340" y2="256" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="0" y1="320" x2="340" y2="320" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="0" y1="0" x2="0" y2="320" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="68" y1="0" x2="68" y2="320" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="136" y1="0" x2="136" y2="320" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="204" y1="0" x2="204" y2="320" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="272" y1="0" x2="272" y2="320" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <line x1="340" y1="0" x2="340" y2="320" stroke="#818cf8" stroke-width="0.5" opacity="0.15"/>
    <!-- Score curve — quality trend rising -->
    <polyline points="20,260 68,240 110,220 150,180 190,120 230,90 270,60 310,45"
      stroke="#34d399" stroke-width="2" opacity="0.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
    <!-- Threshold line -->
    <line x1="0" y1="160" x2="340" y2="160" stroke="#fbbf24" stroke-width="1" opacity="0.3" stroke-dasharray="6 4"/>
    <text x="6" y="154" font-family="JetBrains Mono, monospace" font-size="8" fill="#fbbf24" opacity="0.5">THRESHOLD</text>
    <!-- Score data points -->
    <circle cx="68" cy="240" r="3" fill="#f87171" opacity="0.6"/>
    <circle cx="150" cy="180" r="3" fill="#fbbf24" opacity="0.6"/>
    <circle cx="230" cy="90" r="3" fill="#34d399" opacity="0.6"/>
    <circle cx="310" cy="45" r="3" fill="#34d399" opacity="0.6"/>
    <!-- Axis labels -->
    <text x="6" y="312" font-family="JetBrains Mono, monospace" font-size="7" fill="#6b7394" opacity="0.6">-100</text>
    <text x="6" y="12" font-family="JetBrains Mono, monospace" font-size="7" fill="#6b7394" opacity="0.6">+100</text>
    <!-- Blocker zone -->
    <rect x="0" y="200" width="340" height="120" fill="#f87171" opacity="0.03"/>
  </svg>
</div>
<div class="divider"></div>

<div id="playground-section" class="playground" hidden>
<h2>Try it</h2>
<p class="playground-desc">
Explore Qualifier in your browser. Real <code>qualifier</code> commands, real output.
</p>
<script>window.__PLAYGROUND_FILES__ = {{ playgroundFiles | dump | safe }};</script>
<div id="playground-terminal" class="playground-terminal"></div>
</div>
<script src="/js/playground.js"></script>

## The problem

Someone dropped 30,000 lines of slopcode in your lap and now you need to figure out if it does what it says on the tin. The test suite passes (mostly), the docs are "coming soon," and the last meaningful code review was three sprints ago. Where do you even start?

Qualifier gives you a structured, VCS-friendly way to **record what you know about code quality** — and a scoring model that propagates those signals through your dependency graph so you always know where the bodies are buried.

## Three core concepts

<div class="concepts">
  <div class="concept-card">
    <h3>Attestation</h3>
    <p>A single quality signal about an artifact. A blocker, a concern, a praise, a pass. Immutable once written, superseded by newer signals.</p>
  </div>
  <div class="concept-card">
    <h3>Score</h3>
    <p>Sum of attestations, clamped to [-100,&thinsp;100]. Raw score is local. Effective score propagates through the dependency graph — your worst dependency is your ceiling.</p>
  </div>
  <div class="concept-card">
    <h3>Graph</h3>
    <p>A DAG of artifact dependencies. Quality flows downhill. A pristine binary that links a cursed library inherits the curse.</p>
  </div>
</div>

## How scores propagate

<div class="propagation-figure">
<svg class="propagation-svg" viewBox="0 0 700 220" fill="none" xmlns="http://www.w3.org/2000/svg" aria-label="Score propagation diagram showing how a low-quality dependency drags down a high-quality binary">
  <!-- lib/crypto: raw -20, eff -20 (blocker) -->
  <rect x="0" y="88" width="130" height="50" fill="#f8717115" stroke="#f87171" stroke-width="1.5" rx="2"/>
  <text x="65" y="108" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="11" font-weight="600" fill="#d0d5e3">lib/crypto</text>
  <text x="65" y="126" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" fill="#f87171">raw: -20  eff: -20</text>
  <!-- lib/http: raw 50, eff 50 (healthy) -->
  <rect x="0" y="168" width="130" height="50" fill="#34d39915" stroke="#34d399" stroke-width="1.5" rx="2"/>
  <text x="65" y="188" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="11" font-weight="600" fill="#d0d5e3">lib/http</text>
  <text x="65" y="206" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" fill="#34d399">raw: 50  eff: 50</text>
  <!-- src/auth.rs: raw -30, eff -30 -->
  <rect x="220" y="48" width="130" height="50" fill="#f8717115" stroke="#f87171" stroke-width="1.5" rx="2"/>
  <text x="285" y="68" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="11" font-weight="600" fill="#d0d5e3">src/auth.rs</text>
  <text x="285" y="86" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" fill="#f87171">raw: -30  eff: -30</text>
  <!-- src/parser.rs: raw 5, eff 5 -->
  <rect x="220" y="168" width="130" height="50" fill="#34d39915" stroke="#34d399" stroke-width="1.5" rx="2"/>
  <text x="285" y="188" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="11" font-weight="600" fill="#d0d5e3">src/parser.rs</text>
  <text x="285" y="206" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" fill="#34d399">raw: 5  eff: 5</text>
  <!-- bin/server: raw 50, eff -30 -->
  <rect x="480" y="88" width="130" height="50" fill="#f8717115" stroke="#f87171" stroke-width="2.5" rx="2"/>
  <text x="545" y="108" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="11" font-weight="700" fill="#d0d5e3">bin/server</text>
  <text x="545" y="126" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" fill="#f87171">raw: 50  eff: -30</text>
  <!-- Edges -->
  <line x1="130" y1="113" x2="220" y2="73" stroke="#f87171" stroke-width="1.5"/>
  <line x1="350" y1="73" x2="480" y2="108" stroke="#f87171" stroke-width="2" />
  <line x1="130" y1="193" x2="480" y2="118" stroke="#6b7394" stroke-width="1" stroke-dasharray="5 3"/>
  <!-- Labels -->
  <text x="545" y="78" text-anchor="middle" font-family="Instrument Sans, sans-serif" font-size="9" font-weight="600" fill="#f87171" letter-spacing="0.06em" text-transform="uppercase">LIMITED BY AUTH</text>
  <text x="65" y="78" text-anchor="middle" font-family="Instrument Sans, sans-serif" font-size="9" font-weight="600" fill="#f87171" letter-spacing="0.06em">BLOCKER</text>
  <text x="285" y="38" text-anchor="middle" font-family="Instrument Sans, sans-serif" font-size="9" font-weight="600" fill="#f87171" letter-spacing="0.06em">BLOCKER</text>
  <!-- Arrow tips -->
  <polygon points="220,73 214,68 214,78" fill="#f87171"/>
  <polygon points="480,108 474,103 474,113" fill="#f87171"/>
  <polygon points="480,118 474,113 474,123" fill="#6b7394"/>
</svg>
</div>

`bin/server` has a raw score of 50 — its own attestations are healthy. But it depends on `src/auth.rs` (effective: -30), so its effective score drops to -30. Quality can never exceed your worst dependency.

## What Qualifier adds

| What                     | Without Qualifier             | With Qualifier                                      |
| ------------------------ | ----------------------------- | --------------------------------------------------- |
| Quality tracking         | Spreadsheets, tickets, memory | Structured `.qual` files in your repo                |
| Score propagation        | Manual dependency analysis    | Automatic through the dependency graph               |
| CI gating                | Custom scripts                | `qualifier check --min-score 0`                      |
| Agent integration        | None                          | JSON output, batch attestation, suggested fixes      |
| Merge conflicts          | Guaranteed with shared files  | Structurally impossible (append-only JSONL)           |
| History                  | Lost in ticket graveyards     | VCS-native — blame, diff, bisect all work            |

## Minimal example

A `.qual` file is just JSONL — one attestation per line:

```jsonl
{"artifact":"src/parser.rs","kind":"concern","score":-30,"summary":"Panics on malformed UTF-8 input","author":"alice@example.com","created_at":"2026-02-24T10:00:00Z","id":"a1b2c3d4..."}
{"artifact":"src/parser.rs","kind":"praise","score":40,"summary":"Excellent property-based test coverage","author":"bob@example.com","created_at":"2026-02-24T11:00:00Z","id":"e5f6a7b8..."}
```

No parents, no headers, no schema declarations. Each line is self-contained.

## Quick start

```bash
# Install
cargo install qualifier

# Initialize qualifier in your repo
qualifier init

# Attest a concern
qualifier attest src/parser.rs --kind concern --score -30 \
  --summary "Panics on malformed input"

# See scores for everything
qualifier score

# CI gate — fail if anything is below zero
qualifier check --min-score 0

# Show details for one artifact
qualifier show src/parser.rs

# List the worst offenders
qualifier ls --below 0
```

<svg class="topo topo-wide" viewBox="0 0 900 60" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <!-- Blueprint ruler marks -->
  <line x1="0" y1="30" x2="900" y2="30" stroke="#818cf8" stroke-width="0.5" opacity="0.1"/>
  <line x1="0" y1="0" x2="0" y2="60" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="100" y1="20" x2="100" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="200" y1="0" x2="200" y2="60" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="300" y1="20" x2="300" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="400" y1="0" x2="400" y2="60" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="500" y1="20" x2="500" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="600" y1="0" x2="600" y2="60" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="700" y1="20" x2="700" y2="40" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="800" y1="0" x2="800" y2="60" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
  <line x1="900" y1="0" x2="900" y2="60" stroke="#818cf8" stroke-width="0.5" opacity="0.08"/>
</svg>

## How it works

Qualifier is a Rust crate with a library and a CLI:

| Component          | What it does                                               |
| ------------------ | ---------------------------------------------------------- |
| `.qual` files      | VCS-friendly JSONL attestations — the primary interface    |
| `qualifier` CLI    | Human-friendly commands for attesting, scoring, gating     |
| `libqualifier`     | Library API for tools, agents, and editor plugins          |
| Dependency graph   | `qualifier.graph.jsonl` — feeds the propagation engine     |

See [Format](/format/) for the file spec, [CLI](/cli/) for command reference, or the full [Specification](/spec/).
