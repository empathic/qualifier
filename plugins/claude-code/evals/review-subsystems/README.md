# review-subsystems

Exercises the large-scope path of `reviewing-into-qualifier`: the shared
fixture (`plugins/claude-code/evals/_fixture/scaffold.sh`) now has three
subsystems under `src/` (`net.rs`, `auth.rs`, `cache.rs`), so the prompt
asks for a full review of all of them — large enough that the skill's
"Review" step should split by subsystem and dispatch one reviewer subagent
per part, then a fresh verifier subagent per batch of findings, per the
skill's "Review" and "Verify" sections.

## Grader visibility into subagent tool calls

`skill-fired` and `no-bare-qualifier` are `tool_used` graders over the
run's tool calls. `code.claude.com/docs/en/plugin-evals.md`
doesn't say directly whether a dispatched subagent's own tool calls count;
it defines `trace` as "the session as JSON, one message per line."

`code.claude.com/docs/en/headless.md` ("Follow subagent messages") does say
this, for `claude -p`'s `stream-json` output: by default — no
`--forward-subagent-text` needed — a subagent's `tool_use` and
`tool_result` blocks appear in the stream, tagged with the spawning tool
call's ID in `parent_tool_use_id`; only the subagent's text/thinking blocks
are gated behind that flag. So these graders should see a reviewer or
verifier subagent's `Bash` calls the same as the controller session's own.

Not yet confirmed against `claude plugin eval`; running the suite costs
money. Treat it as a working assumption to verify on the first run: if
`no-bare-qualifier` never sees a reviewer's or verifier's calls, the
eval's tool calls do *not* include subagent calls, and it only checks the
controller session. `wrote-record` and `verified-tag` don't depend on
this: they read the file the records land in.

## Graders

Bash graders match the JSON-encoded tool input, which also holds the
call's free-text `description`; each is anchored inside the `command`
string and names real `qualifier` subcommands, so a description such as
"List qualifier threads…" can't match. `scripts/test-plugin.sh` checks
each against sample calls and `qualifier --help`'s subcommand list.

- `skill-fired.md` — `reviewing-into-qualifier` fired, prefixed or bare
  (same form as `review-branch`'s).
- `wrote-record.md` — `src/.qual`, after the run, holds a record tagged
  `session:claude-code:<id>`: a record this run wrote, by the controller
  or a subagent, however it invoked qualifier. The fixture's own records
  carry no session tag.
- `no-bare-qualifier.md` — no Bash command runs a bare `qualifier <sub>`;
  only the wrapper's `<path>/ensure-qualifier.sh exec <args>` form, since the plugin keeps
  its binary off `PATH` (see `scripts/ensure-qualifier.sh` and
  `hooks/session-start`). The regex is coarse: a `grep`/`echo` command
  whose own text contains "qualifier record" would also match.
- `verified-tag.md` — `src/.qual`, after the run, holds a reply tagged
  `verified:confirmed`, `verified:refuted`, or `verified:downgraded`. The
  fixture's findings are all on `src/*.rs`, so their threads, and the
  verifier's replies, live in the directory-level `src/.qual`. It grades
  that file (`target: { source: file, path: src/.qual }`) rather than the
  trace, which also holds the loaded skill and the filled verifier brief —
  both quote the verdict tags, so a trace regex would pass with no reply
  written.
