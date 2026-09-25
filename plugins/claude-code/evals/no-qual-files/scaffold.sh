#!/usr/bin/env bash
# Build a fixture with no qualifier threads at all, to confirm the qual
# plugin's skills stay quiet when there is nothing for them to consult.
set -euo pipefail
git init -q --initial-branch=main
# Never sign in the scratch repo, whatever the host's global config says.
git config commit.gpgsign false
mkdir -p src
cat > src/net.rs <<'RUST'
pub fn connect(addr: &str) -> std::io::Result<std::net::TcpStream> {
    let stream = std::net::TcpStream::connect(addr)?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

pub fn send(stream: &mut std::net::TcpStream, buf: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    stream.write_all(buf)
}
RUST
