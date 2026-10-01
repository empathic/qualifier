---
max_turns: 40
allowed_tools: [Read, Glob, Grep, Edit, Write, Bash, Skill, Agent]
---

Review the whole src/ tree for bugs — src/net.rs, src/auth.rs, and
src/cache.rs are three separate subsystems, so treat this as a full,
large-scope review of all of them, not a quick pass over one file.
