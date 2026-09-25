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
qualifier threads src/net                   # under a directory
qualifier threads 'src/**/*.rs'             # glob (* does not cross /)
qualifier threads src/net/tcp.rs:40:80      # root span overlaps 40–80
qualifier threads --kind blocker,concern
qualifier threads --tag 'status:*'          # tag on root or a live reply
qualifier threads --status needs-decision   # latest status:* tag on the thread
qualifier threads --changed-since main      # root subject changed vs. main (git only)
qualifier threads --all                     # include closed threads
qualifier threads --format json             # one JSON array
qualifier threads --summary                 # 0-2 line digest for session-start hooks
```

Locations are relative to the current directory; subjects are stored
relative to the project root. From `src/`, `qualifier threads net/tcp.rs`
and `qualifier threads ../src/net/tcp.rs` both select `src/net/tcp.rs`.

## JSON shape

```json
[{"origin": "<full id>", "open": true,
  "root": { ...record... }, "closed_by": null,
  "history": [{ ...record... }],
  "replies": [{"active": true, "record": { ...record... }}],
  "latest_at": "2026-09-24T10:00:00+00:00"}]
```

Reply to or resolve `root.id` — the live head — not `origin`, which may be
superseded.
