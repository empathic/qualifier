#!/usr/bin/env bash
# Build the eval fixture in the current directory: a git repo with a spec,
# source files across a few subsystems (networking, auth, caching), and
# qualifier threads including a blocker on src/net.rs.
set -euo pipefail

# `claude plugin eval --scaffold` always runs this in an empty workspace
# (https://code.claude.com/docs/en/plugin-evals.md: "A scaffold script
# starts in the empty workspace..."). That's the only guarantee the docs
# make, so it's the only thing to check: an empty directory is safe to run
# this in even when it's nested inside another repository, because `git
# init` below creates this directory's own .git before any `git config`
# runs, so config always lands in that new repo, never an enclosing one. A
# non-empty directory isn't safe (a repository root always has a `.git`, so
# it is never empty), so refuse before touching git or the filesystem, in
# case this is ever run by hand from one.
#
# Fail closed: an unreadable directory makes `ls -A .` itself fail (and
# print nothing), which must not be mistaken for "empty". Capture the
# listing inside the `if` condition so `set -e` doesn't exit before this
# refusal can print its own message.
if ! listing="$(ls -A . 2>/dev/null)" || [ -n "$listing" ]; then
    echo "scaffold.sh: refusing to run in a non-empty or unreadable directory ($PWD)" >&2
    exit 1
fi

git init -q --initial-branch=main
git config user.email eval@example.com
git config user.name eval
# The scratch repo commits under the host's global config; signing there
# would fail (or prompt) with no key.
git config commit.gpgsign false
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
cat > src/auth.rs <<'EOF'
pub fn check_password(candidate: &str, expected: &str) -> bool {
    candidate == expected
}

pub fn session_token(user: &str) -> String {
    format!("{}-{}", user, "static-salt")
}
EOF
cat > src/cache.rs <<'EOF'
use std::collections::HashMap;

pub struct Cache {
    entries: HashMap<String, String>,
}

impl Cache {
    pub fn new() -> Self {
        Cache { entries: HashMap::new() }
    }

    pub fn get(&self, key: &str) -> String {
        self.entries.get(key).unwrap().clone()
    }
}
EOF
Q="${QUALIFIER_BIN:-qualifier}"
"$Q" record blocker src/net.rs:1:5 "connect has no timeout; a dead peer hangs the caller" \
  --suggested-fix "Use TcpStream::connect_timeout with a configurable budget" --issuer mailto:eval@example.com
"$Q" record concern src/net.rs:7:10 "send ignores partial-write retries on EINTR" --issuer mailto:eval@example.com
"$Q" record suggestion docs/cache-design.md:5 "Consider per-tenant quotas" --issuer mailto:eval@example.com
echo "Entries are recieved from the origin on a miss." >> docs/cache-design.md
git add -A && git commit -q -m fixture
