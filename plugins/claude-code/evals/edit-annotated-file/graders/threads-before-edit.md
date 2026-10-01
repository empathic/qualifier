---
type: tool_order
before: { tool: Bash, input_match: '"command"\s*:\s*"(?:[^"\\]|\\.)*?(?:\bexec|(?:(?<=\\[nt])|(?<![\w-]))qualifier)\s+threads\b(?:[^"\\]|\\.)*?\bsrc/net\.rs\b' }
after: { tool: Edit, input_match: '"file_path"\s*:\s*"(?:[^"]*/)?src/net\.rs"' }
---
