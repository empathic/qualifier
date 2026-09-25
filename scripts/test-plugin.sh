#!/usr/bin/env bash
# Offline checks for the Claude Code plugin (plugins/claude-code): manifest
# consistency, ensure-qualifier.sh resolution and install against a stubbed
# GitHub release, the SessionStart hook, and skill structure.

set -euo pipefail
cd "$(dirname "$0")/.."

PLUGIN="plugins/claude-code"
ENSURE="$PWD/$PLUGIN/scripts/ensure-qualifier.sh"
PASS=0

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

ok() {
    PASS=$((PASS + 1))
    echo "ok: $*"
}

# --- manifests -------------------------------------------------------------

python3 - "$PLUGIN" <<'PY' || fail "manifest checks"
import json, sys

plugin_dir = sys.argv[1]
market = json.load(open(".claude-plugin/marketplace.json"))
plugin = json.load(open(f"{plugin_dir}/.claude-plugin/plugin.json"))

assert market["name"] == "qualifier", "marketplace must be named 'qualifier'"
entries = [p for p in market["plugins"] if p["name"] == plugin["name"]]
assert len(entries) == 1, f"marketplace must list exactly one '{plugin['name']}' plugin"
entry = entries[0]
assert entry["source"] == f"./{plugin_dir}", f"source {entry['source']!r} != ./{plugin_dir}"
assert entry["version"] == plugin["version"], (
    f"version mismatch: marketplace {entry['version']} vs plugin.json {plugin['version']}"
)
assert plugin["name"] == "qual", "plugin must be named 'qual' (skills are /qual:*)"
PY
ok "manifests parse and agree (plugin 'qual', versions match)"

bash -n "$ENSURE" || fail "ensure-qualifier.sh does not parse"
if command -v shellcheck >/dev/null 2>&1; then
    shellcheck "$ENSURE" "$0" || fail "shellcheck"
    ok "shellcheck clean"
else
    echo "skip: shellcheck not installed"
fi

# --- ensure-qualifier.sh ----------------------------------------------------

SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT
export HOME="$SANDBOX/home"
export QUALIFIER_INSTALL_DIR="$SANDBOX/global-bin"
mkdir -p "$HOME"
unset QUALIFIER_BIN

make_fake_qualifier() {
    # A stand-in binary that answers --version like the real CLI and
    # prints a fixed summary for `threads --summary`.
    local file="$1" version="$2" identity="${3:-qualifier}"
    mkdir -p "$(dirname "$file")"
    cat >"$file" <<EOF
#!/usr/bin/env bash
case "\${1:-}" in
    --version) echo "$identity $version" ;;
    threads) echo "qualifier: 1 blocker and 0 concerns open on files changed since main" ;;
    *) echo "fake-qualifier ran: \$*" ;;
esac
EOF
    chmod +x "$file"
}

# 1. A qualifier on PATH wins.
STUB1="$SANDBOX/stub1"
make_fake_qualifier "$STUB1/qualifier" "9.9.9"
out="$(PATH="$STUB1:$PATH" "$ENSURE")"
[ "$out" = "$STUB1/qualifier" ] || fail "expected $STUB1/qualifier, got $out"
ok "prefers a qualifier on PATH"

# 2. $QUALIFIER_BIN beats PATH.
STUB_OVERRIDE="$SANDBOX/stub-override"
make_fake_qualifier "$STUB_OVERRIDE/qualifier" "8.8.8"
out="$(QUALIFIER_BIN="$STUB_OVERRIDE/qualifier" PATH="$STUB1:$PATH" "$ENSURE")"
[ "$out" = "$STUB_OVERRIDE/qualifier" ] || fail "expected \$QUALIFIER_BIN to win, got $out"
ok "\$QUALIFIER_BIN wins over PATH"

# 3. An unusable $QUALIFIER_BIN warns and falls through.
err="$(QUALIFIER_BIN="$SANDBOX/gone/qualifier" PATH="$STUB1:$PATH" "$ENSURE" 2>&1 >/dev/null)"
out="$(QUALIFIER_BIN="$SANDBOX/gone/qualifier" PATH="$STUB1:$PATH" "$ENSURE" 2>/dev/null)"
case "$err" in *"QUALIFIER_BIN"*) ;; *) fail "expected a warning naming QUALIFIER_BIN, got: $err" ;; esac
[ "$out" = "$STUB1/qualifier" ] || fail "expected fall-through to $STUB1/qualifier, got $out"
ok "an unusable \$QUALIFIER_BIN warns and falls through"

# 4. A foreign binary named qualifier is rejected.
FOREIGN="$SANDBOX/foreign"
make_fake_qualifier "$FOREIGN/qualifier" "1.0" "something-else"
out="$(PATH="$FOREIGN:$STUB1:$PATH" "$ENSURE" 2>/dev/null)"
[ "$out" = "$STUB1/qualifier" ] || fail "expected the foreign binary to be skipped, got $out"
ok "skips a foreign binary named qualifier"

# 5. exec mode runs the resolved binary.
out="$(PATH="$STUB1:$PATH" "$ENSURE" exec --version)"
[ "$out" = "qualifier 9.9.9" ] || fail "exec mode: got '$out'"
ok "exec mode runs the resolved binary"

# 6. Older than MIN_VERSION warns but resolves.
STUB_OLD="$SANDBOX/stub-old"
make_fake_qualifier "$STUB_OLD/qualifier" "0.1.0"
err="$(PATH="$STUB_OLD:$PATH" "$ENSURE" 2>&1 >/dev/null)"
echo "$err" | grep -q "older than" || fail "expected an old-version warning, got: $err"
ok "warns when the binary predates MIN_VERSION"

# --- stubbed download ---
# The download tests run a copy of the wrapper pinned to a fixture release
# (version 9.9.9 and the fixture's checksum), served by a curl stub.

out="$("$ENSURE" min-version)"
[ "$out" = "0.8.0" ] || fail "min-version: expected 0.8.0, got $out"
ok "min-version reports MIN_VERSION without resolving a binary"

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)              TARGET="aarch64-apple-darwin";      SHAVAR="SHA256_AARCH64_APPLE_DARWIN" ;;
    Linux-x86_64)              TARGET="x86_64-unknown-linux-musl"; SHAVAR="SHA256_X86_64_UNKNOWN_LINUX_MUSL" ;;
    Linux-aarch64|Linux-arm64) TARGET="aarch64-unknown-linux-gnu"; SHAVAR="SHA256_AARCH64_UNKNOWN_LINUX_GNU" ;;
    *)
        echo "skip: no release target for $(uname -s)-$(uname -m); download tests skipped"
        echo "test-plugin: $PASS checks passed"
        exit 0
        ;;
esac

export FIXTURE_DIR="$SANDBOX/release"
mkdir -p "$FIXTURE_DIR"
make_fake_qualifier "$SANDBOX/payload/qualifier" "9.9.9"
tar -C "$SANDBOX/payload" -czf "$FIXTURE_DIR/qualifier-$TARGET.tar.gz" qualifier
if command -v sha256sum >/dev/null 2>&1; then
    FIXTURE_SHA="$(sha256sum "$FIXTURE_DIR/qualifier-$TARGET.tar.gz" | awk '{print $1}')"
else
    FIXTURE_SHA="$(shasum -a 256 "$FIXTURE_DIR/qualifier-$TARGET.tar.gz" | awk '{print $1}')"
fi

# A copy of the wrapper pinned to version 9.9.9 with the given checksum for
# this platform's target.
pinned_copy() {
    local dest="$1" sha="$2"
    sed -e 's/^PINNED_VERSION=.*/PINNED_VERSION="9.9.9"/' \
        -e "s/^${SHAVAR}=.*/${SHAVAR}=\"${sha}\"/" "$ENSURE" >"$dest"
    chmod +x "$dest"
    # shellcheck disable=SC2015 # fail-fast check, not if/then/else: either grep failing should fail.
    grep -q '^PINNED_VERSION="9.9.9"$' "$dest" && grep -q "^${SHAVAR}=\"${sha}\"$" "$dest" \
        || fail "could not pin a wrapper copy"
}

CURL_STUB="$SANDBOX/curl-stub"
mkdir -p "$CURL_STUB"
cat >"$CURL_STUB/curl" <<'EOF'
#!/usr/bin/env bash
out=""; url=""; prev=""
for a in "$@"; do
    case "$prev" in -o) out="$a" ;; esac
    case "$a" in
        -o|--connect-timeout|--max-time) prev="$a" ;;
        http://*|https://*) url="$a"; prev="" ;;
        *) prev="" ;;
    esac
done
case "$url" in
    https://github.com/empathic/qualifier/releases/download/v9.9.9/*)
        cp "$FIXTURE_DIR/$(basename "$url")" "$out" ;;
    *)
        echo "curl stub: unexpected URL $url" >&2
        exit 22 ;;
esac
EOF
chmod +x "$CURL_STUB/curl"

SAFE_PATH="$CURL_STUB:/usr/bin:/bin:/usr/sbin:/sbin"
if PATH="/usr/bin:/bin:/usr/sbin:/sbin" command -v qualifier >/dev/null 2>&1; then
    echo "skip: a qualifier binary exists in the system dirs; download tests skipped"
    echo "test-plugin: $PASS checks passed"
    exit 0
fi

# 7. No embedded checksum for this target: refuse to download, point at cargo.
pinned_copy "$SANDBOX/ensure-unverified.sh" ""
err="$(PATH="$SAFE_PATH" "$SANDBOX/ensure-unverified.sh" 2>&1 >/dev/null)" && fail "an unverified target must not install"
case "$err" in *"cargo install qualifier --version 9.9.9"*) ;; *) fail "expected the cargo fallback, got: $err" ;; esac
[ ! -e "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "nothing may be installed without a checksum"
ok "refuses to download a target without an embedded checksum"

# 8. Download the pinned release, verify, install.
pinned_copy "$SANDBOX/ensure-pinned.sh" "$FIXTURE_SHA"
out="$(PATH="$SAFE_PATH" "$SANDBOX/ensure-pinned.sh" 2>/dev/null)"
[ "$out" = "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "expected install to $QUALIFIER_INSTALL_DIR, got $out"
[ "$("$out" --version)" = "qualifier 9.9.9" ] || fail "installed binary does not run"
ok "downloads, verifies, and installs the pinned release"

# 9. A second run finds the installed binary without downloading.
out="$(PATH="/usr/bin:/bin" "$SANDBOX/ensure-pinned.sh" 2>/dev/null)"
[ "$out" = "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "expected the installed binary, got $out"
ok "reuses the installed binary"

# 10. A checksum mismatch aborts and installs nothing.
rm -f "$QUALIFIER_INSTALL_DIR/qualifier"
pinned_copy "$SANDBOX/ensure-wrong-sha.sh" "0000000000000000000000000000000000000000000000000000000000000000"
if PATH="$SAFE_PATH" "$SANDBOX/ensure-wrong-sha.sh" >/dev/null 2>&1; then
    fail "a checksum mismatch must fail"
fi
[ ! -e "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "nothing may be installed on checksum mismatch"
ok "checksum mismatch aborts the install"

echo "test-plugin: $PASS checks passed"
