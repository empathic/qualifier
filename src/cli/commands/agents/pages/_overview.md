+++
name = "_overview"
+++

# qualifier — guide for AI coding agents

You are an AI coding agent in a user's repository. The `qualifier` CLI is
installed. This page tells you what qualifier is, when to use it, and how to
get more detail on any specific feature.

## What it is

Qualifier records *annotations* — structured, content-addressed notes about
software artifacts (files, directories, line ranges). Annotations live in
`.qual` files alongside the code, are intended to be committed to version
control, and form a durable, reviewable record of quality observations
attached to specific places in the codebase.

## When to use it

- You found a bug, smell, risk, or stylistic concern that survives this
  edit and is worth surfacing for whoever touches the code next. Record it.
- An earlier annotation no longer applies (the code changed, the concern
  was addressed). Resolve it.

## When NOT to use it

- One-off scratch debugging notes. Use scratch space.
- Information that belongs in the commit message (what *this* change does
  and why). Use git.
- Project-wide policy or architecture decisions. Those belong in
  `docs/` or an ADR, not as an annotation on a file.

## Quickstart

```bash
# Record a concern about a function in src/foo.rs
qualifier record concern src/foo.rs --message "tight coupling to the cache"

# See what's annotated on a file
qualifier show src/foo.rs

# After fixing it, resolve the annotation by id-prefix or location
qualifier resolve src/foo.rs --message "refactored to inject the cache"
```

## Available topics

Run `qualifier agents <topic>` for any of:

{{TOPICS}}

## Reference

For exact flag tables on any subcommand, run `qualifier <subcommand> --help`.
For the JSONL wire format and library API, see `SPEC.md` in this repo (if
present) or the published spec.
