# qual — qualifier skills for Claude Code

Skills that make Claude use [qualifier](https://github.com/empathic/qualifier)
at every phase of development: record design decisions and paths not taken,
write review findings into `.qual` files, triage and escalate threads,
consult threads before editing, and hand off threads a fresh session can act on.

## Install

```
/plugin marketplace add empathic/qualifier
/plugin install qual@qualifier
```

The plugin keeps its own copy of the qualifier release it pins, under
`~/.local/share/qualifier/plugin/<version>/` (`$XDG_DATA_HOME/qualifier/plugin/`
when `XDG_DATA_HOME` is set). It downloads that release on first use,
verified against checksums shipped in the plugin, and installs the new
release automatically when a plugin update pins a newer one, removing
older versions' directories (never a newer one). It never runs, replaces, or installs over a
`qualifier` on your `PATH`, so the one you use in your shell can be any
version. Platforms without a prebuilt binary get a
`cargo install qualifier --version …` line.

- `QUALIFIER_BIN=/abs/path/to/qualifier` makes the plugin run that binary
  instead (for a development build).
- `QUALIFIER_PLUGIN_HOME=/some/dir` relocates the plugin's installs (an
  absolute path other than `/`; anything else falls back to the default
  with a warning).

## What it does

In a repository that contains `.qual` files, a SessionStart hook loads the
`using-qualifier` skill and a two-line summary of open threads. The other
skills trigger at their moment in the lifecycle, or on demand as
`/qual:<skill>`. Elsewhere the plugin is silent.

Records written from Claude Code are marked `issuer_type: ai` and tagged
`session:claude-code:<session id>` by the qualifier CLI itself.

## Development

```bash
claude --plugin-dir ./plugins/claude-code         # run with the local plugin
QUALIFIER_BIN=$PWD/target/debug/qualifier claude --plugin-dir ./plugins/claude-code
scripts/test-plugin.sh                            # offline checks
```

## Evals

`plugins/claude-code/evals/` is a trigger-eval suite for `claude plugin eval`:
one case per skill, seeded by `evals/_fixture/scaffold.sh` (a git repo with a
spec, a source file, and open qualifier threads including a blocker on
`src/net.rs`), plus three quiet cases that must *not* write a record and a
`no-qual-files` case that must not fire any `qual:` skill at all. Running
evals calls the model and costs money — confirm before running.

Each run's agent session is isolated and inherits only an allowlist of
environment variables (`PATH`, locale, provider credentials, `EVAL_*`), so
`QUALIFIER_BIN` does not reach it. The plugin installs its pinned
qualifier release itself inside the session; no `PATH` setup is needed,
but the eval sandbox must allow that download (from GitHub releases), or
runs will report qualifier as not installed. The `scaffold_script` runs
on the host, outside that sandbox, and needs a qualifier there to seed the
fixture threads: it uses `QUALIFIER_BIN` if set, else `qualifier` on
`PATH`. `Bash`, `Write`, and `Edit` are gated tools: listing them in a
case's `allowed_tools` is not enough, they also need an operator grant on
the command line. Smoke-test one case first, then run the full suite:

```bash
cargo build --bin qualifier
QUALIFIER_BIN="$PWD/target/debug/qualifier" claude plugin eval plugins/claude-code \
  --case design-rejects-option --runs 1 --scaffold --allow-tools Write Edit Bash

QUALIFIER_BIN="$PWD/target/debug/qualifier" claude plugin eval plugins/claude-code \
  --runs 3 --scaffold --allow-tools Write Edit Bash
```

Each case also runs against a no-plugin baseline by default (`--ablation
with-without`, automatic once the plugin resolves), so the summary reports
`WITH`, `W/OUT`, and `Δ`.

To also run with `superpowers` loaded alongside `qual`, add its plugin
directory to the `plugins:` list in each case (`prompt.md` frontmatter, or
`case.yaml`; it defaults to just the enclosing `qual` plugin) — for example
`plugins: ["../..", "/path/to/superpowers"]` — then run the same command
again. `claude plugin eval` has no flag for adding a second plugin to a run;
`plugins:` is the documented way to list more than one. This two-plugin
form of `plugins:` has not been run yet — smoke-test it on one case first
(`--case <name> --runs 1`) and confirm both skills are actually available
before trusting a full-suite comparison.

Expected: each positive case passes `skill-fired` in at least 2 of 3 runs.
`review-spec` accepts either `recording-design-decisions` or
`reviewing-into-qualifier` firing — a "review this spec" prompt can
legitimately trigger either skill, and the case exists to watch that
overlap rather than force one winner. Every quiet case and `no-qual-files`
should pass in 3 of 3, and the no-plugin baseline should pass the positive
cases far less often than the with-plugin arm.
