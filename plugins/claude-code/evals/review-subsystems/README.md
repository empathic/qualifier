# review-subsystems

Exercises the large-scope path of `reviewing-into-qualifier`: the shared
fixture (`plugins/claude-code/evals/_fixture/scaffold.sh`) now has three
subsystems under `src/` (`net.rs`, `auth.rs`, `cache.rs`), so the prompt
asks for a full review of all of them — large enough that the skill's
"Review" step should split by subsystem and dispatch one reviewer subagent
per part, then a fresh verifier subagent per batch of findings, per the
skill's "Review" and "Verify" sections.

## Grader visibility into subagent tool calls

Every grader here reads the run's message stream (`tool_used`, or `regex`
over `target: trace`). `code.claude.com/docs/en/plugin-evals.md` doesn't
say directly whether a dispatched subagent's own tool calls land in that
stream — it only defines `trace` as "the session as JSON, one message per
line."

`code.claude.com/docs/en/headless.md` ("Follow subagent messages") does say
this, for `claude -p`'s `stream-json` output: by default — no
`--forward-subagent-text` needed — a subagent's `tool_use` and
`tool_result` blocks appear in the stream, tagged with the spawning tool
call's ID in `parent_tool_use_id`; only the subagent's text/thinking blocks
are gated behind that flag. Since `tool_used` and `trace`-regex graders
only look at `tool_use` blocks (and their results), they should see a
reviewer or verifier subagent's `Bash`/`Write` calls the same as the
controller session's own.

This isn't confirmed against `claude plugin eval` itself — this suite
wasn't run to write these cases (per the S-tier pass rules, running evals
costs money and wasn't authorized). Treat it as a working assumption to
verify the first time this case actually runs: if `wrote-record` or
`verified-tag` fail on an otherwise-correct run that fully delegated to
subagents, that means eval's `trace` does *not* include subagent tool
calls, and the fix is to grade the controller session only — e.g. require
its "After verification" `resolve` / `record --supersedes` step, or the
closing `threads --tag review:<tag>` report, rather than the subagents'
own writes.

## Graders

- `skill-fired.md` — `reviewing-into-qualifier` fired, prefixed or bare
  (same form as `review-branch`'s).
- `wrote-record.md` — a `record`/`reply`/`resolve` call ran, wrapper or
  bare form (same pattern as `review-branch`'s).
- `no-bare-qualifier.md` — no Bash call runs a bare `qualifier <sub>`; only
  the wrapper's `"<path>" exec <args>` form, since the plugin keeps its
  binary off `PATH` (see `scripts/ensure-qualifier.sh` and
  `hooks/session-start`). The regex is coarse: a `grep`/`cat` command whose
  own text happens to contain "qualifier record" would also match — the
  same tradeoff the suite's other regex graders already accept.
- `verified-tag.md` — some tool call carries a `verified:confirmed`,
  `verified:refuted`, or `verified:downgraded` tag (the verifier's reply),
  wherever it landed. It targets `trace` with a plain regex rather than one
  `tool` name, because the tag can arrive either in a `Write`'s JSONL
  `content` (the batch form `verifier-prompt.md` documents) or a Bash
  `--tag` flag (a direct `qualifier record --tag verified:confirmed ...`
  call) — `tool_used` can only check one tool at a time.
