#!/usr/bin/env bash
# Build the eval fixture in the current directory: a git repo with a spec,
# a source file, and qualifier threads including a blocker on src/net.rs.
set -euo pipefail
git init -q --initial-branch=main
git config user.email eval@example.com
git config user.name eval
mkdir -p src docs
cat > docs/cache-design.md <<'EOF'
# Cache design

## Approach

A single shared cache process serves every tenant.

## Eviction

LRU with a 10-minute TTL.
EOF
cat > src/net.rs <<'EOF'
pub fn connect(addr: &str) -> std::io::Result<std::net::TcpStream> {
    let stream = std::net::TcpStream::connect(addr)?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

pub fn send(stream: &mut std::net::TcpStream, buf: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    stream.write_all(buf)
}
EOF
Q="${QUALIFIER_BIN:-qualifier}"
"$Q" record blocker src/net.rs:1:5 "connect has no timeout; a dead peer hangs the caller" \
  --suggested-fix "Use TcpStream::connect_timeout with a configurable budget" --issuer mailto:eval@example.com
"$Q" record concern src/net.rs:7:10 "send ignores partial-write retries on EINTR" --issuer mailto:eval@example.com
"$Q" record suggestion docs/cache-design.md:5 "Consider per-tenant quotas" --issuer mailto:eval@example.com
echo "Entries are recieved from the origin on a miss." >> docs/cache-design.md
git add -A && git commit -q -m fixture
