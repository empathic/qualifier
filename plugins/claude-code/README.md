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
verified against checksums shipped in the plugin, and records the binary's
sha256 beside it; a binary that no longer matches that record is
reinstalled before it is run. It installs the new release automatically
when a plugin update pins a newer one, removing older versions'
directories (never a newer one). It never runs, replaces, or installs over
a `qualifier` on your `PATH`, so the one you use in your shell can be any
version.

- `QUALIFIER_BIN=/abs/path/to/qualifier` makes the plugin run that binary
  instead (a development build, or a source install on a platform without
  a prebuilt release; see below).
- `QUALIFIER_PLUGIN_HOME=/some/dir` relocates the plugin's installs (an
  absolute path other than `/`; anything else falls back to the default
  with a warning).

Prebuilt releases cover macOS on Apple silicon and Linux on x86_64 and
aarch64. On any other platform, install the pinned version from source,
then point the plugin at it with `QUALIFIER_BIN` (a `qualifier` on `PATH`
is not used):

```bash
cargo install qualifier --version <pinned> --locked
```

and set `QUALIFIER_BIN` to the installed binary's absolute path (for
example `$HOME/.cargo/bin/qualifier`, written out in full) in your shell
profile, or under `env` in Claude Code's `settings.json`. `<pinned>` is
`PINNED_VERSION` in `scripts/ensure-qualifier.sh`; in a repository with
`.qual` files, the SessionStart hook gives the exact command.

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

`scripts/test-plugin.sh` also runs every qualifier command and record line
in the skills and subagent briefs against a real binary in a fixture
repository (`scripts/check-skill-examples.py`): `$QUALIFIER_BIN`, else
`target/debug/qualifier` or `target/release/qualifier`. Placeholders such as
`<root.id>` are filled from the table in that script; an example that can't
run is marked on the line before it with `<!-- example: skip — <reason> -->`
(or `<!-- example: expect-fail -->` for an intended failure).

## Evals

`plugins/claude-code/evals/` is a trigger-eval suite for `claude plugin eval`:
one case per skill, seeded by `evals/_fixture/scaffold.sh` (a git repo with a
spec, three source files under `src/`, and open qualifier threads including
a blocker on `src/net.rs`); `review-subsystems`, a large-scope review that
should dispatch reviewer and verifier subagents (see
[its README](evals/review-subsystems/README.md)); three quiet cases that
must *not* write a record; and a `no-qual-files` case that must not fire
any `qual:` skill at all. Running evals calls the model and costs money —
confirm before running.

Each run's agent session is isolated and inherits only an allowlist of
environment variables (`PATH`, locale, provider credentials, `EVAL_*`), so
`QUALIFIER_BIN` does not reach it. The plugin installs its pinned
qualifier release itself inside the session; no `PATH` setup is needed,
but the eval sandbox must allow that download (from GitHub releases), or
runs will report that the plugin could not install qualifier. The `scaffold_script` runs
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

Expected: each non-quiet case passes its graders in at least 2 of 3 runs.
`review-spec` accepts either `recording-design-decisions` or
`reviewing-into-qualifier` firing — a "review this spec" prompt can
legitimately trigger either skill, and the case exists to watch that
overlap rather than force one winner. Every quiet case and `no-qual-files`
should pass in 3 of 3, and the no-plugin baseline should pass the
non-quiet cases far less often than the with-plugin arm.

### Results

Pass rate per case, per configuration. "Plugin alone" is `qual` with no
other plugin loaded; "with superpowers" adds `superpowers`'s plugin
directory to `plugins:` as described above. Fill this in from an actual
`claude plugin eval` run — nothing below has been run yet.

| Case | Plugin alone | With superpowers | Date | qualifier / plugin version |
| :- | :- | :- | :- | :- |
| design-rejects-option | not yet run | not yet run | — | — |
| done-commit | not yet run | not yet run | — | — |
| edit-annotated-file | not yet run | not yet run | — | — |
| handoff | not yet run | not yet run | — | — |
| needs-decision | not yet run | not yet run | — | — |
| no-qual-files | not yet run | not yet run | — | — |
| plan-from-threads | not yet run | not yet run | — | — |
| quiet-explore | not yet run | not yet run | — | — |
| quiet-question | not yet run | not yet run | — | — |
| quiet-typo | not yet run | not yet run | — | — |
| review-branch | not yet run | not yet run | — | — |
| review-spec | not yet run | not yet run | — | — |
| review-subsystems | not yet run | not yet run | — | — |
| triage-open | not yet run | not yet run | — | — |

### Release checklist

Before a plugin release:

1. Build the qualifier release the plugin pins and run the full suite in
   both configurations (plugin alone, then again with `superpowers`
   loaded), at least 3 runs per case:
   ```
   QUALIFIER_BIN="$PWD/target/debug/qualifier" claude plugin eval plugins/claude-code \
     --runs 3 --scaffold --allow-tools Write Edit Bash
   ```
2. Every non-quiet case (each one but `quiet-typo`, `quiet-question`,
   `quiet-explore`, and `no-qual-files`) must pass at least 2 of 3 runs.
   Every quiet case (those four) must pass 3 of 3.
   `review-spec` passes on either of its two accepted skills firing.
3. Record the outcome in the [Results](#results) table above: pass rate
   for each configuration, the date, and the qualifier/plugin version pair
   under test. Do this for every release, even one where nothing under
   `evals/` changed — the table tracks drift against new models as much as
   against plugin changes.
