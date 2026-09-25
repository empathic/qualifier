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

The plugin uses an existing `qualifier` if one is on your `PATH`; otherwise
it installs the qualifier release this plugin pins to `~/.local/bin` on
first use, verified against checksums shipped in the plugin. Platforms
without a prebuilt binary get a `cargo install qualifier --version …` line.

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
