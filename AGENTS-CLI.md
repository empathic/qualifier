# AGENTS-CLI 0.1

A convention for command-line tools that self-describe to AI coding agents.

> **Status:** Draft. Breaking changes are allowed until 1.0. Feedback and adoption reports welcome via issues on any conforming tool's repository.

## Audience

CLI tool authors who want their tool to be productively usable by an AI coding agent on first contact, with no separately-installed prompt bundle, MCP server, or external skill.

## Motivation

A coding agent dropped into an unfamiliar shell typically does one of two things when it encounters a CLI: relies on prior training-data knowledge of the tool, or runs `<tool> --help` and pattern-matches. Neither path teaches the agent the *judgment* it needs — when to use the tool, when not to, what the common workflows look like, what mistakes to avoid. That judgment is what separates an agent that uses your tool well from one that strings invocations together by syntactic mimicry.

AGENTS-CLI defines the smallest surface that lets a tool ship its own onboarding for AI agents, version-locked to the binary, discovered through the existing `--help` channel that agents already inspect.

## Conformance

This document uses the requirement levels from RFC 2119: **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, **MAY**.

### Required (MUST)

A conforming tool **MUST** satisfy all of the following.

1. **Subcommand exists.** The tool exposes a subcommand named `agents`. Examples: `qualifier agents`, `gh agents`, `cargo agents`.

2. **Bare invocation prints an orientation page.** Running `<tool> agents` with no further arguments prints a self-contained orientation document to standard output and exits with status 0. The orientation document MUST be sufficient on its own to teach an agent (a) what the tool is, (b) when to use it, (c) when not to use it, and (d) how to drill into per-topic detail.

3. **Topic invocation prints the named page.** Running `<tool> agents <topic>` prints the page identified by `<topic>` to standard output and exits with status 0. The orientation page MUST list the available topics.

4. **Unknown topic produces a structured error.** Running `<tool> agents <unknown-topic>` MUST exit with status 2, and MUST write a message to standard error of the form:

   ```
   <tool> agents: no such topic '<unknown>'. Available: <name1>, <name2>, ...
   ```

   The available list MUST contain every topic name that would succeed under rule 3.

5. **`--help` discoverability.** The output of `<tool> --help` MUST surface the `agents` subcommand in a way that signals to a coding agent that it is the agent-targeted entry point. The conformance test is informal but operational: *an LLM scanning the help output reliably identifies `agents` as the entry point intended for AI agents.* In practice this means a labeled section header ("For AI agents:"), a banner line, or an equivalent affordance — not just an unmarked entry buried in a generic command list.

### Recommended (SHOULD)

A conforming tool **SHOULD** satisfy the following. These are common-sense quality bars; departing from them is allowed if there's a clear reason.

1. **Output is markdown.** Pages SHOULD be UTF-8 markdown text. Each page SHOULD begin with a single `# ` heading naming the page (e.g., `# <tool> record`).

2. **Page set covers concepts, workflows, pitfalls, and per-subcommand detail.** At minimum, the topic set SHOULD include:

   - A concept primer (often `concepts`) — the tool's data model, key invariants, and any vocabulary an agent must understand to use the tool well.
   - Common workflows (often `workflows`) — three to five worked recipes covering the most common tasks.
   - Common pitfalls (often `pitfalls`) — mistakes agents make and how to avoid them.
   - One page per non-trivial subcommand, named after the subcommand.

3. **Pages are hand-written.** Pages SHOULD be authored with judgment, not auto-generated from `--help` output. Agents already have access to `--help` for syntax; AGENTS-CLI exists to convey the *when* and *why* that flag tables cannot.

4. **Pages are version-locked to the binary.** Pages SHOULD ship inside the binary or alongside it such that the guidance an agent receives matches the tool it can actually invoke. Network fetches at runtime are discouraged.

### Out of scope for 0.1

The following are deferred to later versions of this protocol or to companion specifications. Tools MAY implement them, but doing so is not part of 0.1 conformance:

- A discovery handshake for sweeping `$PATH` (e.g., `<tool> agents --probe`).
- Structured (JSON) output for programmatic consumption.
- Internationalization or per-locale page sets.
- A standard format for storing pages in the source tree (each tool chooses).
- A registry of conforming tools.

## Reference implementation

**qualifier** (0.5.0 and later) is the reference implementation: <https://github.com/empathic/qualifier>.

`qualifier agents`, `qualifier agents concepts`, `qualifier agents <subcommand>`, and `qualifier agents <unknown>` exercise every MUST in this document. The "For AI agents:" group at the top of `qualifier --help` is the discoverability mechanism for rule 5. Pages live at `src/cli/commands/agents/pages/*.md`, embedded into the binary at compile time.

## Versioning

AGENTS-CLI uses semantic versioning. The current version is 0.1. While the major version is 0, breaking changes are allowed between minor versions; downstream tools should expect to update conformance as the protocol stabilizes. Tools MAY indicate which version they conform to in any way they choose at 0.1 (a footer in the orientation page is the conventional choice). A standardized declaration mechanism is deferred to 1.0.

## Acknowledgements

The protocol distills practice from man pages (orientation + per-topic drill-down), shell help conventions (machine-discoverable invocation surface), and the more recent `AGENTS.md` repo-level convention (the audience signal). It exists because no single one of those mechanisms gives a coding agent everything it needs on first contact, and because the friction of bolting on a separate skill bundle for every CLI is incompatible with how agents actually arrive at a codebase.
