# Qualifier Brand Book

Quality is forensic. Qualifier records who said what about which artifact,
when, and with what verdict — flags, suggestions, approvals, threads,
resolutions, scores that propagate through a dependency graph. The visual
identity has to feel like a manual that earned its precision through use,
not aesthetic posturing.

The system holds two readings in tension. **Blueprint** — graph paper, ink,
dimensional precision, the engineer's notebook open at a desk. **Logbook** —
`git log` output, JSONL records, console UI, terminal-ready text. Both
sensibilities want the same things: a single accent color that says "the
system noted this," cool dark grounds that don't fight the highlighter, a
monospace register that's load-bearing rather than decorative. Where they
disagree, the blueprint wins on color and the logbook wins on layout.

Two themes, one cool cast. **Dark is canonical** — qualifier is read where
logs are read, on a cool near-black surface under fluorescent light. **Light
ships as a pencil-on-paper inversion**: cool off-white grounds (never warm
beige), graphite text, an indigo-600 accent that meets WCAG AA against the
lifted surfaces. The surface ramp stays cool-cast so the grid paper still
reads as blueprint paper, not a kraft sheet. Theme is selected via the
tri-state nav toggle (system / dark / light) and persisted under the
`qualifier-theme` localStorage key. Never neutral gray, never warm beige,
never decorative gradients.

This document is the source of truth for site implementation.

## Family

Qualifier sits in the Empathic family of sibling brands:

| Sibling             | Voice                                      | Palette signal     |
| ------------------- | ------------------------------------------ | ------------------ |
| `empathic/site`     | Parent. Reticle, scope-and-target.         | Cool warm-cast     |
| `empathic/pathbase` | Night Garden. Forest floor, growth.        | Emerald + gold     |
| `empathic/toolpath` | Workshop Terminal. Tools and contour.      | Copper + parchment |
| **Qualifier**       | Inspector's Logbook. Blueprint annotation. | **Indigo + slate** |

What's shared across the family: sharp corners, tracked-uppercase mono
chrome, dark grounds, no decorative gradients, an `@layer reset, base,
layout, components` cascade target. What's distinct: the palette, the body
type, and the primary visual artifact. Toolpath's distinctive marks are
copper, the DAG, and FIG_NNN; qualifier's distinctive marks are **indigo**,
the **graph-paper grid**, the **threaded record output**, and the **score
readout**.

## Canonical tokens

A single source of truth. Components reference tokens, never hex.

### Surfaces (the lift ramp)

| Token     | Dark      | Light     | Purpose                                            |
| --------- | --------- | --------- | -------------------------------------------------- |
| `--void`  | `#0c0e11` | `#f5f7fa` | Page ground. The desk surface.                     |
| `--slab`  | `#141720` | `#eaeef4` | Code blocks, hero panels, code-compare panes.      |
| `--plate` | `#1b1f2a` | `#dde2eb` | Inline `<code>`, hovered cards, recessed surfaces. |
| `--ridge` | `#252a38` | `#c8cdd9` | Default 1px separator; card borders.               |

Surfaces lift in pairs (`void` → `slab` → `plate`). On dark, they lift
without ever brightening into anything you'd call "blue." On light, they
recess from off-white toward a cool slate without ever warming toward
beige. Borders use `--ridge` rather than text color at low alpha so cards
stack against either ground without re-specification.

### Text ramp (graphite to chalk)

| Token     | Dark      | Light     | Purpose                                            |
| --------- | --------- | --------- | -------------------------------------------------- |
| `--steel` | `#3a4158` | `#9098ad` | Hint text, code comments, contour visuals.         |
| `--flint` | `#6b7394` | `#6b7394` | Captions, table headers, nav links, chrome labels. |
| `--ash`   | `#9ba3bf` | `#4a5168` | Section eyebrows, secondary headings.              |
| `--chalk` | `#d0d5e3` | `#1a1d2a` | Body prose, code text — the primary reading color. |
| `--white` | `#eef0f6` | `#0a0c14` | Headings, wordmark, primary emphasis.              |

The ramp is named to read as graphite-pencil weights. On dark, steel is
the finest (darkest) mark and chalk is the body of the page; on light the
ramp is mirrored — `--white` becomes max-contrast graphite at the top,
`--steel` is the lightest hint tier just above the ridge. `--flint` is
the only token that holds its value across both grounds because mid-cool
gray (#6b7394) reads correctly on either; the rest invert. Bypassing the
ramp (hex values, ad-hoc grays) breaks the stack.

### Accent (the inspector's mark)

| Token          | Dark        | Light       | Purpose                                          |
| -------------- | ----------- | ----------- | ------------------------------------------------ |
| `--accent`     | `#818cf8`   | `#4f46e5`   | Links, wordmark dot, active nav, highlight rule. |
| `--accent-dim` | `#818cf815` | `#4f46e515` | Tinted surfaces (active record, body distinct).  |
| `--accent-mid` | `#818cf840` | `#4f46e540` | Code-block left rule, focus-state separators.    |

The dark accent is indigo-400; the light accent is indigo-600 — darker
to clear AA against the off-white ground. Both read as "the inspector's
stamp"; both are the only color that says "qualifier acted" or "the user
is here." Indigo is never used for ranking, never used for body text,
never used as a neutral fill.

### Signal palette (the verdict colors)

The signal palette encodes scoring and status. Scoring is core to
qualifier, and the palette is the readout. It is the only place in the
brand where multiple non-accent colors are permitted, and they are
allowed only on **status marks** — not on flooded surfaces.

| Token        | Dark        | Light       | Purpose                                       |
| ------------ | ----------- | ----------- | --------------------------------------------- |
| `--pass`     | `#34d399`   | `#059669`   | Pass / praise / healthy. Emerald.             |
| `--pass-dim` | `#34d39920` | `#05966920` | Pass swatch backgrounds, score-bar fills.     |
| `--warn`     | `#fbbf24`   | `#d97706`   | Warn / suggestion / unqualified. Amber.       |
| `--warn-dim` | `#fbbf2420` | `#d9770620` | Warn swatch backgrounds.                      |
| `--fail`     | `#f87171`   | `#dc2626`   | Fail / blocker / concern (severe). Coral.     |
| `--fail-dim` | `#f8717120` | `#dc262620` | Fail swatch backgrounds, blocker row tinting. |
| `--info`     | `#60a5fa`   | `#2563eb`   | Informational. Sky.                           |
| `--info-dim` | `#60a5fa20` | `#2563eb20` | Info swatch backgrounds.                      |

Light-theme signal values shift from the bright dark-bg variants to the
600-tier of each hue family — emerald-600, amber-600, red-600, blue-600
— so the verdict marks meet AA contrast on light grounds without losing
their identity.

Signal colors **never** flood a card or section. A red full-bleed
background is wrong. A `--fail` left-border on a single subject row is
right. Color alone never encodes hierarchy — the status word always
appears next to the swatch.

### Grid lines

| Token               | Dark        | Light       | Purpose                                |
| ------------------- | ----------- | ----------- | -------------------------------------- |
| `--grid-line`       | `#ffffff06` | `#00000008` | Minor lines of the body grid.          |
| `--grid-line-major` | `#ffffff0a` | `#0000000d` | Reserved for major grid intersections. |

The body grid is qualifier's quiet brand mark. It must be visible at
the periphery and silent at the center. On light the grid switches from
white-on-dark to black-on-light at a comparably faint alpha — same
"barely there" reading, opposite polarity.

### Spacing

| Token         | Value           | Purpose                            |
| ------------- | --------------- | ---------------------------------- |
| `--space-xs`  | `0.25rem` (4px) | Inline gaps, icon-to-label spacing |
| `--space-sm`  | `0.5rem` (8px)  | Between related elements           |
| `--space-md`  | `1rem` (16px)   | Component internal padding         |
| `--space-lg`  | `1.5rem` (24px) | Container padding, section gutters |
| `--space-xl`  | `2.5rem` (40px) | Between major sections             |
| `--space-2xl` | `4rem` (64px)   | Hero / page-level breathing room   |

### Layout primitives

| Token       | Value           | Purpose                            |
| ----------- | --------------- | ---------------------------------- |
| `--max-w`   | `68rem`         | Body content max-width             |
| `--nav-h`   | `~56px`         | Navigation height (1rem v-padding) |
| `--measure` | `40rem` (~62ch) | Prose column for long-form reading |

## Typography

Three registers, one role each. A contemporary technical sans owns the
display headings. An accessibility-tuned sans owns the body prose. A
monospace owns code and chrome. The split is principled: each face is
chosen for the kind of reading it does, and swapping any one breaks a
contract elsewhere.

**Why Instrument Sans for display.** Instrument Sans reads like the
cover sheet of an engineering specification — slightly compressed, even
weight, mechanical without feeling cold. Variable weight (400–700) gives
the headings room to range from quiet (sub-section) to assertive (page
title) without changing family.

**Why Atkinson Hyperlegible Next for body.** Atkinson was designed by
the Braille Institute for low-vision readers; it is the only sans on
the site shipped explicitly for legibility under sustained reading.
Qualifier publishes long-form pages (the spec, the format guide, the
Why chapter) and lives in dark mode under fluorescent office light —
the same context Atkinson was tuned for. The character set is also
distinctive: open apertures, unambiguous `0`/`O` and `I`/`l`/`1`. The
brand's accessibility floor isn't an afterthought; it's the body type.

**Why JetBrains Mono for code.** JetBrains Mono is variable-weight
(400–600), which is what the format page's envelope/body distinction
relies on (envelope at 400, body at 600 inside `.record-body`).
Ligatures stay off — JSON does not need them.

```
/* Display — headings, wordmark */
"Instrument Sans", "Inter", "Helvetica Neue", sans-serif;

/* Body — prose, list items, table cells */
"Atkinson Hyperlegible Next", "Atkinson Hyperlegible", system-ui, sans-serif;

/* Mono — code, labels, chrome, captions */
"JetBrains Mono", "SF Mono", "Consolas", monospace;
```

### Canonical hierarchy

One table. Every text element pinned to family / size / weight / tracking
/ color.

| Element                | Family   | Size                    | Weight  | Tracking       | Color         |
| ---------------------- | -------- | ----------------------- | ------- | -------------- | ------------- |
| Wordmark               | display  | `0.9rem`                | 700     | `-0.02em`      | `--white`     |
| Page heading (h1)      | display  | `1.5rem`                | 700     | `-0.01em`      | `--white`     |
| Hero heading           | display  | `clamp(1.8,5vw,2.6rem)` | 700     | `-0.02em`      | `--white`     |
| Section (h2)           | display  | `1.15rem`               | 600     | `-0.005em`     | `--white`     |
| Subsection (h3)        | display  | `0.82rem`               | 600     | `0.06em` upper | `--ash`       |
| Sub-subsection (h4)    | mono     | `0.8rem`                | 600     | `0.04em` upper | `--flint`     |
| **Body / list / `td`** | **body** | **`1rem` (16px)**       | **400** | **`0`**        | **`--chalk`** |
| Tagline / subtitle     | body     | `1.05–1.1rem`           | 400     | `0`            | `--ash`       |
| Code (inline + block)  | mono     | `0.84em` / `0.82rem`    | 400     | `0`            | `--chalk`     |
| Caption / label        | mono     | `0.72rem`               | 600     | `0.08em` upper | `--flint`     |
| Table header (`th`)    | mono     | `0.72rem`               | 600     | `0.08em` upper | `--flint`     |
| Nav link               | mono     | `0.75rem`               | 500     | `0.02em`       | `--flint`     |
| Footer                 | mono     | `0.72rem`               | 400     | `0.04em`       | `--steel`     |
| Active nav (current)   | mono     | `0.75rem`               | 500     | `0.02em`       | `--accent`    |

Body line-height is **1.6**; headings are **1.2–1.3**. Body is
left-aligned; never justified, never centered. Hyphens off. The `16px`
`html` font-size is the rem anchor for the body display size; chrome and
code are sized in `rem` so they scale predictably.

## The primary visual artifacts

Qualifier is rendered, not just described. Three concrete renders carry
brand weight, and each has its own discipline.

### The annotated artifact

The conceptual hub. A `.qual` file is a stack of typed observations
stamped against a subject. Visually this echoes the inspector's redline
on an engineering drawing — each annotation is a stamp; threads are
stamps connected by thin lines; a resolve is a check-mark over the
original. Every render below is a projection of this metaphor.

### The threaded show output

`qualifier show <subject>` renders annotations as a tree. Tree-drawing
characters in `--flint`. Each record line carries a score bracket, a
kind label, summary, issuer, date, and short ID:

```
[-30] concern  L42–58 "Panics on malformed input"    alice  2026-02-24  a1b2c3d4
├── [ 0] comment       "Good catch, fixed"           bob    2026-02-25  b2c3d4e5
└── [ 0] resolve       "Resolved"                    alice  2026-02-25  c3d4e5f6
[+40] praise          "Excellent property test coverage"  bob  2026-02-24  e5f6a7b8
```

The render is plain text in monospace; that is the brand. No iconography
for kinds (the words are the icons), no curved connectors, no color in
the body of the line — color is reserved for the score bracket and the
kind label, drawn from the signal palette.

### The score readout

`qualifier score` renders a table of subjects with raw and effective
scores plus a status word and a unit-bar:

```
SUBJECT               RAW    EFF   STATUS
lib/crypto            -20    -20   ██░░░░░░░░  blocker
lib/auth               60    -20   ██░░░░░░░░  blocker
lib/http               80     80   ████████░░  healthy
bin/server             45    -20   ██░░░░░░░░  blocker
```

Status words map to the signal palette per spec §4.4: `blocker`/`fail` →
`--fail`; `unqualified` → `--flint`; `ok`/`info` → `--info` or
`--chalk`; `healthy`/`pass` → `--pass`. The bar uses the matching
`*-dim` token as fill and `--ridge` as the empty track. The status word
**always** appears next to the bar; color alone never carries the
verdict.

### The activity strip (the topo motif)

The hero ships a small SVG line chart — annotation activity rising over
time, dotted with signal-colored events at threshold moments. It also
appears as a divider between major sections (`.topo-wide`). This is
qualifier's equivalent of toolpath's contour motif: a backdrop that
quietly signals "this is qualifier, not pathbase, not toolpath." The
strip uses `--accent`, `--pass`, `--warn`, and `--fail` at low opacity;
it is never the primary content of a section.

### What "on-brand" means for a qualifier render

A generic table or tree is not on-brand even if the colors are right.
On-brand renders are: monospace text bodies with sharp-corner panels,
status words paired with color swatches, tracked-uppercase labels above
data, signal colors restricted to verdict marks, indigo reserved for
"the active row," and grid-paper visible at the periphery.

## Visual elements

### Graph-paper background

The body of every page carries a 24px grid drawn from `--grid-line`. The
intent is "the inspector left this open on a graphing notebook." The
grid must be visible at the periphery and silent at the center — never
trace lines around content, never strengthen the grid for emphasis. A
future hook for `--grid-line-major` exists for major intersection
emphasis but is not currently used.

```css
body {
  background-image:
    linear-gradient(var(--grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--grid-line) 1px, transparent 1px);
  background-size: 24px 24px;
  background-position: -1px -1px;
}
```

### Topographic activity strip

A thin SVG used in the hero (`.topo-hero`, vertical) and as a divider
(`.topo-wide`, horizontal). Always inline SVG, never raster. Stroke is
`--accent` at ~50% opacity; signal-colored dots punctuate threshold
events at ~60% opacity. Used **once** in the hero and **at most once
per page** as a section divider.

### Accent dot wordmark

The wordmark in nav prefixes "Qualifier" with an 8px square indigo dot
(`--accent`, 1px border-radius). The dot is the inspector's stamp pad
— present, small, never loud. The wordmark itself is `--white` display
sans at 0.9rem / 700 / `-0.02em` tracking.

### No drop shadows. No decorative gradients. No drop caps.

Depth comes from surface layering (`--void` → `--slab` → `--plate`) and
border opacity (`--ridge` for default, `--accent-mid` for emphasis).
The inspector's office is well-lit and flat; depth lives in the
records, not in the lighting.

## Components

### Navigation

Full-width, ~56px tall (1rem vertical padding around mono content).
`--ridge` 1px bottom border, no background. Wordmark left, links right.
Links are `--flint` mono 0.75rem; hover shifts to `--white`; the active
page (`aria-current`) is `--accent`. The GitHub link sits at half
opacity until hovered.

### Hero

Two-column layout on desktop (content + topo SVG), single column on
mobile. Headline at the hero size from the type table. Tagline in body
sans, `--ash`. Optional inline command (`cargo install qualifier`) in
mono on a `--slab` plate. CTA is a bordered indigo button (see below).

### Buttons (CTA)

```
border: 1px solid var(--accent);
color: var(--accent);
background: transparent;
padding: 0.6rem 1.25rem;
font-family: var(--mono);
font-size: 0.78rem;
letter-spacing: 0.06em;
text-transform: uppercase;
```

Hover fills with `--accent`, text inverts to `--void`. Sharp corners
or 2px radius maximum. No drop shadow. No icon-with-button unless the
icon is monospace text (`→`, `▾`).

### Links (inline)

`--accent` color, no underline at rest. Hover adds a 1px underline at
2px offset and shifts color to `--white`. Heading anchors hover with a
2px `--accent` underline.

### Code blocks

```
background: var(--slab);
border: 1px solid var(--ridge);
border-left: 3px solid var(--accent-mid);
padding: 1rem 1.25rem;
font-size: 0.82rem;
line-height: 1.55;
overflow-x: auto;
border-radius: 2px;
```

The 3px `--accent-mid` left rule is the brand mark on code. Inline
`<code>` uses `--plate` background, `--ridge` border, `0.84em`. No
copy buttons hovering inside. Syntax highlighting stays inside the
warm-cool indigo palette — see the Prism token map in the
implementation appendix.

### Side-by-side / tabbed code (`code-compare`)

Two code panes labelled with mono-tracked captions. On viewports

> 640px, both panes display side-by-side with their own headers. At or
> below 640px, the captions become tappable tabs and only the active
> pane is shown; the active tab gains a `--accent-mid` bottom border.
> This is the standard pattern for showing two related records (a
> comment and its reply, an envelope before and after compaction, etc.).
> Implemented purely with CSS via the radio-button pattern; no JS.

### Cards (concept cards)

Three-column grid on desktop, single column on mobile. `--slab`
background, `--ridge` 1px border, `--space-md` padding. No drop shadow,
no radius beyond 2px. Hover state lifts to `--plate` background. The
card edge is a cut, not a lift.

### Tables

No outer border. Header row is mono uppercase tracked `--flint` — no
fill, no underline. Row separators are 1px `--ridge`. Cells get
`0.5rem 0.75rem` padding. Numeric columns right-align; status columns
center-align with the swatch left of the word.

### Prose container

Single column constrained by `--measure` (~62ch) for long-form pages.
Body left-aligned. Headings break the prose with `--space-xl` above and
`--space-md` below. Lists use the standard mono bullet (`•` or `-`); no
custom markers. Inline `<code>` carries the plate background even
inside prose.

### Footer

Small, mono, `--steel`, centered. A single line with copyright and the
GitHub / docs.rs / crates.io links separated by middots. 1px `--ridge`
top border. The family-standard footer pattern shared with toolpath and
pathbase — terse, monospaced, no decoration.

## Tone of voice

### Stance

Direct, precise, observational. The reader is intelligent and curious;
you don't sell them, you orient them. Concrete before abstract — show
the record, then explain the field. Slight wryness OK in long-form
("where the bodies are buried", "the format being boring on purpose").
Never enthusiastic; never apologetic.

### Material vocabulary

Reach for: **stamp, mark, redline, flag, log, record, witness, attest,
qualify, sign-off, pin, anchor, thread, supersede, resolve,
propagate.** Avoid: _journey, ecosystem, holistic, leverage, paradigm,
seamless, robust, powerful, blazing._ The vocabulary of inspection —
observing, marking, certifying — reinforces that qualifier records the
act of judging.

### Body copy

| Good                                                                                       | Bad                                                                   |
| ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| An annotation records one observation about one subject by one issuer.                     | Annotations are our flexible way of capturing feedback in the system. |
| Resolution withdraws a concern's score from the raw total via supersession.                | We provide flexible options for closing out review items.             |
| When Claude flags an issue and a human replies with a fix, both records share an envelope. | Modern AI workflows can integrate seamlessly with human review.       |
| Push straight to main. Append-only files merge cleanly because there's nothing to edit.    | Our innovative architecture eliminates merge conflicts.               |

### CLI voice

Qualifier is primarily a CLI. Three surfaces need brand discipline:

**Error messages.** Name the artifact, say what failed, suggest the
fix. One sentence each.

```
Good:  error: no .qual file for src/parser.rs (run `qualifier init` to bootstrap)
Bad:   Error: An issue was encountered while attempting to locate the qualifier file.
```

**`--help` text.** Imperative one-liners. Arguments before flags.
Always include one example invocation. No marketing.

```
Good:  Record an annotation at a specific location.
       Usage: qualifier record <kind> <location> [message]
       Example: qualifier record concern src/auth.rs:42 "SQL injection risk"

Bad:   The record command provides a powerful way to record concerns
       in your qualifier-enabled workflow.
```

**README rhythm.** One-sentence thesis, install, smallest useful
example, then escape hatches. The shape of a UNIX manual page, not a
landing page.

### Headings and labels

Imperative or noun-phrase, never sentence. "Anatomy of a record," not
"How to understand the record format." Tracked-uppercase mono labels
are the voice of the masthead — keep them short (1–3 words).

## Wordmark

`Qualifier` set in display sans (Instrument Sans), weight 700, tracked
at `-0.02em`, colored `--white`. Always title-case ("Qualifier"), never
all-caps in marketing prose. In nav and chrome, the wordmark is
preceded by an 8px square `--accent` dot with 1px border-radius — the
inspector's stamp pad.

```
■ Qualifier                                  Continuous Annotation, stored as files.
```

The dot is the only color in the wordmark lockup; the word itself is
always `--white` (or `--void` reversed when on a flooded `--accent`
field). There is no monogram; the dot+word is the mark.

## What we're not

- **We are not Toolpath.** Toolpath is workshop+manual,
  copper-on-parchment, machinist vocabulary. Don't import copper
  accents, parchment grounds, or vocabulary like _carve, mill, plunge_.
- **We are not Pathbase.** Pathbase is night-garden,
  emerald-and-gold, organic. Don't import emerald-as-accent or
  vocabulary like _garden, bloom, root_. (Qualifier uses emerald for
  the `--pass` signal, but that's a verdict, not the accent.)
- **We are not the parent empathic palette.** The parent has its own
  warm cast at slightly cooler hues. Qualifier's accent is colder
  (`#818cf8`); its grounds are darker and bluer. Family resemblance,
  not identity.
- **We are not enterprise.** No corporate blue. No glossy surfaces.
  No SaaS aesthetic. The brand is craft documentation, not B2B.
- **We are not garish.** The signal palette is bright by necessity but
  used in small marks, not flooded surfaces. A `--fail` red full-card
  background is wrong; a `--fail` left-border on a single subject row
  is right.
- **We are not three sans families.** Instrument Sans is for display,
  Atkinson is for body. Reaching for a third sans in chrome (or
  putting Atkinson in headings) is a regression.

## Don'ts

Each rule comes with the reason it exists. A rule without a reason
breaks at the first edge case.

- **Don't introduce warm-cast surfaces.** Indigo lives on cool grounds.
  Warm grounds shift the brand toward toolpath and lose the
  blueprint reading.
- **Don't round corners beyond 2px.** Sharp edges read as drafting;
  rounded edges read as consumer software. The exception is the
  wordmark dot at 1px (just enough to soften the corner artifact at
  small sizes).
- **Don't add drop shadows or decorative gradients.** Depth comes from
  surface layering. Shadows imply elevation; qualifier shows strata,
  not altitude. The grid-paper background is the only persistent
  texture; the topographic strip is the only persistent figure.
- **Don't put display sans in body prose, and don't put Atkinson in
  chrome.** The split is load-bearing. Atkinson is the body type
  precisely because the body is where qualifier publishes long-form;
  Instrument Sans is the heading type because headings need
  proportion, not legibility-first.
- **Don't put mono in prose.** A page of 16px monospace is fatiguing
  and reads as a code listing. The body is sans by contract.
- **Don't hardcode hex in components.** Every color is a token. A
  light theme can only ship if nothing leaks below the token layer.
- **Don't use color alone to encode hierarchy.** Reach for size,
  weight, and tracking first. The signal palette pairs with text
  status — color is the last differentiator, not the first.
- **Don't decorate the threaded render or the score readout.** Those
  renders are information. No emoji status, no animated edges, no
  curved connectors, no node icons. Their plainness is the brand.
- **Don't replace the topographic activity strip with a generic
  divider or `<hr>`.** The strip is one of the few marks that
  distinguish qualifier from any other dark-mode mono site.
- **Don't write marketing copy in `--help` text.** A CLI is read by
  someone who is already inside the door; selling them is noise.

## Implementation appendix

### Cascade contract

CSS is organized into a layered cascade. The order below is the
authoritative spec; later layers beat earlier layers regardless of
selector specificity:

```css
@layer reset, base, layout, components;
```

- `reset` — box-sizing, margin/padding zeroing, list/anchor/button
  resets. Nothing brand-specific.
- `base` — `:root` token definitions (dark — the canonical theme),
  the `[data-theme="light"]` token override block, `html` /
  `body` defaults, `::selection`, the `html.fonts-loading body`
  visibility hook, font-face declarations, base typography
  (`h1`–`h4`, `p`, `a`, `ul`, `ol`, `li`, `code`, `pre`, `table`),
  Prism token colors.
- `layout` — graph-paper grid on `body`, the `main` max-width
  container, the `.prose` long-form container. No component styling.
- `components` — `.site-nav`, `.site-nav-inner`, `.nav-logo`,
  `.nav-right`, `.nav-links`, `.theme-toggle`, `.nav-menu-btn`,
  `.site-footer`, `.hero`, `.hero-install`, `.try-it-btn`,
  `.code-compare` and friends, `.concepts`, `.concept-card`,
  `.divider`, `.subtitle`, `.topo-*`, `.kind-badge`, `.score-bar`,
  `.propagation-figure`, `.spec-content`. Everything in the
  Components section above lives here.

A component must never set a token-level variable; it consumes them. A
layout primitive must never touch component visuals. Anything declared
outside a layer beats anything inside, so leave nothing unwrapped.

### Theme switching

Dark is canonical. Light ships alongside as a cool pencil-on-paper
inversion. Resolution is tri-state — `system` (follows
`prefers-color-scheme`), `dark`, `light` — and the user's choice is
persisted under the `qualifier-theme` localStorage key.

Implementation:

- A synchronous inline script in `<head>` reads the stored preference,
  resolves `system` against `prefers-color-scheme`, and writes both
  `data-theme-pref` and `data-theme` to `<html>` before first paint.
  This eliminates a wrong-theme flash on hard refresh.
- The same script gates body visibility on font load (`html.fonts-loading`
  → `html.fonts-loaded`) with a 1500ms ceiling, collapsing multi-stage
  FOUT into a single render commit.
- A trailing script binds the `.theme-toggle` button to cycle
  `system → dark → light` and live-follows
  `prefers-color-scheme` changes while in `system` mode. Theme changes
  fire a `qualifier:theme-change` `CustomEvent` for any listener that
  needs to react (the playground in particular).

Body-text token pairs meet WCAG AA 4.5:1 on both grounds. Hint-tier
tokens (`--steel`) sit at the family-standard low-but-legible register
on both grounds (~3:1 for code comments and footer marks), matching
toolpath's pattern.

### Font loading

Self-hosted under `/fonts/`, no third-party CDNs at runtime. Variable
fonts where supported. `font-display: swap`. Subsets latin + latin-ext.

| Asset                                  | Source (build-time)               | Output                                 |
| -------------------------------------- | --------------------------------- | -------------------------------------- |
| Instrument Sans 400–700 (variable)     | self-hosted woff2                 | `/fonts/instrument-sans*.woff2`        |
| Atkinson Hyperlegible Next 400 / 600   | self-hosted woff2                 | `/fonts/atkinson-*.woff2`              |
| Atkinson Hyperlegible Next 400 italic  | self-hosted woff2                 | `/fonts/atkinson-italic*.woff2`        |
| JetBrains Mono 400–600 (variable)      | self-hosted woff2                 | `/fonts/jetbrains-mono*.woff2`         |
| Prism core + json + bash + rust + toml | npm `prismjs` (build-time bundle) | inline rendered into `_site/`          |
| Wordmark / favicon                     | TBD                               | `/favicon.svg`, `/favicon-{16,32}.png` |
| OG image template                      | TBD                               | `/assets/og.png` (1200×630)            |

**Preload contract.** The four most-used font weights (Atkinson 400,
Instrument Sans 400/700, JetBrains Mono 400) are emitted as `<link
rel="preload" as="font" crossorigin>` in `<head>` so the browser
fetches them in parallel with the stylesheet — no late swap on hard
refresh.

### Prism token map

Syntax highlighting stays inside an unsaturated cool palette that
reads as "marker on graph paper" — no token color competes with
indigo.

Token colors live behind `--syn-*` variables so they can theme. Dark
uses bright pastel families that read as marker-on-graph-paper; light
shifts to the 700-tier of each family to keep AA on the lifted grounds.

| Prism token              | Token            | Dark      | Light     |
| ------------------------ | ---------------- | --------- | --------- |
| `comment`                | `--steel` italic | `#3a4158` | `#9098ad` |
| `keyword`, `selector`    | `--syn-keyword`  | `#f0abfc` | `#a21caf` |
| `string`, `url`          | `--syn-string`   | `#6ee7b7` | `#047857` |
| `number`                 | `--syn-number`   | `#fcd34d` | `#b45309` |
| `function`, `class-name` | `--syn-function` | `#93c5fd` | `#1d4ed8` |
| `property`               | `--syn-property` | `#c4b5fd` | `#6d28d9` |
| `operator`               | `--ash`          | (text)    | (text)    |
| `punctuation`            | `--flint`        | (text)    | (text)    |
| `namespace`              | `opacity: 0.7`   | (mute)    | (mute)    |

These are the only non-token colors permitted on the site, and they
appear only inside `<code>` blocks.

### Accessibility floor

- All token pairs meet **WCAG AA** for body text (4.5:1) on the
  intended ground. Verify before changing any token value.
- Atkinson Hyperlegible Next is the body type precisely because it
  raises the floor; never pair it with sub-AA contrast.
- Focus states use `--accent` as a 2px outline at `2px` offset. Never
  rely on color alone — the outline change is structural.
- `prefers-reduced-motion: reduce` disables the topographic strip's
  animation (if any) and the smooth-scroll on anchor jumps.
- Status words always accompany signal-colored marks; color alone
  never encodes verdict.

## Summary

| Attribute    | Choice                                                                            |
| ------------ | --------------------------------------------------------------------------------- |
| Palette      | Indigo accent, cool charcoal/slate grounds, four-color signal palette             |
| Themes       | Dark + light, both cool-cast, token-driven                                        |
| Typography   | Instrument Sans display, Atkinson Hyperlegible Next body, JetBrains Mono code     |
| Hierarchy    | Tracked uppercase mono chrome (10–13px); 16px / 1.6 Atkinson body                 |
| Container    | 68rem max-width, ~56px nav, 1.5rem gutter, sharp corners                          |
| Renders      | Threaded show output, score readout, topographic activity strip                   |
| Distinctives | Graph-paper grid, indigo accent dot, signal palette for verdicts                  |
| Voice        | Direct, precise, observational — the inspector's logbook                          |
| Cascade      | `@layer reset, base, layout, components` (current)                                |
| Don'ts       | Warm casts, rounded corners, shadows, gradients, hex in components, mono in prose |
