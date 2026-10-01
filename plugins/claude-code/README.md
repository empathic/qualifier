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
(or `<!-- example: expect-fail -->` for an intended failure). That binary
must report the version the plugin pins (`PINNED_VERSION`), since that is
the release users run; between a crate version bump and the plugin
release that pins it, set `ALLOW_UNPINNED_SKILLS=1` to check the skills
anyway (the mismatch is then a warning).

Any change under `plugins/claude-code/` outside `evals/` and `.qual`
files needs a version bump in `.claude-plugin/plugin.json` (and the
marketplace entry): an installed plugin stays on its version until that
changes. On pull requests CI runs
`scripts/test-plugin.sh --check-version-bump origin/<base branch>`, which
fails when such a change leaves the version as it was at the merge base.

## Evals

`plugins/claude-code/evals/` is a trigger-eval suite for `claude plugin eval`:
one case per skill, seeded by a fixture script (a git repo with a spec,
three source files under `src/`, and open qualifier threads including a
blocker on `src/net.rs`); `review-subsystems`, a large-scope review that
should dispatch reviewer and verifier subagents (see
[its README](evals/review-subsystems/README.md)); and four negative
cases: three quiet cases (`quiet-typo`, `quiet-question`, `quiet-explore`)
that must *not* write a record, and `no-qual-files`, which must not fire
any `qual:` skill at all. Running evals calls the model and costs money —
confirm before running.

A case's `scaffold_script` must name a file inside its own case directory,
so each case that uses the shared fixture has its own `scaffold.sh`, a
copy of `evals/_fixture/scaffold.sh`. Edit `_fixture/scaffold.sh`, then
copy it over each case's `scaffold.sh` (every case except `no-qual-files`,
which has its own).

Graders check outcomes, not how a command was spelled. A record written
from an eval session carries `"issuer_type":"ai"` and a
`session:claude-code:<id>` tag (Claude Code sets `CLAUDECODE` in its Bash
environment), while the fixture's own records carry neither. So each
positive case's `wrote-record` grader is a regex over the `.qual` file its
record lands in: `"kind":"alternative"` in `docs/.qual`
(`design-rejects-option`), a `status:needs-decision` tag in `docs/.qual`
(`needs-decision`), and any session record in `src/.qual` or `docs/.qual`
for the review cases. Cases whose skill only reads have a scored check
on what it did: `edit-annotated-file` runs `threads` on `src/net.rs`
before its first Edit of that file (`tool_order`), `handoff` runs
`threads`, `done-change` runs `review` (closing-the-loop's drift check),
and the final reply of `plan-from-threads` and `triage-open` names at
least two different thread IDs.

`git` is unusable inside the eval sandbox on macOS: `/usr/bin/git` is a
stub that needs Xcode's command-line tools, which the sandbox does not
let it reach. So no case asks Claude to commit, and no grader depends on
git inside the session. `done-change` ends the change uncommitted and
grades the closing-the-loop checklist that runs before "done". The
scaffold script runs outside the sandbox, so the fixture's own commit is
unaffected. `tool_used: Skill` graders are
unscored indicators in a two-arm run, so every case except
`no-qual-files` has at least one of these scored outcome graders.

The quiet cases forbid writes twice. `no-session-records` requires that
the `.qual` file the case could touch holds no session record after the
run, whatever route wrote it. `no-record` counts any `record`, `reply`,
`resolve`, or `emit` call however qualifier is reached (bare, by path, or
the wrapper's `exec`, including through a variable such as `"$Q" exec
record`), `record --stdin --dry-run` included: that is deliberately
conservative, so a dry run fails a negative case. Bash graders match only
inside the call's `command`, not its description.
`scripts/test-plugin.sh` proves the file graders against the scaffolded
fixture and records written by the real binary, and the others against
sample calls and replies.

Each run's agent session is isolated and inherits only an allowlist of
environment variables (`PATH`, locale, provider credentials, `EVAL_*`), so
`QUALIFIER_BIN` does not reach it. The plugin installs its pinned
qualifier release itself inside the session; no `PATH` setup is needed,
but the eval sandbox must allow that download (from GitHub releases), or
runs will report that the plugin could not install qualifier. The
`scaffold_script` runs on the host, outside that sandbox, and needs a
qualifier there to seed the fixture threads. It receives only `PATH`,
`TMPDIR`, a temporary `HOME`, and a few constants, so `QUALIFIER_BIN`
does not reach it either: put the built binary first on `PATH`. `Bash`,
`Write`, and `Edit` are gated tools: listing them in a case's
`allowed_tools` is not enough, they also need an operator grant on the
command line. `scripts/eval-plugin.sh` does all of this: it builds
qualifier, puts it first on `PATH`, passes `--scaffold` and the tool
grants, runs from any directory, and sets a `--max-cost-usd 40` ceiling
unless you give one. Any other option goes through to
`claude plugin eval`. Smoke-test one case first, then run the full suite:

```bash
scripts/eval-plugin.sh --case design-rejects-option --runs 1
scripts/eval-plugin.sh --runs 3
```

Each case also runs against a no-plugin baseline by default (`--ablation
with-without`, automatic once the plugin resolves), so the summary reports
`WITH`, `W/OUT`, and `Δ`.

To also load `superpowers` (or any other plugin) alongside `qual`, pass
its directory with `--with`:

```bash
scripts/eval-plugin.sh --with ~/.claude/plugins/cache/claude-plugins-official/superpowers/<version> --runs 3
```

A case's `plugins:` list can load more than one plugin, but only from
inside the plugin under test, so `--with` runs the suite from a temporary
copy of `plugins/claude-code` with the extra plugin copied under
`.eval-plugins/` and added to every case. Reports still land in
`evals/results/`.

Expected: each of the four negative cases passes in 3 of 3 runs, and
every other case in at least 2 of 3. `review-spec` accepts either
`recording-design-decisions` or `reviewing-into-qualifier` firing — a
"review this spec" prompt can legitimately trigger either skill, and the
case exists to watch that overlap rather than force one winner. The
no-plugin baseline should pass the cases other than the negative ones far
less often than the with-plugin arm.

### Results

Pass rate per case, per configuration. "Plugin alone" is `qual` with no
other plugin loaded; "with superpowers" is the same suite run with
`--with` pointing at `superpowers`, as described above. The negative cases
(`no-qual-files`, `quiet-explore`, `quiet-question`, `quiet-typo`) need 3
of 3; every other case needs at least 2 of 3. Each cell is passing
with-plugin runs out of 3, with the no-plugin baseline's in parentheses.
The baseline has qualifier on `PATH` (the scaffold needs it there) and
learns its conventions from `qualifier agents`, so a case whose prompt
names qualifier threads (`handoff`, `plan-from-threads`, `triage-open`)
can pass without the plugin; there the plugin's value is in the other
cases, where nothing in the prompt mentions qualifier.

| Case | Plugin alone | With superpowers | Date | qualifier / plugin version |
| :- | :- | :- | :- | :- |
| design-rejects-option | 3/3 (0/3) | 3/3 (0/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| done-change | 3/3 (0/3) | 3/3 (0/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| edit-annotated-file | 3/3 (0/3) | 3/3 (0/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| handoff | 3/3 (3/3) | 3/3 (3/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| needs-decision | 3/3 (0/3) | 3/3 (1/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| no-qual-files | 3/3 (3/3) | 3/3 (3/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| plan-from-threads | 3/3 (3/3) | 3/3 (3/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| quiet-explore | 3/3 (3/3) | 3/3 (3/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| quiet-question | 3/3 (3/3) | 3/3 (3/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| quiet-typo | 3/3 (3/3) | 3/3 (3/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| review-branch | 3/3 (0/3) | 3/3 (0/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| review-spec | 3/3 (0/3) | 3/3 (0/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| review-subsystems | 3/3 (0/3) | 3/3 (0/3) | 2026-10-01 | 0.8.0 / 0.1.0 |
| triage-open | 3/3 (3/3) | 2/3 (3/3) | 2026-10-01 | 0.8.0 / 0.1.0 |

### Release checklist

Before a plugin release:

1. Build the qualifier release the plugin pins and run the full suite in
   both configurations (plugin alone, then again with `superpowers`
   loaded), at least 3 runs per case:
   ```
   scripts/eval-plugin.sh --runs 3
   scripts/eval-plugin.sh --with <superpowers plugin dir> --runs 3
   ```
2. Every negative case (`quiet-typo`, `quiet-question`, `quiet-explore`,
   and `no-qual-files`) must pass 3 of 3 runs; every other case must pass
   at least 2 of 3. `review-spec` passes on either of its two accepted
   skills firing.
3. Record the outcome in the [Results](#results) table above: pass rate
   for each configuration, the date, and the qualifier/plugin version pair
   under test. Do this for every release, even one where nothing under
   `evals/` changed — the table tracks drift against new models as much as
   against plugin changes.
