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
<svg class="propagation-svg" viewBox="0 0 700 150" fill="none" xmlns="http://www.w3.org/2000/svg" aria-label="Attestations feed scores into a dependency graph, where the worst dependency limits effective scores">
  <!--
    Every node has its own attestations shown beneath it.
    Node box shows: name + raw score + effective score.
    Attestations shown as small lines below each box.
    3 layers, 6 nodes. Compact.

    Scores:
      lib/crypto:  blocker -50, pass +30         → raw -20, eff -20
      lib/http:    praise +30, pass +20           → raw +50, eff +50
      lib/log:     pass +10                       → raw +10, eff +10
      src/auth.rs: pass +20, concern -10          → raw +10, eff -20 (limited by crypto)
      src/api.rs:  praise +30                     → raw +30, eff +10 (limited by log)
      bin/server:  pass +20, praise +30           → raw +50, eff -20 (limited by auth)
  -->

  <!-- ═══ L0: leaf libraries ═══ -->

  <!-- lib/crypto -->
  <rect x="0" y="0" width="120" height="36" fill="#f8717110" stroke="#f87171" stroke-width="1.5" rx="2"/>
  <text x="60" y="13" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" font-weight="600" fill="#eef0f6">lib/crypto</text>
  <text x="60" y="28" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="7.5" fill="#f87171">raw -20 · eff -20</text>
  <text x="4" y="50" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#f87171">-50 blocker</text>
  <text x="4" y="60" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#34d399">+30 pass</text>

  <!-- lib/http -->
  <rect x="0" y="72" width="120" height="36" fill="#34d39910" stroke="#34d399" stroke-width="1.5" rx="2"/>
  <text x="60" y="85" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" font-weight="600" fill="#eef0f6">lib/http</text>
  <text x="60" y="100" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="7.5" fill="#34d399">raw +50 · eff +50</text>
  <text x="4" y="122" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#34d399">+30 praise · +20 pass</text>

  <!-- lib/log -->
  <rect x="0" y="134" width="120" height="16" fill="#34d39910" stroke="#34d399" stroke-width="1" rx="2"/>
  <text x="60" y="146" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="8" font-weight="600" fill="#eef0f6">lib/log <tspan fill="#34d399" font-size="7">+10 pass</tspan></text>

  <!-- ═══ L1: mid-level ═══ -->

  <!-- src/auth.rs -->
  <rect x="230" y="14" width="130" height="36" fill="#f8717110" stroke="#f87171" stroke-width="1.5" rx="2"/>
  <text x="295" y="27" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" font-weight="600" fill="#eef0f6">src/auth.rs</text>
  <text x="295" y="42" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="7.5" fill="#f87171">raw +10 · eff -20</text>
  <text x="234" y="63" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#34d399">+20 pass</text>
  <text x="290" y="63" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#fbbf24">-10 concern</text>

  <!-- src/api.rs -->
  <rect x="230" y="80" width="130" height="36" fill="#34d39910" stroke="#34d399" stroke-width="1.5" rx="2"/>
  <text x="295" y="93" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="9" font-weight="600" fill="#eef0f6">src/api.rs</text>
  <text x="295" y="108" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="7.5" fill="#34d399">raw +30 · eff +10</text>
  <text x="234" y="128" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#34d399">+30 praise</text>

  <!-- ═══ L2: root ═══ -->

  <rect x="480" y="40" width="218" height="42" fill="#f8717110" stroke="#f87171" stroke-width="2" rx="2"/>
  <text x="589" y="56" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="10" font-weight="700" fill="#eef0f6">bin/server</text>
  <text x="589" y="72" text-anchor="middle" font-family="JetBrains Mono, monospace" font-size="8" fill="#f87171">raw +50 · eff -20</text>
  <text x="484" y="96" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#34d399">+20 pass · +30 praise</text>
  <text x="484" y="106" font-family="JetBrains Mono, monospace" font-size="6.5" fill="#6b7394">limited by lib/crypto via src/auth.rs</text>

  <!-- ═══ Edges ═══ -->

  <!-- lib/crypto → src/auth.rs  (limiting) -->
  <line x1="120" y1="18" x2="227" y2="28" stroke="#f87171" stroke-width="1.5"/>
  <polygon points="227,28 221,24 221,33" fill="#f87171"/>

  <!-- lib/http → src/auth.rs -->
  <line x1="120" y1="82" x2="227" y2="42" stroke="#3a4158" stroke-width="1"/>
  <polygon points="227,42 221,38 221,47" fill="#3a4158"/>

  <!-- lib/http → src/api.rs -->
  <line x1="120" y1="96" x2="227" y2="94" stroke="#3a4158" stroke-width="1"/>
  <polygon points="227,94 221,89 221,99" fill="#3a4158"/>

  <!-- lib/log → src/api.rs -->
  <path d="M 120,142 C 170,142 190,110 227,104" stroke="#3a4158" stroke-width="1" fill="none"/>
  <polygon points="227,104 221,99 221,109" fill="#3a4158"/>

  <!-- src/auth.rs → bin/server  (limiting) -->
  <line x1="360" y1="36" x2="477" y2="54" stroke="#f87171" stroke-width="2"/>
  <polygon points="477,54 470,50 471,59" fill="#f87171"/>

  <!-- src/api.rs → bin/server -->
  <line x1="360" y1="94" x2="477" y2="72" stroke="#3a4158" stroke-width="1"/>
  <polygon points="477,72 470,68 471,77" fill="#3a4158"/>
</svg>
</div>

`bin/server`'s own attestations give it a raw score of +50 — healthy. But it depends on `src/auth.rs` (eff: -20, limited by `lib/crypto`'s blocker), so its effective score drops to -20. Your effective score can never exceed your worst dependency.

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
| `qualifier` crate  | Library API for tools, agents, and editor plugins          |
| Dependency graph   | `qualifier.graph.jsonl` — feeds the propagation engine     |

See [Format](/format/) for the file spec, [CLI](/cli/) for command reference, or the full [Specification](/spec/).
