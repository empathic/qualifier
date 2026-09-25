---
max_turns: 15
allowed_tools: [Read, Glob, Grep, Edit, Write, Bash, Skill]
---

Change connect in src/net.rs to use TcpStream::connect_timeout with a 5 second timeout, then commit it.
