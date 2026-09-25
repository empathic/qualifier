---
max_turns: 15
allowed_tools: [Read, Glob, Grep, Edit, Write, Bash, Skill]
---

For docs/cache-design.md we considered per-tenant cache processes and decided against them — one shared process is simpler until we have more than 50 tenants. Keep the shared design.
