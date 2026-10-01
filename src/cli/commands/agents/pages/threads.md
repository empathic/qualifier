+++
name = "threads"
summary = "List conversation threads (roots, replies, open/closed) across the project"
sees_also = ["reply", "resolve", "show"]
since = "0.8.0"
+++

# qualifier threads

## Purpose

List every open thread in the project: the root annotation, its live
replies, and whether it is closed. This is the entry point for triage,
planning, and "what is open on the code I'm about to touch".

## Usage

```bash
qualifier threads                           # all open threads
qualifier threads src/net                   # under src/net, or on src (an ancestor)
qualifier threads 'src/**/*.rs'             # glob (* does not cross /)
qualifier threads src/net/tcp.rs:40:80      # span overlaps 40–80, span-less on the file, or on src/net, src
qualifier threads 3f9a2c1d                  # thread containing a record with this ID prefix
qualifier threads --kind blocker,concern
qualifier threads --tag 'status:*'          # tag on root or a live reply (or closed_by, with --all)
qualifier threads --status needs-decision   # latest status:* tag on the thread
qualifier threads --changed-since main      # root subject changed vs. main (git only)
qualifier threads --all                     # include closed threads
qualifier threads --format json             # one JSON array
qualifier threads --summary                 # 0-2 line digest for session-start hooks
```

A path filter also matches threads on the path's ancestor directories: a
concern on `src/net` shows up for `qualifier threads src/net/tcp.rs:40:80`,
as does a span-less blocker on `src/net/tcp.rs`. An argument of 4+ hex
characters is an ID prefix and matches the thread containing that record —
as root, origin, history, reply, or `closed_by` — so
`qualifier threads <id> --format json` fetches one thread without pulling
the whole project. Write `./cafe` for a directory whose name is all hex.

Locations are relative to the current directory; subjects are stored
relative to the project root. From `src/`, `qualifier threads net/tcp.rs`
and `qualifier threads ../src/net/tcp.rs` both select `src/net/tcp.rs`.

## Human output

Each thread starts with its root line; an open thread's live replies
follow, indented. The root line ends with the thread's state when there is
one to report:

```
[3f9a2c1d] concern    src/net/tcp.rs:40  retry loop never backs off — needs decision from alice
    [7b21e0aa] comment    Option A: exponential; option B: fixed 1s. Which?
[31ef1c78] concern    lib.rs:1  a() rounds wrong — closed (wontfix): Won't change b()
```

A closed thread (listed with `--all`) is one line that carries its answer:
the closing resolve's reason and summary, plus `— question still pending`
when it closed while its latest `status:*` tag was still
`status:needs-decision`. Its replies are not printed, so piping through
`head` or `tail` never shows a question without its answer. Ask for the
thread by ID (`qualifier threads --all <id>`) to see its replies, with the
closing resolve as the last line. Human output is a summary; JSON is
complete.

## JSON shape

```json
[{"origin": "<full id>", "open": true,
  "state": {"name": "needs-decision", "addressee": "mailto:alice@example.com"},
  "root": { ...record... }, "closed_by": null,
  "history": [{ ...record... }],
  "replies": [{"active": true, "record": { ...record... }}],
  "latest_at": "2026-09-24T10:00:00+00:00"}]
```

`state.name` is `open`, `needs-decision` (with `addressee`, or null),
`decided`, or `closed` (with `reason` from the resolve's `reason:*` tag,
`closed_by` ID, and `pending_question`). It comes from the latest
`status:*` tag on the root, a live reply, or the closing resolve;
`status:deferred` counts as `open`.

Reply to or resolve `root.id` — the live head — not `origin`, which may be
superseded.
