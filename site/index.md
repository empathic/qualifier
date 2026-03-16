---
layout: base.njk
title: Qualifier
nav: home
---

<div class="hero">
  <div class="hero-content">
    <h1>Qualifier</h1>
    <p class="tagline">
      Continuous Annotation, stored as files. CI and CD aren't enough
      anymore &mdash; CA records concerns, suggestions, and feedback the
      moment you see them. Humans and bots write the same format.
      No server, no database, just <code>.qual</code> files next to your source.
    </p>
    <div class="hero-install">
      <span class="prompt">$ </span>cargo install qualifier
      <button id="try-it-btn" class="try-it-btn">Try in browser</button>
    </div>
  </div>
  <svg class="topo topo-hero" viewBox="0 0 340 320" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
    <!-- Blueprint grid -->
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
    <!-- Annotation activity rising over time -->
    <polyline points="20,260 68,240 110,220 150,180 190,120 230,90 270,60 310,45"
      stroke="#34d399" stroke-width="2" opacity="0.5" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
    <circle cx="68" cy="240" r="3" fill="#f87171" opacity="0.6"/>
    <circle cx="150" cy="180" r="3" fill="#fbbf24" opacity="0.6"/>
    <circle cx="230" cy="90" r="3" fill="#34d399" opacity="0.6"/>
    <circle cx="310" cy="45" r="3" fill="#34d399" opacity="0.6"/>
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

Quality improvement gets batched behind gates. You see a problem now, but there's nowhere to put it until the next PR, the next sprint review, the next audit. Inline comments vanish into merged PRs. Three sprints later, nobody remembers what was flagged, what was fixed, and what was quietly ignored.

Qualifier lets you record quality signals **the moment you see them**. Every concern, suggestion, comment, and approval is a structured record in a `.qual` file next to your source — persistent, threaded, and VCS-native.

## Three core concepts

<div class="concepts">
  <div class="concept-card">
    <h3>Signals</h3>
    <p>Flag, comment, suggest, approve, reject. Record quality observations the moment you see them. No process, no PR required. Immutable once written, resolved when fixed.</p>
  </div>
  <div class="concept-card">
    <h3>Conversations</h3>
    <p>Reply to any signal, build threaded discussions, resolve when done. Conversations that survive merges, rebases, and the passage of time.</p>
  </div>
  <div class="concept-card">
    <h3>Ambient Review</h3>
    <p>Review isn't a gate you pass through &mdash; it's a practice that's always on. Flag any file, any time, whether it changed today or three years ago. Nothing slips through because there's no window to miss.</p>
  </div>
</div>

## Ambient review

Traditional code review is a gate: you see the diff, approve or reject, move on. This has two problems. First, you can only flag what's in the current diff — concerns about existing code have nowhere to go. Second, feedback evaporates when PRs merge. Three months later, nobody remembers what was discussed, what was deferred, and what was quietly ignored.

Ambient review removes the gate. Flag a concern about any file, any time — whether the code changed today or three years ago. Reply to existing signals. Resolve them when they're fixed. The record persists in `.qual` files that travel with your source through merges, rebases, and team turnover.

Annotations don't rot. Every signal is pinned to the exact code you were looking at. When that code changes, `qualifier review` tells you what's drifted and what's still fresh — no archaeology required. Concerns follow their code through rebases, squashes, and refactors. When you re-confirm a concern against changed code, that's a signal too.

Because annotations are structured and machine-readable, bots and AI agents participate in the same review process as humans. A CI pipeline can flag a vulnerability. A code health agent can suggest a refactoring. A teammate can reply to either. All signals live in the same format, in the same files, in your repo.

## A day with Qualifier

You're reading `src/auth.rs` and notice the login handler isn't sanitizing input. You could file a ticket — but it'll sit in a backlog nobody triages. You could fix it now — but you're in the middle of something else. You could mention it in Slack — but it'll scroll away by lunch.

```bash
qualifier flag src/auth.rs:42 "SQL injection risk in login handler"
```

Five seconds. A teammate replies Thursday, the fix lands Friday, you close it:

```bash
qualifier reply a1b2 "Parameterized queries added in 8f3c2a1"
qualifier resolve a1b2
```

The concern was recorded when it was seen, discussed in context, and resolved when fixed. The entire history lives in your repo — `git blame`, `git log`, and `git diff` all work on it. No tickets, no ceremonies, no context switching.

*"But don't I have to commit the `.qual` file?"* Yes — and you can push it straight to main. Annotations are append-only: adding your observation never modifies anyone else's, and **merge conflicts are structurally impossible** when adding annotations. Two people flagging concerns in the same file on different branches just works. That's not an accident — the format was designed for exactly this. (Compaction — periodic cleanup of resolved signals — is the one operation that rewrites the file and should be coordinated, like any maintenance task.)

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

## Get started

Try Qualifier in your browser above, or install locally:

```bash
cargo install qualifier
qualifier init
```

See [CLI](/cli/) for the full command reference or [Format](/format/) for the file spec.
