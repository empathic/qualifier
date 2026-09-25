---
name: consulting-threads
description: Use before modifying a file in a repository with .qual files — reads the open blockers, concerns, and alternatives on the lines about to change so the edit accounts for them
allowed-tools: Bash(qualifier:*), Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*)
---

# Consulting threads before editing

Once per file per task, before the first edit:

```bash
qualifier threads src/net/tcp.rs            # the whole file
qualifier threads src/net/tcp.rs:40:80      # just the lines you will change
```

## Act on what you find

- **Open `blocker` on those lines:** either this edit fixes it — then
  `qual:closing-the-loop` resolves it after you commit — or the edit must
  not make it worse. State which in your working notes.
- **`concern` or `suggestion`:** fold it in if it fits the task; otherwise
  leave it.
- **`alternative` whose `revisit:` condition now holds:** stop and tell the
  user before continuing; the design may need to change.
- **`status:needs-decision`:** don't pre-empt the human's call in code.

Don't re-run for every edit to the same file within a task.

Next: `qual:closing-the-loop` when the change is done.
