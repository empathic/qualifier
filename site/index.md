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
      anymore, CA records concerns, suggestions, and feedback the
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

Someone dropped 30,000 lines of slopcode in your lap and now you need to figure
out if it does what it says on the tin. The test suite passes (mostly), the docs
are "coming soon," and the last meaningful code review was three sprints ago.
Where do you even start?

The problems you spot aren't associated with a current PR, so there's nowhere to
put them. Qualifier gives you somewhere: a structured, VCS-native record pinned
to the exact lines you read. Don't wait for review to annotate.  Don't forget if
a given PR doesn't include it. Let people and agents get to the work when it
makes sense.

## Three core concepts

<div class="concepts">
  <div class="concept-card">
    <h3>Signals</h3>
    <p>Flag, comment, suggest, approve, reject. Record quality observations the
    moment you see them. No process, no PR required. Immutable once written,
    resolved when fixed.</p>
  </div>
  <div class="concept-card">
    <h3>Conversations</h3>
    <p>Reply to any signal, build threaded discussions, resolve when done.
    Conversations that survive merges, rebases, and the passage of time.</p>
  </div>
  <div class="concept-card">
    <h3>Ambient Review</h3>
    <p>Review isn't a gate you pass through, it's a practice that's
    always on. Flag any file, any time, whether it changed today or three years
    ago. Nothing slips through because there's no window to miss.</p>
  </div>
</div>

## Ambient review

Code review today only catches what's in the current diff. Qualifier removes that constraint. Flag any file, any time. Annotations are pinned to the exact code you were looking at. When that code changes, `qualifier review` surfaces what's drifted.

Bots and humans write the same format. A CI pipeline flags a vulnerability. A code health agent suggests a refactoring. A teammate replies to either. Same files, same repo.

## A day with Qualifier

You're reading `src/auth.rs` and notice the login handler isn't sanitizing
input. You could file a ticket, fix it yourself, or mention it in Slack. Instead:

```bash
qualifier flag src/auth.rs:42 "SQL injection risk in login handler"
```

Five seconds. The concern is recorded against the file and line, pinned to the
exact code you're looking at. You go back to what you were doing.

Thursday, a teammate sees it and replies:

```bash
qualifier reply a1b2 "Parameterized queries added in 8f3c2a1"
```

Friday, you verify the fix and close it:

```bash
qualifier resolve a1b2
```

The whole conversation lives in your repo. `git blame`, `git log`, `git diff`
all work on `.qual` files. Two months later, the code under your flag changes
again. `qualifier review` surfaces the drift automatically.

Annotations are append-only, so **merge conflicts are structurally impossible**.
Push straight to main. No branch, no PR, no ceremony.

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
