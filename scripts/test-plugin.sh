#!/usr/bin/env bash
# Offline checks for the Claude Code plugin (plugins/claude-code): manifest
# consistency, ensure-qualifier.sh resolution and its managed install against
# a stubbed GitHub release, the SessionStart hook, and skill structure.

set -euo pipefail
cd "$(dirname "$0")/.."

PLUGIN="plugins/claude-code"
ENSURE="$PWD/$PLUGIN/scripts/ensure-qualifier.sh"
HOOK="$PWD/$PLUGIN/hooks/session-start"
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

"$BASH" -n "$ENSURE" || fail "ensure-qualifier.sh does not parse"
"$BASH" -n "$HOOK" || fail "session-start does not parse"
python3 -c "import json; json.load(open('$PLUGIN/hooks/hooks.json'))" || fail "hooks.json is not valid JSON"
if command -v shellcheck >/dev/null 2>&1; then
    shellcheck "$ENSURE" "$HOOK" "$0" || fail "shellcheck"
    ok "shellcheck clean"
else
    echo "skip: shellcheck not installed"
fi

PINNED="$("$BASH" "$ENSURE" pinned-version)"

# The binary the skill-example check runs (the wrapper tests below unset
# QUALIFIER_BIN); empty means the check looks under target/.
EXAMPLES_BIN="${QUALIFIER_BIN:-}"
# The binary the eval-grader checks run (the qualifier subcommand list, the
# fixture scaffold); empty when there is none, and those checks skip.
GRADER_BIN="$EXAMPLES_BIN"
for candidate in target/debug/qualifier target/release/qualifier; do
    [ -n "$GRADER_BIN" ] && break
    [ -x "$candidate" ] && GRADER_BIN="$PWD/$candidate"
done

# --- sandbox and stubs -----------------------------------------------------

SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT
export HOME="$SANDBOX/home"
mkdir -p "$HOME"
unset QUALIFIER_BIN QUALIFIER_PLUGIN_HOME XDG_DATA_HOME

# The wrapper, the hook, and every stub start with `#!/usr/bin/env bash`.
# A `bash` first on each PATH the tests use makes all of them run under the
# interpreter running this script, so `/bin/bash scripts/test-plugin.sh`
# exercises macOS's bash 3.2 throughout, even with a newer bash on PATH.
INTERP="$SANDBOX/interp"
mkdir -p "$INTERP"
ln -s "$BASH" "$INTERP/bash"
export PATH="$INTERP:$PATH"
for script in "$ENSURE" "$HOOK"; do
    [ "$(head -n 1 "$script")" = "#!/usr/bin/env bash" ] \
        || fail "$script must start with #!/usr/bin/env bash (the interpreter shim relies on it)"
done
cat >"$SANDBOX/bash-probe" <<'EOF'
#!/usr/bin/env bash
echo "$BASH_VERSION"
EOF
chmod +x "$SANDBOX/bash-probe"
for probe_path in "$PATH" "$INTERP:/usr/bin:/bin"; do
    got="$(PATH="$probe_path" "$SANDBOX/bash-probe")"
    [ "$got" = "$BASH_VERSION" ] || fail "scripts under test run bash $got, not this script's $BASH_VERSION"
done
ok "the wrapper, hook, and stubs run under this script's bash ($BASH, $BASH_VERSION)"

make_fake_qualifier() {
    # A stand-in binary that answers --version like the real CLI and prints
    # a fixed summary for `threads --summary`.
    local file="$1" version="$2" identity="${3:-qualifier}"
    local summary="${4:-qualifier: 1 blocker and 0 concerns open on files changed since main}"
    mkdir -p "$(dirname "$file")"
    cat >"$file" <<EOF
#!/usr/bin/env bash
case "\${1:-}" in
    --version) echo "$identity $version" ;;
    threads) echo "$summary" ;;
    *) echo "fake-qualifier ran: \$*" ;;
esac
EOF
    chmod +x "$file"
}

# Every curl stub touches this marker, so a test can assert "no download".
export CURL_MARKER="$SANDBOX/curl-was-called"

# curl that refuses all network access.
NOACCESS="$SANDBOX/curl-noaccess"
mkdir -p "$NOACCESS"
cat >"$NOACCESS/curl" <<'EOF'
#!/usr/bin/env bash
touch "$CURL_MARKER"
echo "curl stub: unexpected network access" >&2
exit 7
EOF
chmod +x "$NOACCESS/curl"

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

# Writes <dir>/qualifier.sha256, the record of <dir>/qualifier's sha256 an
# install leaves beside the binary.
record_install_hash() {
    sha256_file "$1/qualifier" >"$1/qualifier.sha256"
}

# A managed install of the real pinned version, for tests of the shipped
# wrapper that must not download anything.
populate_managed() {
    make_fake_qualifier "$1/$PINNED/qualifier" "$PINNED" qualifier "${2:-qualifier: 1 blocker and 0 concerns open on files changed since main}"
    record_install_hash "$1/$PINNED"
}

# A uname shim that reports FAKE_UNAME_S for -s and FAKE_UNAME_M for -m, so
# every platform mapping in the wrapper runs on any host.
UNAME_SHIM="$SANDBOX/uname-shim"
mkdir -p "$UNAME_SHIM"
cat >"$UNAME_SHIM/uname" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
    -m) echo "$FAKE_UNAME_M" ;;
    *) echo "$FAKE_UNAME_S" ;;
esac
EOF
chmod +x "$UNAME_SHIM/uname"

# --- ensure-qualifier.sh: modes and $QUALIFIER_BIN -------------------------

# W1. pinned-version resolves nothing: it works with an empty PATH.
out="$(env -i PATH= HOME="$HOME" "$BASH" "$ENSURE" pinned-version)" || fail "pinned-version failed with an empty PATH"
[ "$out" = "0.8.0" ] || fail "pinned-version: expected 0.8.0, got $out"
ok "pinned-version reports PINNED_VERSION with an empty PATH"

# W2. The min-version mode is gone.
if "$ENSURE" min-version >/dev/null 2>&1; then fail "min-version must no longer be a mode"; fi
ok "min-version is not a mode"

MANAGED="$SANDBOX/managed"
populate_managed "$MANAGED"
MANAGED_BIN="$MANAGED/$PINNED/qualifier"
PATHQ="$SANDBOX/path-qualifier"
make_fake_qualifier "$PATHQ/qualifier" "$PINNED" qualifier "PATH-QUALIFIER-SUMMARY"
OVERRIDE="$SANDBOX/override"
make_fake_qualifier "$OVERRIDE/qualifier" "8.8.8"

# W3. A valid $QUALIFIER_BIN wins over the managed install and PATH.
rm -f "$CURL_MARKER"
out="$(QUALIFIER_BIN="$OVERRIDE/qualifier" QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$PATHQ:$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE")"
[ "$out" = "$OVERRIDE/qualifier" ] || fail "expected \$QUALIFIER_BIN to win, got $out"
[ ! -e "$CURL_MARKER" ] || fail "a valid \$QUALIFIER_BIN must not download"
ok "\$QUALIFIER_BIN wins"

# W4. An unusable $QUALIFIER_BIN (missing, not executable, foreign, or
#     relative) warns and falls through to the managed install.
NOEXEC="$SANDBOX/noexec/qualifier"
make_fake_qualifier "$NOEXEC" "8.8.8"
chmod -x "$NOEXEC"
FOREIGN="$SANDBOX/foreign/qualifier"
make_fake_qualifier "$FOREIGN" "1.0" "something-else"
for bad in "$SANDBOX/gone/qualifier" "$NOEXEC" "$FOREIGN" "override/qualifier"; do
    err="$(cd "$SANDBOX" && QUALIFIER_BIN="$bad" QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$PATHQ:$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE" 2>&1 >/dev/null)"
    out="$(cd "$SANDBOX" && QUALIFIER_BIN="$bad" QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$PATHQ:$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE" 2>/dev/null)"
    case "$err" in *"QUALIFIER_BIN"*) ;; *) fail "expected a warning naming QUALIFIER_BIN for '$bad', got: $err" ;; esac
    [ "$out" = "$MANAGED_BIN" ] || fail "'$bad': expected fall-through to $MANAGED_BIN, got $out"
done
ok "an unusable, foreign, or relative \$QUALIFIER_BIN warns and falls through"

# W5. A qualifier on PATH is ignored when a managed install exists.
rm -f "$CURL_MARKER"
out="$(QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$PATHQ:$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE")"
[ "$out" = "$MANAGED_BIN" ] || fail "expected the managed install, got $out"
[ ! -e "$CURL_MARKER" ] || fail "a valid managed install must not download"
ok "uses the managed install, not a qualifier on PATH"

# W6. exec mode resolves, then runs the binary with the arguments.
out="$(QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$PATHQ:$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE" exec --version)"
[ "$out" = "qualifier $PINNED" ] || fail "exec mode: got '$out'"
out="$(QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$PATHQ:$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE" exec show "a b")"
[ "$out" = "fake-qualifier ran: show a b" ] || fail "exec mode arguments: got '$out'"
ok "exec mode runs the managed binary with the arguments"

# W7. The default plugin home is $XDG_DATA_HOME/qualifier/plugin, else
#     ~/.local/share/qualifier/plugin.
populate_managed "$SANDBOX/xdg/qualifier/plugin"
out="$(XDG_DATA_HOME="$SANDBOX/xdg" PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE")"
[ "$out" = "$SANDBOX/xdg/qualifier/plugin/$PINNED/qualifier" ] || fail "XDG_DATA_HOME default: got $out"
populate_managed "$HOME/.local/share/qualifier/plugin"
out="$(PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE")"
[ "$out" = "$HOME/.local/share/qualifier/plugin/$PINNED/qualifier" ] || fail "HOME default: got $out"
ok "plugin home defaults to \$XDG_DATA_HOME, then ~/.local/share"

# W8. An unusable plugin home (relative, or /) from QUALIFIER_PLUGIN_HOME or
#     XDG_DATA_HOME warns and falls back to ~/.local/share/qualifier/plugin.
DEFAULT_BIN="$HOME/.local/share/qualifier/plugin/$PINNED/qualifier"
for bad in "QUALIFIER_PLUGIN_HOME=relative/home" "QUALIFIER_PLUGIN_HOME=/" "QUALIFIER_PLUGIN_HOME=///" \
           "XDG_DATA_HOME=relative/data" "XDG_DATA_HOME=/"; do
    err="$(cd "$SANDBOX" && env "$bad" PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE" 2>&1 >/dev/null)" \
        || fail "$bad: expected a fall-back, got an error: $err"
    out="$(cd "$SANDBOX" && env "$bad" PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE" 2>/dev/null)"
    case "$err" in *"${bad%%=*}"*) ;; *) fail "$bad: expected a warning naming ${bad%%=*}, got: $err" ;; esac
    [ "$out" = "$DEFAULT_BIN" ] || fail "$bad: expected fall-back to $DEFAULT_BIN, got $out"
done
[ ! -e "$SANDBOX/relative" ] || fail "a relative plugin home must not be created"
ok "an unusable QUALIFIER_PLUGIN_HOME or XDG_DATA_HOME warns and falls back to the default"

# W9. With no usable plugin home and no usable HOME, exit 1 before touching
#     anything; with HOME unset but QUALIFIER_PLUGIN_HOME valid, resolve.
for env_args in "-u HOME QUALIFIER_PLUGIN_HOME=/" "-u HOME" "HOME=relative-home" "HOME="; do
    # Word splitting of the env arguments is intended.
    # shellcheck disable=SC2086
    err="$(cd "$SANDBOX" && env $env_args PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE" 2>&1 >/dev/null)" \
        && fail "env $env_args: expected exit 1"
    case "$err" in *"HOME"*) ;; *) fail "env $env_args: expected an error naming HOME, got: $err" ;; esac
done
out="$(env -u HOME QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE")"
[ "$out" = "$MANAGED_BIN" ] || fail "HOME unset with a valid QUALIFIER_PLUGIN_HOME: got $out"
ok "no usable plugin home exits 1; a valid QUALIFIER_PLUGIN_HOME works without HOME"

# W9b. A valid $QUALIFIER_BIN is used even when there is no usable plugin
#      home: it is resolved before the plugin home is computed.
out="$(env -u HOME QUALIFIER_BIN="$OVERRIDE/qualifier" PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$ENSURE")" \
    || fail "a valid QUALIFIER_BIN with HOME unset must resolve"
[ "$out" = "$OVERRIDE/qualifier" ] || fail "QUALIFIER_BIN with HOME unset: got $out"
ok "a valid \$QUALIFIER_BIN works with HOME unset"

# W9c. HOME=/ (a passwd-less UID in a container) is accepted: the default
#      plugin home is /.local/share/qualifier/plugin. Checked on the
#      computed value, since nothing may be written to /.
tail -n 1 "$ENSURE" | grep -qx 'main "\$@"' || fail "ensure-qualifier.sh must end with main \"\$@\""
sed '$d' "$ENSURE" >"$SANDBOX/ensure-functions.sh"
# The single-quoted script expands $1 and $PLUGIN_HOME in the inner shell.
# shellcheck disable=SC2016
out="$(env -u QUALIFIER_PLUGIN_HOME -u XDG_DATA_HOME HOME=/ "$BASH" -c \
    '. "$1"; plugin_home; echo "$PLUGIN_HOME"' _ "$SANDBOX/ensure-functions.sh")" \
    || fail "HOME=/ must be accepted"
[ "$out" = "/.local/share/qualifier/plugin" ] || fail "HOME=/: expected /.local/share/qualifier/plugin, got $out"
ok "HOME=/ gives the default plugin home /.local/share/qualifier/plugin"

# --- managed install against a stubbed release -----------------------------
# These run copies of the wrapper pinned to fixture releases (with the
# fixtures' checksums), served by a curl stub.

export FIXTURE_DIR="$SANDBOX/release"
# Builds the fixture release tarball for version $1 and target $2 and prints
# its sha256. The payload's `threads` output is $3 (default: the target), so
# each target's tarball, and the binary installed from it, is distinct.
make_fixture() {
    local version="$1" target="$2" payload="$SANDBOX/payload-$1-$2"
    make_fake_qualifier "$payload/qualifier" "$version" qualifier "${3:-$target}"
    mkdir -p "$FIXTURE_DIR/v$version"
    tar -C "$payload" -czf "$FIXTURE_DIR/v$version/qualifier-$target.tar.gz" qualifier
    sha256_file "$FIXTURE_DIR/v$version/qualifier-$target.tar.gz"
}

# A copy of the wrapper pinned to version $2 with checksum $3 in the
# checksum variable $4 (default: this platform's, $SHAVAR).
pinned_copy() {
    local dest="$1" version="$2" sha="$3" var="${4:-$SHAVAR}"
    sed -e "s/^PINNED_VERSION=.*/PINNED_VERSION=\"${version}\"/" \
        -e "s/^${var}=.*/${var}=\"${sha}\"/" "$ENSURE" >"$dest"
    chmod +x "$dest"
    # Fail-fast check, not if/then/else: either grep failing should fail.
    # shellcheck disable=SC2015
    grep -q "^PINNED_VERSION=\"${version}\"$" "$dest" && grep -q "^${var}=\"${sha}\"$" "$dest" \
        || fail "could not pin a wrapper copy"
}

# Serves the fixture releases. When CURL_RACE_DEST is set, it first plays a
# concurrent session that finishes its install there while this download is
# in flight, copying CURL_RACE_SRC and its recorded hash into place.
CURL_STUB="$SANDBOX/curl-stub"
mkdir -p "$CURL_STUB"
cat >"$CURL_STUB/curl" <<'EOF'
#!/usr/bin/env bash
touch "$CURL_MARKER"
out=""; url=""; prev=""
for a in "$@"; do
    case "$prev" in -o) out="$a" ;; esac
    case "$a" in
        -o|--connect-timeout|--max-time) prev="$a" ;;
        http://*|https://*) url="$a"; prev="" ;;
        *) prev="" ;;
    esac
done
if [ -n "${CURL_RACE_DEST:-}" ]; then
    mkdir -p "$CURL_RACE_DEST"
    cp "$CURL_RACE_SRC" "$CURL_RACE_DEST/qualifier"
    cp "$CURL_RACE_SRC.sha256" "$CURL_RACE_DEST/qualifier.sha256"
fi
case "$url" in
    https://github.com/empathic/qualifier/releases/download/v*/*)
        rest="${url#https://github.com/empathic/qualifier/releases/download/}"
        cp "$FIXTURE_DIR/${rest%%/*}/$(basename "$url")" "$out" ;;
    *)
        echo "curl stub: unexpected URL $url" >&2
        exit 22 ;;
esac
EOF
chmod +x "$CURL_STUB/curl"
SAFE_PATH="$CURL_STUB:$INTERP:/usr/bin:/bin:/usr/sbin:/sbin"

# P1. Every platform mapping in the shipped wrapper resolves to its release
#     target, and that target has a 64-hex-digit embedded checksum.
# uname-s uname-m target checksum-variable
PLATFORMS="Darwin arm64 aarch64-apple-darwin SHA256_AARCH64_APPLE_DARWIN
Linux x86_64 x86_64-unknown-linux-musl SHA256_X86_64_UNKNOWN_LINUX_MUSL
Linux aarch64 aarch64-unknown-linux-gnu SHA256_AARCH64_UNKNOWN_LINUX_GNU
Linux arm64 aarch64-unknown-linux-gnu SHA256_AARCH64_UNKNOWN_LINUX_GNU"
while read -r os arch target var; do
    # The single-quoted script expands its positional parameters in the
    # inner shell.
    # shellcheck disable=SC2016
    out="$(FAKE_UNAME_S="$os" FAKE_UNAME_M="$arch" PATH="$UNAME_SHIM:$INTERP:/usr/bin:/bin" "$BASH" -c \
        '. "$1"; t="$(resolve_target)"; echo "$t $(expected_sha256 "$t")"' _ "$SANDBOX/ensure-functions.sh")" \
        || fail "$os-$arch: resolve_target failed"
    [ "${out%% *}" = "$target" ] || fail "$os-$arch: expected target $target, got ${out%% *}"
    sha="${out#* }"
    case "$sha" in
        *[!0-9a-f]*|"") fail "$os-$arch: checksum for $target is not hex: '$sha'" ;;
    esac
    [ "${#sha}" -eq 64 ] || fail "$os-$arch: checksum for $target is ${#sha} chars, not 64"
    grep -q "^${var}=\"${sha}\"$" "$ENSURE" || fail "$os-$arch: $target's checksum does not come from $var"
    got="$(FAKE_UNAME_S="$os" FAKE_UNAME_M="$arch" PATH="$UNAME_SHIM:$INTERP:/usr/bin:/bin" "$ENSURE" prebuilt-target)" \
        || fail "$os-$arch: prebuilt-target failed"
    [ "$got" = "$target" ] || fail "$os-$arch: prebuilt-target printed '$got', not $target"
done <<EOF
$PLATFORMS
EOF
ok "every platform mapping resolves to its target (also via prebuilt-target) with a 64-hex-digit embedded checksum"

# P2. The full download/verify/install path for each target, whatever the
#     host: the wrapper fetches that target's tarball, verifies it against
#     that target's checksum variable (the others keep the real release
#     checksums, which the fixtures don't match), and installs it.
while read -r os arch target var; do
    sha="$(make_fixture 9.8.0 "$target")"
    pinned_copy "$SANDBOX/ensure-$target.sh" 9.8.0 "$sha" "$var"
    ph="$SANDBOX/home-$target-$arch"
    rm -f "$CURL_MARKER"
    out="$(FAKE_UNAME_S="$os" FAKE_UNAME_M="$arch" QUALIFIER_PLUGIN_HOME="$ph" \
        PATH="$UNAME_SHIM:$SAFE_PATH" "$SANDBOX/ensure-$target.sh" 2>/dev/null)" \
        || fail "$os-$arch: install of $target failed"
    [ "$out" = "$ph/9.8.0/qualifier" ] || fail "$os-$arch: expected $ph/9.8.0/qualifier, got $out"
    [ -e "$CURL_MARKER" ] || fail "$os-$arch: expected a download"
    [ "$("$out" threads)" = "$target" ] || fail "$os-$arch: installed the wrong tarball: $("$out" threads)"
    [ "$(cat "$ph/9.8.0/qualifier.sha256")" = "$(sha256_file "$out")" ] \
        || fail "$os-$arch: the install's recorded hash does not match its binary"
done <<EOF
$PLATFORMS
EOF
ok "downloads, verifies, and installs the release for every target"

# P3. A platform without a release target falls back to cargo: exit 1,
#     nothing downloaded, nothing created.
for platform in "Darwin x86_64" "FreeBSD amd64" "Linux armv7l"; do
    os="${platform% *}"; arch="${platform#* }"
    ph="$SANDBOX/home-unsupported-$os-$arch"
    rm -f "$CURL_MARKER"
    err="$(FAKE_UNAME_S="$os" FAKE_UNAME_M="$arch" QUALIFIER_PLUGIN_HOME="$ph" \
        PATH="$UNAME_SHIM:$SAFE_PATH" "$ENSURE" 2>&1 >/dev/null)" \
        && fail "$os-$arch: an unsupported platform must fail"
    case "$err" in *"cargo install qualifier --version $PINNED --locked"*"QUALIFIER_BIN"*) ;; *) fail "$os-$arch: expected the cargo fallback and QUALIFIER_BIN, got: $err" ;; esac
    FAKE_UNAME_S="$os" FAKE_UNAME_M="$arch" PATH="$UNAME_SHIM:$SAFE_PATH" "$ENSURE" prebuilt-target >/dev/null 2>&1 \
        && fail "$os-$arch: prebuilt-target must fail on an unsupported platform"
    [ ! -e "$CURL_MARKER" ] || fail "$os-$arch: an unsupported platform must not download"
    [ ! -e "$ph" ] || fail "$os-$arch: an unsupported platform must not create the plugin home"
done
ok "an unsupported platform falls back to cargo without downloading; prebuilt-target fails"

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)              TARGET="aarch64-apple-darwin";      SHAVAR="SHA256_AARCH64_APPLE_DARWIN" ;;
    Linux-x86_64)              TARGET="x86_64-unknown-linux-musl"; SHAVAR="SHA256_X86_64_UNKNOWN_LINUX_MUSL" ;;
    Linux-aarch64|Linux-arm64) TARGET="aarch64-unknown-linux-gnu"; SHAVAR="SHA256_AARCH64_UNKNOWN_LINUX_GNU" ;;
    *)                         TARGET="" ;;
esac

# D1-D10 need a release target for this host (their fixtures are served
# for it); P1-P3 above cover every target on any host. The body is left
# unindented so its heredocs keep their column-0 terminators.
if [ -z "$TARGET" ]; then
    echo "skip: no release target for $(uname -s)-$(uname -m); host download tests skipped"
else


SHA_999="$(make_fixture 9.9.9 "$TARGET")"
SHA_9910="$(make_fixture 9.9.10 "$TARGET")"
pinned_copy "$SANDBOX/ensure-999.sh" 9.9.9 "$SHA_999"
pinned_copy "$SANDBOX/ensure-9910.sh" 9.9.10 "$SHA_9910"
pinned_copy "$SANDBOX/ensure-unverified.sh" 9.9.9 ""
pinned_copy "$SANDBOX/ensure-wrong-sha.sh" 9.9.9 "0000000000000000000000000000000000000000000000000000000000000000"

# Lists a plugin home's entries, dotfiles included, space-separated.
entries_of() {
    (cd "$1" && find . -mindepth 1 -maxdepth 1 | sed 's|^\./||' | LC_ALL=C sort | tr '\n' ' ' | sed 's/ $//')
}

# D1. A qualifier on PATH, of any version including the pinned one, is not
#     used: resolution downloads into the managed install.
for v in 9.9.9 0.1.0; do
    ph="$SANDBOX/home-path-$v"
    pq="$SANDBOX/pathq-$v"
    make_fake_qualifier "$pq/qualifier" "$v"
    rm -f "$CURL_MARKER"
    out="$(QUALIFIER_PLUGIN_HOME="$ph" PATH="$pq:$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
    [ "$out" = "$ph/9.9.9/qualifier" ] || fail "PATH qualifier $v must be ignored; got $out"
    [ -e "$CURL_MARKER" ] || fail "PATH qualifier $v: expected a download"
done
ok "a qualifier on PATH (pinned version or not) is not used"

# D2. Fresh install: download, verify, install to <home>/<pinned>/qualifier.
PH="$SANDBOX/plugin-home"
out="$(QUALIFIER_PLUGIN_HOME="$PH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
[ "$out" = "$PH/9.9.9/qualifier" ] || fail "expected install to $PH/9.9.9/qualifier, got $out"
[ "$("$out" --version)" = "qualifier 9.9.9" ] || fail "installed binary does not run"
[ "$(entries_of "$PH")" = "9.9.9" ] || fail "plugin home must hold only 9.9.9, has: $(entries_of "$PH")"
[ "$(entries_of "$PH/9.9.9")" = "qualifier qualifier.sha256" ] || fail "install must hold the binary and its hash, has: $(entries_of "$PH/9.9.9")"
[ "$(cat "$PH/9.9.9/qualifier.sha256")" = "$(sha256_file "$out")" ] || fail "the recorded hash does not match the installed binary"
ok "downloads, verifies, and installs the pinned release into the plugin home, recording the binary's sha256"

# D3. A second run reuses the install without calling curl.
rm -f "$CURL_MARKER"
out="$(QUALIFIER_PLUGIN_HOME="$PH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
[ "$out" = "$PH/9.9.9/qualifier" ] || fail "expected the installed binary, got $out"
[ ! -e "$CURL_MARKER" ] || fail "reuse must not call curl"
ok "reuses the managed install without calling curl"

# D3b. The reuse path also clears install leftovers older than an hour
#      (a crashed install's), keeping fresh ones and everything else.
mkdir -p "$PH/.staging.crashed" "$PH/.stale.crashed" "$PH/.staging.live"
touch -t 202001010000 "$PH/.staging.crashed" "$PH/.stale.crashed"
rm -f "$CURL_MARKER"
out="$(QUALIFIER_PLUGIN_HOME="$PH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
[ "$out" = "$PH/9.9.9/qualifier" ] || fail "reuse with leftovers: got $out"
[ ! -e "$CURL_MARKER" ] || fail "reuse with leftovers must not call curl"
[ "$(entries_of "$PH")" = ".staging.live 9.9.9" ] || fail "reuse cleanup left: $(entries_of "$PH")"
rm -rf "$PH/.staging.live"
ok "the reuse path removes install leftovers older than an hour, keeps fresh ones"

# D3c. A managed install whose binary no longer matches its recorded hash is
#      reinstalled, even when it reports the pinned version. The tampered
#      binary is never run: the hash is checked first. Also in exec mode.
PH_HASH="$SANDBOX/home-hash"
QUALIFIER_PLUGIN_HOME="$PH_HASH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" >/dev/null 2>&1 \
    || fail "hash tests: initial install failed"
TAMPER_MARKER="$SANDBOX/tampered-binary-ran"
tamper() {
    cat >"$PH_HASH/9.9.9/qualifier" <<EOF
#!/usr/bin/env bash
touch "$TAMPER_MARKER"
case "\${1:-}" in
    --version) echo "qualifier 9.9.9" ;;
    *) echo "TAMPERED" ;;
esac
EOF
    chmod +x "$PH_HASH/9.9.9/qualifier"
}
for mode in resolve exec; do
    tamper
    rm -f "$CURL_MARKER" "$TAMPER_MARKER"
    if [ "$mode" = exec ]; then
        out="$(QUALIFIER_PLUGIN_HOME="$PH_HASH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" exec threads 2>/dev/null)"
        [ "$out" = "$TARGET" ] || fail "tampered binary (exec): expected the reinstalled binary's output, got $out"
    else
        out="$(QUALIFIER_PLUGIN_HOME="$PH_HASH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
        [ "$out" = "$PH_HASH/9.9.9/qualifier" ] || fail "tampered binary: got $out"
    fi
    [ -e "$CURL_MARKER" ] || fail "tampered binary ($mode): expected a reinstall"
    [ ! -e "$TAMPER_MARKER" ] || fail "tampered binary ($mode): it must not be run"
    [ "$("$PH_HASH/9.9.9/qualifier" threads)" = "$TARGET" ] || fail "tampered binary ($mode): not replaced"
    [ "$(entries_of "$PH_HASH")" = "9.9.9" ] || fail "tampered binary ($mode) left: $(entries_of "$PH_HASH")"
done
ok "a binary that does not match its recorded hash is reinstalled without being run"

# D3d. A missing, empty, or garbled hash record makes the install invalid.
for record in missing empty garbled; do
    case "$record" in
        missing) rm -f "$PH_HASH/9.9.9/qualifier.sha256" ;;
        empty) : >"$PH_HASH/9.9.9/qualifier.sha256" ;;
        garbled) echo "not-a-hash" >"$PH_HASH/9.9.9/qualifier.sha256" ;;
    esac
    rm -f "$CURL_MARKER"
    out="$(QUALIFIER_PLUGIN_HOME="$PH_HASH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
    [ "$out" = "$PH_HASH/9.9.9/qualifier" ] || fail "$record hash record: got $out"
    [ -e "$CURL_MARKER" ] || fail "$record hash record: expected a reinstall"
    [ "$(cat "$PH_HASH/9.9.9/qualifier.sha256")" = "$(sha256_file "$out")" ] \
        || fail "$record hash record: the reinstall did not record the binary's hash"
done
ok "a missing, empty, or garbled hash record triggers a reinstall"

# D3e. A valid install with a matching record is reused with no download.
rm -f "$CURL_MARKER"
out="$(QUALIFIER_PLUGIN_HOME="$PH_HASH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" exec threads 2>/dev/null)"
[ "$out" = "$TARGET" ] || fail "valid install: got $out"
[ ! -e "$CURL_MARKER" ] || fail "a valid install with a matching hash record must not download"
ok "a valid install with a matching hash record is reused without a download"

# D4. exec mode on the managed install.
out="$(QUALIFIER_PLUGIN_HOME="$PH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" exec --version)"
[ "$out" = "qualifier 9.9.9" ] || fail "exec mode: got '$out'"
ok "exec mode runs the managed install"

# D5. A plugin update pinned to a newer version installs it and removes the
#     older version directory, but nothing that doesn't look like a
#     version (N.N.N, digits only), however old it is.
mkdir -p "$PH/notes" "$PH/1.2" "$PH/0.1.0-rc1"
touch "$PH/notes/keep" "$PH/readme.txt"
touch -t 202001010000 "$PH/notes" "$PH/readme.txt"
# Leftovers of an interrupted install: an hour-plus old staging and stale
# dir are removed, a fresh one (a concurrent install may own it) is kept.
mkdir -p "$PH/.staging.aged" "$PH/.stale.aged" "$PH/.staging.fresh"
touch -t 202001010000 "$PH/.staging.aged" "$PH/.stale.aged"
rm -f "$CURL_MARKER"
out="$(QUALIFIER_PLUGIN_HOME="$PH" PATH="$SAFE_PATH" "$SANDBOX/ensure-9910.sh" 2>/dev/null)"
[ "$out" = "$PH/9.9.10/qualifier" ] || fail "version bump: expected $PH/9.9.10/qualifier, got $out"
[ "$("$out" --version)" = "qualifier 9.9.10" ] || fail "version bump: new binary does not run"
[ -e "$CURL_MARKER" ] || fail "version bump must download the new release"
[ "$(entries_of "$PH")" = ".staging.fresh 0.1.0-rc1 1.2 9.9.10 notes readme.txt" ] \
    || fail "version bump left: $(entries_of "$PH")"
[ -e "$PH/notes/keep" ] || fail "version bump must leave non-version entries alone"
ok "a version bump installs the new release and removes only older version dirs"
ok "cleanup removes staging/stale leftovers older than an hour, keeps fresh ones"

# D5b. A wrapper pinned to an older version (e.g. another session still on
#      the previous plugin) installs its own but never removes a newer one.
rm -f "$CURL_MARKER"
out="$(QUALIFIER_PLUGIN_HOME="$PH" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
[ "$out" = "$PH/9.9.9/qualifier" ] || fail "older pin: expected $PH/9.9.9/qualifier, got $out"
[ -e "$CURL_MARKER" ] || fail "older pin must download its release"
[ "$("$PH/9.9.10/qualifier" --version)" = "qualifier 9.9.10" ] || fail "older pin removed the newer version dir"
[ "$(entries_of "$PH")" = ".staging.fresh 0.1.0-rc1 1.2 9.9.10 9.9.9 notes readme.txt" ] \
    || fail "older pin left: $(entries_of "$PH")"
ok "an older pin never removes a newer version dir"

# D5c. Versions compare numerically per component, not as strings.
PH_NUM="$SANDBOX/home-numeric"
make_fake_qualifier "$PH_NUM/9.9.9/qualifier" "9.9.9"
make_fake_qualifier "$PH_NUM/10.0.0/qualifier" "10.0.0"
make_fake_qualifier "$PH_NUM/9.10.0/qualifier" "9.10.0"
make_fake_qualifier "$PH_NUM/9.9.08/qualifier" "9.9.08"
# A component longer than 9 digits is unparseable, so never removed.
mkdir -p "$PH_NUM/0.0.0000000001"
out="$(QUALIFIER_PLUGIN_HOME="$PH_NUM" PATH="$SAFE_PATH" "$SANDBOX/ensure-9910.sh" 2>/dev/null)"
[ "$out" = "$PH_NUM/9.9.10/qualifier" ] || fail "numeric compare: got $out"
[ "$(entries_of "$PH_NUM")" = "0.0.0000000001 10.0.0 9.10.0 9.9.10" ] || fail "numeric compare left: $(entries_of "$PH_NUM")"
ok "version dirs compare numerically (9.9.9 and 9.9.08 < 9.9.10 < 9.10.0 < 10.0.0); over-long components are kept"

# D6. A managed dir holding a binary with the wrong version is replaced.
PH_WRONG="$SANDBOX/home-wrong"
make_fake_qualifier "$PH_WRONG/9.9.9/qualifier" "9.9.8"
rm -f "$CURL_MARKER"
out="$(QUALIFIER_PLUGIN_HOME="$PH_WRONG" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
[ "$out" = "$PH_WRONG/9.9.9/qualifier" ] || fail "wrong version: got $out"
[ "$("$out" --version)" = "qualifier 9.9.9" ] || fail "the wrong-version binary was not replaced"
[ -e "$CURL_MARKER" ] || fail "a wrong-version managed dir must trigger a download"
[ "$(entries_of "$PH_WRONG")" = "9.9.9" ] || fail "replacement left: $(entries_of "$PH_WRONG")"
ok "replaces a managed dir holding the wrong version"

# D7. A concurrent session that finishes installing first wins: its binary
#     is used and this run's download is discarded.
PH_RACE="$SANDBOX/home-race"
make_fake_qualifier "$SANDBOX/race-winner/qualifier" "9.9.9" qualifier "RACE-WINNER"
record_install_hash "$SANDBOX/race-winner"
out="$(CURL_RACE_DEST="$PH_RACE/9.9.9" CURL_RACE_SRC="$SANDBOX/race-winner/qualifier" \
    QUALIFIER_PLUGIN_HOME="$PH_RACE" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"
[ "$out" = "$PH_RACE/9.9.9/qualifier" ] || fail "race: got $out"
[ "$("$out" threads)" = "RACE-WINNER" ] || fail "race: the concurrent install must be kept"
[ "$(entries_of "$PH_RACE")" = "9.9.9" ] || fail "race left: $(entries_of "$PH_RACE")"
[ "$(entries_of "$PH_RACE/9.9.9")" = "qualifier qualifier.sha256" ] || fail "race: the discarded download leaked into 9.9.9: $(entries_of "$PH_RACE/9.9.9")"
ok "a concurrent install that lands first is used and this download discarded"

# An mv shim, first on the wrapper's PATH, that plays a concurrent session
# at the exact moment of one of the wrapper's renames:
#   dest-appears  just before the staging rename, the other session's valid
#                 install lands at MV_SHIM_DEST (so mv nests staging in it);
#   dest-taken    just before this run sets the invalid MV_SHIM_DEST aside,
#                 the other session has already moved it away.
MV_SHIM="$SANDBOX/mv-shim"
mkdir -p "$MV_SHIM"
cat >"$MV_SHIM/mv" <<'EOF'
#!/usr/bin/env bash
case "${MV_SHIM_MODE:-}" in
    dest-appears)
        case "$1" in
            */.staging.*)
                mkdir -p "$MV_SHIM_DEST"
                cp "$MV_SHIM_SRC" "$MV_SHIM_DEST/qualifier"
                cp "$MV_SHIM_SRC.sha256" "$MV_SHIM_DEST/qualifier.sha256" ;;
        esac ;;
    dest-taken)
        if [ "$1" = "$MV_SHIM_DEST" ]; then
            /bin/mv "$MV_SHIM_DEST" "$MV_SHIM_AWAY"
        fi ;;
esac
exec /bin/mv "$@"
EOF
chmod +x "$MV_SHIM/mv"

# D7b. Another install lands between the existence check and the staging
#      rename: mv nests staging inside it; the nested copy is removed and
#      the other install used.
PH_NEST="$SANDBOX/home-nest"
out="$(MV_SHIM_MODE=dest-appears MV_SHIM_DEST="$PH_NEST/9.9.9" MV_SHIM_SRC="$SANDBOX/race-winner/qualifier" \
    QUALIFIER_PLUGIN_HOME="$PH_NEST" PATH="$MV_SHIM:$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)" \
    || fail "nested rename: the wrapper must succeed"
[ "$out" = "$PH_NEST/9.9.9/qualifier" ] || fail "nested rename: got $out"
[ "$("$out" threads)" = "RACE-WINNER" ] || fail "nested rename: the other install must be kept"
[ "$(entries_of "$PH_NEST")" = "9.9.9" ] || fail "nested rename left: $(entries_of "$PH_NEST")"
[ "$(entries_of "$PH_NEST/9.9.9")" = "qualifier qualifier.sha256" ] || fail "nested rename leaked staging: $(entries_of "$PH_NEST/9.9.9")"
ok "a rename that nests staging inside a concurrent install is cleaned up"

# D7c. Two sessions replacing the same invalid install: the other one moves
#      it away first, so this run's set-aside finds nothing. Both outcomes
#      settle: exit 0, one valid install, no .stale.* left.
PH_TAKEN="$SANDBOX/home-taken"
make_fake_qualifier "$PH_TAKEN/9.9.9/qualifier" "9.9.8"
out="$(MV_SHIM_MODE=dest-taken MV_SHIM_DEST="$PH_TAKEN/9.9.9" MV_SHIM_AWAY="$SANDBOX/taken-by-other" \
    QUALIFIER_PLUGIN_HOME="$PH_TAKEN" PATH="$MV_SHIM:$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)" \
    || fail "concurrent replace: the wrapper must succeed"
[ -e "$SANDBOX/taken-by-other" ] || fail "concurrent replace: the shim did not run"
[ "$out" = "$PH_TAKEN/9.9.9/qualifier" ] || fail "concurrent replace: got $out"
[ "$("$out" --version)" = "qualifier 9.9.9" ] || fail "concurrent replace: no valid install"
[ "$(entries_of "$PH_TAKEN")" = "9.9.9" ] || fail "concurrent replace left: $(entries_of "$PH_TAKEN")"
ok "a concurrent replace of an invalid install settles on one valid install"

# D7c2. An invalid install that a concurrent session replaces with a valid
#       one after this run's first check, but before the set-aside: this
#       run re-checks, keeps the valid install (never moving or deleting it),
#       and discards its own download. A mktemp shim plays the other session
#       when the wrapper creates its .stale.* directory; the marker file it
#       leaves inside the install shows the directory was never replaced.
MKTEMP_SHIM="$SANDBOX/mktemp-shim"
mkdir -p "$MKTEMP_SHIM"
cat >"$MKTEMP_SHIM/mktemp" <<'EOF'
#!/usr/bin/env bash
for a in "$@"; do
    case "$a" in
        */.stale.*)
            cp "$MKTEMP_SHIM_SRC" "$MKTEMP_SHIM_DEST/qualifier"
            cp "$MKTEMP_SHIM_SRC.sha256" "$MKTEMP_SHIM_DEST/qualifier.sha256"
            touch "$MKTEMP_SHIM_DEST/other-session-marker" ;;
    esac
done
exec /usr/bin/mktemp "$@"
EOF
chmod +x "$MKTEMP_SHIM/mktemp"
PH_REVALID="$SANDBOX/home-revalid"
make_fake_qualifier "$PH_REVALID/9.9.9/qualifier" "9.9.8"
out="$(MKTEMP_SHIM_DEST="$PH_REVALID/9.9.9" MKTEMP_SHIM_SRC="$SANDBOX/race-winner/qualifier" \
    QUALIFIER_PLUGIN_HOME="$PH_REVALID" PATH="$MKTEMP_SHIM:$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)" \
    || fail "revalidated install: the wrapper must succeed"
[ -e "$PH_REVALID/9.9.9/other-session-marker" ] || fail "revalidated install: it was moved or deleted"
[ "$out" = "$PH_REVALID/9.9.9/qualifier" ] || fail "revalidated install: got $out"
[ "$("$out" threads)" = "RACE-WINNER" ] || fail "revalidated install: the concurrent install must be kept"
[ "$(entries_of "$PH_REVALID")" = "9.9.9" ] || fail "revalidated install left: $(entries_of "$PH_REVALID")"
ok "an install made valid by a concurrent session before the set-aside is kept, not moved"

# D7d. A cleanup failure (an old version dir that can't be deleted) never
#      fails a successful install.
PH_STUCK="$SANDBOX/home-stuck"
make_fake_qualifier "$PH_STUCK/9.9.8/locked/qualifier" "9.9.8"
chmod 555 "$PH_STUCK/9.9.8/locked"
if out="$(QUALIFIER_PLUGIN_HOME="$PH_STUCK" PATH="$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)"; then
    status=0
else
    status=$?
fi
chmod 755 "$PH_STUCK/9.9.8/locked"
[ "$status" -eq 0 ] || fail "cleanup failure: exit $status"
[ "$out" = "$PH_STUCK/9.9.9/qualifier" ] || fail "cleanup failure: got $out"
ok "a cleanup failure does not fail the install"

# D7e. This run's rename fails and a concurrent session's install lands a
#      moment later: the final re-check finds it instead of failing.
cat >"$MV_SHIM/mv-late" <<'EOF'
#!/usr/bin/env bash
case "$1" in
    */.staging.*)
        ( sleep 0.3; mkdir -p "$MV_SHIM_DEST"; cp "$MV_SHIM_SRC" "$MV_SHIM_DEST/qualifier"; cp "$MV_SHIM_SRC.sha256" "$MV_SHIM_DEST/qualifier.sha256" ) >/dev/null 2>&1 &
        exit 1 ;;
esac
exec /bin/mv "$@"
EOF
MV_LATE="$SANDBOX/mv-late"
mkdir -p "$MV_LATE"
mv "$MV_SHIM/mv-late" "$MV_LATE/mv"
chmod +x "$MV_LATE/mv"
PH_LATE="$SANDBOX/home-late"
out="$(MV_SHIM_DEST="$PH_LATE/9.9.9" MV_SHIM_SRC="$SANDBOX/race-winner/qualifier" \
    QUALIFIER_PLUGIN_HOME="$PH_LATE" PATH="$MV_LATE:$SAFE_PATH" "$SANDBOX/ensure-999.sh" 2>/dev/null)" \
    || fail "late concurrent install: the wrapper must succeed"
[ "$out" = "$PH_LATE/9.9.9/qualifier" ] || fail "late concurrent install: got $out"
[ "$("$out" threads)" = "RACE-WINNER" ] || fail "late concurrent install: the other install must be used"
[ "$(entries_of "$PH_LATE")" = "9.9.9" ] || fail "late concurrent install left: $(entries_of "$PH_LATE")"
ok "a concurrent install landing just after a failed rename is used"

# D8. No embedded checksum for this target: cargo fallback, nothing installed.
PH_FAIL="$SANDBOX/home-fail"
make_fake_qualifier "$PH_FAIL/9.9.8/qualifier" "9.9.8"
rm -f "$CURL_MARKER"
err="$(QUALIFIER_PLUGIN_HOME="$PH_FAIL" PATH="$SAFE_PATH" "$SANDBOX/ensure-unverified.sh" 2>&1 >/dev/null)" \
    && fail "an unverified target must not install"
case "$err" in *"cargo install qualifier --version 9.9.9"*) ;; *) fail "expected the cargo fallback, got: $err" ;; esac
case "$err" in *"no verified"*) ;; *) fail "expected a 'no verified' message, got: $err" ;; esac
[ ! -e "$CURL_MARKER" ] || fail "an unverified target must not download"
[ "$(entries_of "$PH_FAIL")" = "9.9.8" ] || fail "unverified target changed the plugin home: $(entries_of "$PH_FAIL")"
ok "refuses a target without an embedded checksum (cargo fallback)"

# D9. A checksum mismatch installs nothing and removes nothing.
err="$(QUALIFIER_PLUGIN_HOME="$PH_FAIL" PATH="$SAFE_PATH" "$SANDBOX/ensure-wrong-sha.sh" 2>&1 >/dev/null)" \
    && fail "a checksum mismatch must fail"
case "$err" in *"checksum mismatch"*) ;; *) fail "expected a checksum mismatch message, got: $err" ;; esac
[ "$(entries_of "$PH_FAIL")" = "9.9.8" ] || fail "checksum mismatch changed the plugin home: $(entries_of "$PH_FAIL")"
ok "checksum mismatch installs nothing and removes nothing"

# D10. A failed download installs nothing and removes nothing.
err="$(QUALIFIER_PLUGIN_HOME="$PH_FAIL" PATH="$NOACCESS:$INTERP:/usr/bin:/bin" "$SANDBOX/ensure-999.sh" 2>&1 >/dev/null)" \
    && fail "a failed download must fail"
case "$err" in *"cargo install qualifier --version 9.9.9"*) ;; *) fail "expected the cargo fallback, got: $err" ;; esac
[ "$(entries_of "$PH_FAIL")" = "9.9.8" ] || fail "failed download changed the plugin home: $(entries_of "$PH_FAIL")"
ok "a failed download installs nothing and removes nothing"

fi # host download tests

# --- SessionStart hook -----------------------------------------------------

run_hook() {
    # Runs the hook as Claude Code would, with a project directory. The
    # plugin root is captured before the cd below: with `cd ... && env
    # VAR="$PWD/..."`, bash expands the env command's arguments only after
    # `cd` has already run, so an inline `$PWD` there would resolve to the
    # project sandbox instead of the plugin directory.
    local project="$1" plugin_root="$PWD/$PLUGIN"; shift
    (cd "$project" && env CLAUDE_PROJECT_DIR="$project" CLAUDE_PLUGIN_ROOT="$plugin_root" "$@" "$HOOK" </dev/null)
}

context_of() {
    python3 -c 'import json,sys; print(json.load(sys.stdin)["hookSpecificOutput"]["additionalContext"])'
}

# Succeeds once no PID listed in the given files is running, within about
# 5s (a SIGKILLed process can take a moment to be reaped under load). A
# zombie counts as gone: in a container whose PID 1 does not reap, a killed
# orphan stays a zombie forever.
pids_gone() {
    local pid file state alive
    for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25; do
        alive=""
        for file in "$@"; do
            pid="$(cat "$file" 2>/dev/null)" || continue
            [ -n "$pid" ] || continue
            # -o stat= works with both BSD and procps ps; empty when gone.
            state="$(ps -o stat= -p "$pid" 2>/dev/null | tr -d ' ')"
            case "$state" in ""|Z*) ;; *) alive=1 ;; esac
        done
        [ -z "$alive" ] && return 0
        sleep 0.2
    done
    return 1
}

# The call line every hook context with a binary must give.
CALL_FORM="Run qualifier as \`\"$PWD/$PLUGIN/scripts/ensure-qualifier.sh\" exec <args>\`"

HOOK_PATH="$NOACCESS:$INTERP:/usr/bin:/bin"

# H1. Silent in a repository without .qual files.
NOQUAL="$SANDBOX/noqual"
mkdir -p "$NOQUAL/.git"
out="$(run_hook "$NOQUAL" QUALIFIER_BIN="$OVERRIDE/qualifier" PATH="$HOOK_PATH")"
[ -z "$out" ] || fail "hook must be silent without .qual files, got: $out"
ok "hook is silent in repositories without .qual files"

# H2. Injects the skill, the wrapper call line, and the summary in a
#     repository with .qual files.
WITHQUAL="$SANDBOX/withqual"
mkdir -p "$WITHQUAL/.git" "$WITHQUAL/src"
echo '{}' >"$WITHQUAL/src/.qual"
# Shims first on the hook's PATH, for the fast-path checks below:
#   sleep     records its PID and, only if it runs to the end, writes
#             sleep.expired (the summary budget elapsed);
#   cat, rm   record their process group and their parent's, so a
#             foreground command run in a group of its own (monitor mode
#             left on, which hands a terminal's foreground to that group)
#             shows up even without a terminal.
FASTPATH="$SANDBOX/fastpath"
mkdir -p "$FASTPATH/bin"
cat >"$FASTPATH/bin/sleep" <<EOF
#!/usr/bin/env bash
echo "\$\$" >"$FASTPATH/sleep.pid"
/bin/sleep "\$@"
touch "$FASTPATH/sleep.expired"
EOF
cat >"$FASTPATH/bin/cat" <<EOF
#!/usr/bin/env bash
echo "\$(ps -o pgid= -p \$\$ | tr -d ' ') \$(ps -o pgid= -p \$PPID | tr -d ' ')" >>"$FASTPATH/pgids"
exec /bin/\$(basename "\$0") "\$@"
EOF
chmod +x "$FASTPATH/bin/sleep" "$FASTPATH/bin/cat"
ln -s cat "$FASTPATH/bin/rm"
out="$(run_hook "$WITHQUAL" QUALIFIER_BIN="$OVERRIDE/qualifier" PATH="$FASTPATH/bin:$HOOK_PATH")"
ctx="$(printf '%s' "$out" | context_of)" || fail "hook output is not the expected JSON: $out"
[ -s "$FASTPATH/sleep.pid" ] || fail "the summary budget's sleep never started"
[ ! -e "$FASTPATH/sleep.expired" ] || fail "a fast summary made the hook wait out the budget"
pids_gone "$FASTPATH/sleep.pid" || fail "the summary budget's sleep outlived the hook"
# A leaked sleep would have run to the end by now (the budget is shorter
# than pids_gone's wait) and written the marker.
[ ! -e "$FASTPATH/sleep.expired" ] || fail "the summary budget's sleep was left running after the hook returned"
[ -s "$FASTPATH/pgids" ] || fail "the pgid shims never ran"
while read -r own parent; do
    [ "$own" = "$parent" ] || fail "a foreground command ran in its own process group ($own, parent $parent): monitor mode was left on"
done <"$FASTPATH/pgids"
ok "a fast summary returns before its budget, leaves no budget sleep, and runs no foreground job in its own group"
case "$ctx" in *"qual:recording-design-decisions"*) ;; *) fail "context lacks the using-qualifier map" ;; esac
case "$ctx" in *"1 blocker"*) ;; *) fail "context lacks the thread summary: $ctx" ;; esac
case "$ctx" in *"name: using-qualifier"*) fail "frontmatter must be stripped" ;; esac
case "$ctx" in *"$CALL_FORM"*) ;; *) fail "context must give the wrapper call line ($CALL_FORM): $ctx" ;; esac
[ "${#ctx}" -lt 10000 ] || fail "context is ${#ctx} chars; the harness caps it at 10000"
ok "hook injects using-qualifier, the wrapper call line, and the summary (${#ctx} chars)"

# H3. A pre-populated managed install is found without PATH or network.
rm -f "$CURL_MARKER"
out="$(run_hook "$WITHQUAL" QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$HOOK_PATH")"
ctx="$(printf '%s' "$out" | context_of)"
case "$ctx" in *"$CALL_FORM"*) ;; *) fail "context must give the wrapper call line: $ctx" ;; esac
case "$ctx" in *"1 blocker"*) ;; *) fail "context lacks the managed binary's summary: $ctx" ;; esac
[ ! -e "$CURL_MARKER" ] || fail "hook must not download when the managed install is valid"
ok "hook uses the managed install"

# H4. No binary and no network: still exit 0 with valid JSON.
out="$(run_hook "$WITHQUAL" QUALIFIER_PLUGIN_HOME="$SANDBOX/hook-empty-home" PATH="$HOOK_PATH")" \
    || fail "hook must exit 0 when the binary cannot be installed"
ctx="$(printf '%s' "$out" | context_of)" || fail "hook output is not valid JSON without a binary: $out"
case "$ctx" in *"cargo install qualifier --version $PINNED --locked"*) ;; *) fail "context must give the pinned install command: $ctx" ;; esac
case "$ctx" in *"could not install its pinned qualifier release"*"QUALIFIER_BIN"*) ;; *) fail "context must report the failed install and name QUALIFIER_BIN: $ctx" ;; esac
case "$ctx" in *"no prebuilt"*) fail "a supported platform must not be reported as lacking a prebuilt binary: $ctx" ;; esac
case "$ctx" in *"GitHub releases), ask the user to run \`cargo install"*) ;; *) fail "context must continue the sentence with 'ask the user to run': $ctx" ;; esac
ok "hook degrades gracefully without a binary or network"

# H4b. On a platform with no prebuilt release, the context says so and gives
#      the whole manual route: cargo install, then QUALIFIER_BIN (the wrapper
#      never looks on PATH, so the install alone would not be used).
out="$(run_hook "$WITHQUAL" QUALIFIER_PLUGIN_HOME="$SANDBOX/hook-unsupported-home" \
    FAKE_UNAME_S=Darwin FAKE_UNAME_M=x86_64 PATH="$UNAME_SHIM:$HOOK_PATH")" \
    || fail "hook must exit 0 on an unsupported platform"
ctx="$(printf '%s' "$out" | context_of)" || fail "hook output is not valid JSON on an unsupported platform: $out"
case "$ctx" in *"no prebuilt qualifier binary for this platform"*) ;; *) fail "context must say there is no prebuilt binary: $ctx" ;; esac
case "$ctx" in *"cargo install qualifier --version $PINNED --locked"*"QUALIFIER_BIN"*) ;; *) fail "context must give the cargo install and QUALIFIER_BIN steps: $ctx" ;; esac
case "$ctx" in *"not installed"*) fail "context must not call the platform's problem 'not installed': $ctx" ;; esac
case "$ctx" in *"pinned release. Ask the user to run \`cargo install"*) ;; *) fail "context must say 'Ask the user to run': $ctx" ;; esac
[ ! -e "$SANDBOX/hook-unsupported-home" ] || fail "hook on an unsupported platform must not create the plugin home"
ok "hook on an unsupported platform names the cargo install and QUALIFIER_BIN"

# H5. Works without VCS markers (project dir is the root).
NOVCS="$SANDBOX/novcs"
mkdir -p "$NOVCS"
echo '{}' >"$NOVCS/.qual"
out="$(run_hook "$NOVCS" QUALIFIER_BIN="$OVERRIDE/qualifier" PATH="$HOOK_PATH")"
printf '%s' "$out" | context_of >/dev/null || fail "hook must work outside a VCS"
ok "hook works outside a VCS"

# H6. Real git repositories: an untracked .qual is found; no .qual is silent.
GITQUAL="$SANDBOX/gitqual"
mkdir -p "$GITQUAL/src"
git -C "$GITQUAL" init -q
echo '{}' >"$GITQUAL/src/.qual"
out="$(run_hook "$GITQUAL" QUALIFIER_BIN="$OVERRIDE/qualifier" PATH="$HOOK_PATH")"
printf '%s' "$out" | context_of >/dev/null || fail "an untracked .qual in a git repo must be found"
GITNOQUAL="$SANDBOX/gitnoqual"
mkdir -p "$GITNOQUAL"
git -C "$GITNOQUAL" init -q
echo "x" >"$GITNOQUAL/a.txt"
out="$(run_hook "$GITNOQUAL" QUALIFIER_BIN="$OVERRIDE/qualifier" PATH="$HOOK_PATH")"
[ -z "$out" ] || fail "a git repo without .qual files must be silent, got: $out"
ok "hook gates git repositories through the index"

# H6b. A .qual file excluded by .gitignore must be silent: only the git
# index path (not a filesystem find fallback) is expected to honor it.
GITIGNORED="$SANDBOX/gitignored"
mkdir -p "$GITIGNORED/src"
git -C "$GITIGNORED" init -q
echo '*.qual' >"$GITIGNORED/.gitignore"
echo '{}' >"$GITIGNORED/src/.qual"
out="$(run_hook "$GITIGNORED" QUALIFIER_BIN="$OVERRIDE/qualifier" PATH="$HOOK_PATH")"
[ -z "$out" ] || fail "a gitignored .qual must be silent, got: $out"
ok "hook honors .gitignore via the git index path"

# H7. A qualifier on PATH (even the pinned version) is ignored, and the call
#     line is still the wrapper form.
out="$(run_hook "$WITHQUAL" QUALIFIER_PLUGIN_HOME="$MANAGED" PATH="$PATHQ:$HOOK_PATH")"
ctx="$(printf '%s' "$out" | context_of)"
case "$ctx" in *"PATH-QUALIFIER-SUMMARY"*) fail "the qualifier on PATH must not be run: $ctx" ;; esac
case "$ctx" in *"1 blocker"*) ;; *) fail "context lacks the managed binary's summary: $ctx" ;; esac
case "$ctx" in *"$CALL_FORM"*) ;; *) fail "context must give the wrapper call line: $ctx" ;; esac
# The backticks are a literal call-line quote, not command substitution.
# shellcheck disable=SC2016
case "$ctx" in *'Call it as `qualifier`'*) fail "context must not route calls to the qualifier on PATH: $ctx" ;; esac
ok "hook ignores a qualifier on PATH and gives the wrapper call line"

# H8. A slow summary is dropped instead of delaying the session, and
#     everything it started is killed. The fixture would run for a minute;
#     the hook's budget is two seconds, and the bound asserted here is
#     loose enough for a loaded machine while still catching a hook that
#     waits for the summary (or for a sleep-counting loop, H8b). The fixture
#     records its own PID and a child's, which must be gone afterwards.
SLOW="$SANDBOX/slow"
mkdir -p "$SLOW"
cat >"$SLOW/qualifier" <<EOF
#!/usr/bin/env bash
case "\${1:-}" in
    --version) echo "qualifier 9.9.9" ;;
    threads)
        echo "\$\$" >"$SLOW/summary.pid"
        /bin/sleep 60 &
        echo "\$!" >"$SLOW/child.pid"
        /bin/sleep 60
        echo "qualifier: too late" ;;
esac
EOF
chmod +x "$SLOW/qualifier"

run_slow_summary() {
    # $1 is the PATH to give the hook.
    rm -f "$SLOW/summary.pid" "$SLOW/child.pid"
    start="$(date +%s)"
    out="$(run_hook "$WITHQUAL" QUALIFIER_BIN="$SLOW/qualifier" PATH="$1")"
    elapsed=$(( $(date +%s) - start ))
    [ -s "$SLOW/summary.pid" ] || fail "the slow summary never started"
    ctx="$(printf '%s' "$out" | context_of)" || fail "hook output is not the expected JSON: $out"
    case "$ctx" in *"too late"*) fail "a late summary must be dropped" ;; esac
    if ! pids_gone "$SLOW/summary.pid" "$SLOW/child.pid"; then
        kill -KILL "$(cat "$SLOW/summary.pid")" "$(cat "$SLOW/child.pid")" 2>/dev/null
        fail "the slow summary or its child is still running after the hook returned"
    fi
}

run_slow_summary "$HOOK_PATH"
[ "$elapsed" -lt 15 ] || fail "hook waited ${elapsed}s for a slow summary"
ok "hook drops a summary that misses its budget (${elapsed}s) and kills everything it started"

# H8b. The budget is wall-clock time, not a count of polling sleeps: with a
#      `sleep` whose every start costs a second (as an exec can on a loaded
#      machine), the hook still returns in about budget + one second, where
#      counting ten 0.1s sleeps would take over ten.
SLOW_EXEC="$SANDBOX/slow-exec"
mkdir -p "$SLOW_EXEC"
cat >"$SLOW_EXEC/sleep" <<'EOF'
#!/usr/bin/env bash
/bin/sleep 1
exec /bin/sleep "$@"
EOF
chmod +x "$SLOW_EXEC/sleep"
run_slow_summary "$SLOW_EXEC:$HOOK_PATH"
[ "$elapsed" -lt 9 ] || fail "with slow process starts, the hook waited ${elapsed}s: its budget is not wall-clock time"
ok "the summary budget is wall-clock time even when every sleep starts slowly (${elapsed}s)"

# H9. A summary containing raw control characters (an ANSI color escape, a
# form feed) must not break the JSON: they are stripped before it is quoted.
CTRLCHARS="$SANDBOX/ctrlchars"
mkdir -p "$CTRLCHARS"
# The ${1:-} is a literal shell parameter expansion in the generated
# script, not one to expand here.
# shellcheck disable=SC2016
printf '#!/usr/bin/env bash\ncase "${1:-}" in\n    --version) echo "qualifier 9.9.9" ;;\n    threads) printf "\\033[31m1 blocker\\033[0m\\f\\n" ;;\nesac\n' \
    >"$CTRLCHARS/qualifier"
chmod +x "$CTRLCHARS/qualifier"
out="$(run_hook "$WITHQUAL" QUALIFIER_BIN="$CTRLCHARS/qualifier" PATH="$HOOK_PATH")"
ctx="$(printf '%s' "$out" | context_of)" || fail "control characters in the summary broke the JSON: $out"
case "$ctx" in *"1 blocker"*) ;; *) fail "context lost the summary text: $ctx" ;; esac
ok "hook strips control characters that would break the JSON"

# --- hooks.json: SessionStart matcher sources -------------------------------

python3 -c "
import json
data = json.load(open('$PLUGIN/hooks/hooks.json'))
matcher = data['hooks']['SessionStart'][0]['matcher']
sources = matcher.split('|')
missing = [s for s in ('startup', 'resume', 'clear', 'compact', 'fork') if s not in sources]
assert not missing, f'matcher {matcher!r} is missing {missing}'
" || fail "hooks.json SessionStart matcher must include startup, resume, clear, compact, fork"
ok "hooks.json SessionStart matcher includes startup, resume, clear, compact, fork"

# --- skills ----------------------------------------------------------------

python3 - "$PLUGIN" <<'PY' || fail "skill checks"
import glob, os, re, sys

plugin = sys.argv[1]
skills_dir = f"{plugin}/skills"
expected = {
    "using-qualifier", "recording-design-decisions", "planning-from-threads",
    "consulting-threads", "closing-the-loop", "reviewing-into-qualifier",
    "triaging-threads", "escalating-decisions", "handing-off-threads",
}
found = {d for d in os.listdir(skills_dir) if os.path.isdir(f"{skills_dir}/{d}")}
assert found == expected, f"skill set mismatch: missing {expected - found}, extra {found - expected}"

topics = {f[:-3] for f in os.listdir("src/cli/commands/agents/pages") if f.endswith(".md")}

for name in sorted(expected):
    path = f"{skills_dir}/{name}/SKILL.md"
    text = open(path).read()
    m = re.match(r"^---\n(.*?)\n---\n(.*)$", text, re.S)
    assert m, f"{path}: missing frontmatter"
    front, body = m.group(1), m.group(2)
    fields = dict(line.split(": ", 1) for line in front.splitlines() if ": " in line)
    assert fields.get("name") == name, f"{path}: name must be {name!r}"
    assert fields.get("description", "").startswith("Use "), f"{path}: description must start with 'Use '"
    tools = fields.get("allowed-tools", "")
    for rule in ("Bash(qualifier:*)", "Bash(${CLAUDE_PLUGIN_ROOT}/scripts/ensure-qualifier.sh exec:*)"):
        assert rule in tools, f"{path}: allowed-tools must include {rule}"
    for topic in re.findall(r"qualifier agents ([a-z_-]+)", body):
        assert topic in topics, f"{path}: cites missing agents topic {topic!r}"
    for ref in re.findall(r"qual:([a-z-]+)", body):
        assert ref in expected, f"{path}: references unknown skill qual:{ref}"
    for support in re.findall(r"`([a-z-]+-prompt\.md)`", body):
        assert os.path.exists(f"{skills_dir}/{name}/{support}"), f"{path}: missing {support}"
    # "Filling the brief" sections document placeholder tokens verbatim on
    # purpose; check the rest of the body for accidental leftover ones.
    body_outside_filling = re.sub(r"## Filling the brief\n.*?(\n## |\Z)", "", body, flags=re.S)
    assert not re.search(r"\bTBD\b|(?<!\$)\{[A-Z_ ]+\}", body_outside_filling), f"{path}: placeholder text"
    for prompt_path in sorted(glob.glob(f"{skills_dir}/{name}/*-prompt.md")):
        prompt_text = open(prompt_path).read()
        for topic in re.findall(r"qualifier agents ([a-z_-]+)", prompt_text):
            assert topic in topics, f"{prompt_path}: cites missing agents topic {topic!r}"
        for ref in re.findall(r"qual:([a-z-]+)", prompt_text):
            assert ref in expected, f"{prompt_path}: references unknown skill qual:{ref}"

bootstrap = open(f"{skills_dir}/using-qualifier/SKILL.md").read()
assert len(bootstrap) < 8000, f"using-qualifier is {len(bootstrap)} chars; keep it under 8000 (hook context cap)"
for name in expected - {"using-qualifier"}:
    assert f"qual:{name}" in bootstrap, f"using-qualifier must map qual:{name}"
PY
ok "skills: frontmatter, cited topics, cross-references, supporting files, size"

# The plugin no longer consults PATH, installs to ~/.local/bin, or has a
# minimum version; nothing under plugins/ may still say so. .qual files hold
# review-finding prose (data, not plugin statements), so they're excluded.
if stale="$(grep -rnE --exclude='.qual' 'MIN_VERSION|min-version|QUALIFIER_INSTALL_DIR|\.local/bin|not on PATH' "$PLUGIN")"; then
    fail "stale PATH/install statements under $PLUGIN:
$stale"
fi
ok "no stale PATH, ~/.local/bin, or minimum-version statements under $PLUGIN"

# --- subagent prompts: placeholders, not bare `qualifier` ------------------
# Subagent prompt files never assume `qualifier` is on PATH (a subagent gets
# no wrapper instruction), so every invocation must use the `{QUALIFIER}`
# placeholder, filled in by the dispatching skill before dispatch.

python3 - "$PLUGIN" <<'PY' || fail "bare qualifier invocations in prompt files"
import glob, re, sys

plugin = sys.argv[1]
subcommands = (
    "record|reply|resolve|emit|show|ls|threads|praise|review|diff|compact"
    "|agents|init"
)
# May flag prose that names a subcommand right after "qualifier" (e.g.
# "qualifier review results"); reword such prose rather than loosening this.
pattern = re.compile(r"\bqualifier\s+(?:" + subcommands + r")\b", re.IGNORECASE)

bad = []
for path in sorted(glob.glob(f"{plugin}/skills/*/*-prompt.md")):
    text = open(path).read()
    for m in pattern.finditer(text):
        bad.append(f"{path}: {m.group(0)!r}")
assert not bad, "bare qualifier invocations (use {QUALIFIER} instead):\n" + "\n".join(bad)
PY
ok "no bare qualifier invocations in *-prompt.md files"

# Every {PLACEHOLDER} a prompt uses is documented in its dispatching skill's
# "Filling the brief" section, and every placeholder documented there is used
# by at least one of that skill's prompts.
python3 - "$PLUGIN" <<'PY' || fail "prompt placeholder documentation"
import glob, re, sys

plugin = sys.argv[1]
skills_dir = f"{plugin}/skills"

# Explicit prompt -> dispatching skill map. Every *-prompt.md must be in it.
dispatch = {
    "reviewing-into-qualifier": ["reviewer-prompt.md", "verifier-prompt.md"],
    "triaging-threads": ["triager-prompt.md"],
}
mapped = {f"{skills_dir}/{skill}/{prompt}" for skill, prompts in dispatch.items() for prompt in prompts}
on_disk = set(glob.glob(f"{skills_dir}/*/*-prompt.md"))
assert on_disk == mapped, (
    f"prompt files missing from the dispatch map: {sorted(on_disk - mapped)}; "
    f"mapped but missing on disk: {sorted(mapped - on_disk)}"
)

placeholder_re = re.compile(r"\{[A-Z][A-Z_ a-z]*\}")

for skill, prompts in dispatch.items():
    skill_path = f"{skills_dir}/{skill}/SKILL.md"
    skill_text = open(skill_path).read()
    m = re.search(r"## Filling the brief\n(.*?)(\n## |\Z)", skill_text, re.S)
    assert m, f"{skill_path}: missing a 'Filling the brief' section"
    documented = set(placeholder_re.findall(m.group(1)))

    used = set()
    for prompt in prompts:
        prompt_path = f"{skills_dir}/{skill}/{prompt}"
        text = open(prompt_path).read()
        found = set(placeholder_re.findall(text))
        used |= found
        undocumented = found - documented
        assert not undocumented, f"{prompt_path}: undocumented placeholders {undocumented}"

    unused = documented - used
    assert not unused, f"{skill_path}: documents unused placeholders {unused}"
PY
ok "every *-prompt.md is mapped to its dispatching skill; placeholders documented both ways"

# --- skill examples against the real binary ---------------------------------
# Every qualifier command and record line in the skills and subagent briefs
# runs against a real qualifier in a fixture repository, so no example can
# use a flag, key, line shape, or subcommand the CLI rejects.
status=0
QUALIFIER_BIN="$EXAMPLES_BIN" python3 scripts/check-skill-examples.py || status=$?
case "$status" in
    0) ok "skill and brief examples run cleanly against the real qualifier" ;;
    2) ;; # the checker printed its own skip line
    *) fail "skill examples" ;;
esac

# --- closing-the-loop: every non-fresh `qualifier review` status ----------
# `qualifier review` reports `drifted` and `missing` (file gone, or span past
# the end of the file); the drift step must say what to do with each.

python3 - "$PLUGIN" <<'PY' || fail "closing-the-loop drift step"
import re, sys

path = f"{sys.argv[1]}/skills/closing-the-loop/SKILL.md"
text = open(path).read()
m = re.search(r"\n3\. \*\*Drift\.\*\*(.*?)\n4\. ", text, re.S)
assert m, f"{path}: missing step 3 (Drift)"
step = " ".join(m.group(1).split())
for needle in ("`drifted`", "`missing`", "--supersedes", "--reason obsolete",
               "close authority", "cross-subject"):
    assert needle in step, f"{path}: step 3 must mention {needle}"
PY
ok "closing-the-loop step 3 covers drifted and missing review results"

# --- eval cases: structure and graders ----------------------------------------
# Every case under evals/ (except the shared _fixture) must load in
# `claude plugin eval`, which rejects unknown keys. Key sets are the ones
# https://code.claude.com/docs/en/plugin-evals.md documents (case.yaml fields,
# prompt.md fields, grader frontmatter and grader types). Graders use
# JavaScript regexes over the JSON-encoded tool input; the patterns here are
# also valid Python regexes with the same meaning. Each line the script
# prints is one passed check.

python3 - "$PLUGIN" "$GRADER_BIN" "$SANDBOX" >"$SANDBOX/eval-checks" <<'PY' || fail "eval cases and graders"
import json, os, re, subprocess, sys

plugin, qbin, sandbox = sys.argv[1], sys.argv[2], sys.argv[3]
evals = f"{plugin}/evals"

CASE_KEYS = {"schema_version", "name", "description", "tags", "plugins", "runs",
             "expected_outcome", "context", "execution", "graders"}
CASE_NESTED = {
    "context": {"scaffold_script", "history_file", "add_dirs"},
    "execution": {"prompt", "model", "max_turns", "timeout_seconds", "allowed_tools",
                  "append_system_prompt", "env"},
}
PROMPT_KEYS = {"schema_version", "name", "description", "tags", "plugins", "runs",
               "expected_outcome", "model", "max_turns", "timeout_seconds", "allowed_tools",
               "append_system_prompt", "env"}
GRADER_COMMON = {"type", "weight", "arm"}
GRADER_TYPES = {  # type -> (options, required options)
    "regex": ({"pattern", "flags", "match", "target"}, {"pattern"}),
    "tool_used": ({"tool", "input_match", "min", "max"}, {"tool"}),
    "tool_order": ({"before", "after"}, {"before", "after"}),
    "file_exists": ({"path", "exists"}, {"path"}),
    "llm": ({"criteria", "focus"}, set()),
    "baseline": ({"baseline_file", "criteria"}, {"baseline_file"}),
}
TARGETS = {"last_message", "trace", "files", "mock_calls"}
REGEX_KEYS = ("pattern", "input_match")

def scalar(raw):
    """(value, quoted) for a flow scalar; lists and mappings stay raw."""
    raw = raw.strip()
    if len(raw) >= 2 and raw[0] == raw[-1] == "'":
        return raw[1:-1].replace("''", "'"), True
    if len(raw) >= 2 and raw[0] == raw[-1] == '"':
        return raw[1:-1].encode().decode("unicode_escape"), True
    return raw, False

def read_yaml(path):
    """A minimal reader: `key: scalar` lines, and one level of nested
    `key:` blocks indented by two spaces. Anything else is an error, so an
    unsupported construct fails loudly instead of passing unread."""
    top, current = {}, None
    for n, line in enumerate(open(path).read().splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        m = re.fullmatch(r"( *)([A-Za-z_][A-Za-z0-9_]*):(?: (.*))?", line)
        assert m, f"{path}:{n}: unsupported YAML for this checker's reader: {line!r}"
        indent, key, value = len(m.group(1)), m.group(2), m.group(3)
        if indent == 0:
            assert key not in top, f"{path}:{n}: duplicate key {key!r}"
            if value is None or not value.strip():
                top[key], current = {}, key
            else:
                top[key], current = scalar(value), None
        else:
            assert indent == 2 and current, f"{path}:{n}: unsupported nesting: {line!r}"
            assert value and value.strip(), f"{path}:{n}: unsupported nesting: {line!r}"
            top[current][key] = scalar(value)
    return top

def frontmatter(path):
    text = open(path).read()
    m = re.match(r"^---\n(.*?)\n---\n(.*)$", text, re.S)
    assert m, f"{path}: missing frontmatter"
    fields = {}
    for line in m.group(1).splitlines():
        km = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*):(?: (.*))?", line)
        assert km and km.group(2), f"{path}: unsupported frontmatter line {line!r}"
        assert km.group(1) not in fields, f"{path}: duplicate key {km.group(1)!r}"
        fields[km.group(1)] = scalar(km.group(2))
    return fields, m.group(2)

def file_target(where, raw):
    """Validates the `{ source: file, path: <path> }` target form: exactly
    those two keys, source `file`, and one workspace-relative file path (no
    glob, no `..`). Returns the path."""
    assert raw.startswith("{") and raw.endswith("}"), (
        f"{where}: {raw!r} is not one of {sorted(TARGETS)} or {{ source: file, path: <path> }}")
    entries = {}
    for item in raw[1:-1].split(","):
        km = re.fullmatch(r"\s*([a-z_]+):\s*(\S.*?)\s*", item)
        assert km, f"{where}: cannot read {item!r} in {raw!r}"
        assert km.group(1) not in entries, f"{where}: duplicate key {km.group(1)!r}"
        entries[km.group(1)] = scalar(km.group(2))[0]
    assert set(entries) == {"source", "path"}, (
        f"{where}: a file target takes exactly source and path, got {sorted(entries)}")
    assert entries["source"] == "file", f"{where}: source must be file, got {entries['source']!r}"
    path = entries["path"]
    assert not path.startswith("/") and ".." not in path.split("/"), (
        f"{where}: path must be relative to the workspace, got {path!r}")
    assert not re.search(r"[*?\[\]{}]", path), f"{where}: path is one file, not a glob: {path!r}"
    return path

def check_regex(where, pattern):
    # The runner uses JavaScript regexes; Python's `re` compiles the subset
    # these graders use with the same meaning. Syntax the runner rejects is
    # refused here, and JavaScript-only syntax Python can't compile (e.g.
    # `(?<name>...)` groups, variable-width lookbehind, `\u{...}`) fails
    # too, so keep graders to the common subset.
    assert not re.search(r"\(\?[aiLmsux-]+[:)]", pattern), (
        f"{where}: inline flags like (?i) aren't supported by the eval runner; use `flags`")
    assert not re.search(r"\(\?P[<=>]|\\[AZ]", pattern), (
        f"{where}: Python-only regex syntax the eval runner (JavaScript) rejects")
    try:
        re.compile(pattern)
    except re.error as e:
        raise AssertionError(f"{where}: regex does not compile: {e}: {pattern!r}")

cases = sorted(d for d in os.listdir(evals) if os.path.isdir(f"{evals}/{d}") and d != "_fixture")
assert cases, "no eval cases found"
for case in cases:
    d = f"{evals}/{case}"
    assert os.path.isfile(f"{d}/case.yaml"), f"{d}: missing case.yaml"
    y = read_yaml(f"{d}/case.yaml")
    assert set(y) <= CASE_KEYS, f"{d}/case.yaml: unknown keys {sorted(set(y) - CASE_KEYS)}"
    assert y.get("schema_version") == ("1.1", True), (
        f"{d}/case.yaml: schema_version must be the string \"1.1\", got {y.get('schema_version')}")
    assert y.get("name", (None,))[0] == case, f"{d}/case.yaml: name must be {case!r}"
    for key, allowed in CASE_NESTED.items():
        if key in y:
            assert isinstance(y[key], dict), f"{d}/case.yaml: {key} must be a mapping"
            assert set(y[key]) <= allowed, (
                f"{d}/case.yaml: unknown {key} keys {sorted(set(y[key]) - allowed)}")
    scaffold = y.get("context", {}).get("scaffold_script")
    if scaffold:
        assert os.path.isfile(os.path.join(d, scaffold[0])), f"{d}/case.yaml: no scaffold {scaffold[0]}"

    if os.path.exists(f"{d}/prompt.md"):
        fields, body = frontmatter(f"{d}/prompt.md")
        assert set(fields) <= PROMPT_KEYS, f"{d}/prompt.md: unknown keys {sorted(set(fields) - PROMPT_KEYS)}"
        if "schema_version" in fields:
            assert fields["schema_version"] == ("1.1", True), f"{d}/prompt.md: schema_version must be \"1.1\""
        if "name" in fields:
            assert fields["name"][0] == case, f"{d}/prompt.md: name must be {case!r}"
        assert body.strip(), f"{d}/prompt.md: empty prompt"
    else:
        assert "prompt" in y.get("execution", {}), f"{d}: no prompt.md and no execution.prompt"

    graders = sorted(f for f in os.listdir(f"{d}/graders") if f.endswith(".md")) \
        if os.path.isdir(f"{d}/graders") else []
    assert graders or "graders" in y, f"{d}: no graders"
    for g in graders:
        where = f"{d}/graders/{g}"
        fields, body = frontmatter(where)
        kind = fields.get("type", (None,))[0]
        assert kind in GRADER_TYPES, f"{where}: type {kind!r} is not one of {sorted(GRADER_TYPES)}"
        options, required = GRADER_TYPES[kind]
        unknown = set(fields) - GRADER_COMMON - options
        assert not unknown, f"{where}: keys {sorted(unknown)} are not valid for a {kind} grader"
        missing = required - set(fields)
        assert not missing, f"{where}: a {kind} grader needs {sorted(missing)}"
        if kind == "llm":
            assert "criteria" in fields or body.strip(), f"{where}: an llm grader needs criteria"
        if "arm" in fields:
            assert fields["arm"][0] in ("with-only", "both"), f"{where}: arm must be with-only or both"
        if "weight" in fields:
            assert float(fields["weight"][0]) > 0, f"{where}: weight must be positive"
        for key in ("min", "max"):
            if key in fields:
                assert fields[key][0].isdigit(), f"{where}: {key} must be a non-negative integer"
        if "min" in fields and "max" in fields:
            assert int(fields["min"][0]) <= int(fields["max"][0]), f"{where}: min > max"
        if "flags" in fields:
            assert re.fullmatch(r"[dgimsuvy]+", fields["flags"][0]), f"{where}: invalid regex flags"
        if "match" in fields:
            assert re.fullmatch(r"not_contains|count:\d+", fields["match"][0]), (
                f"{where}: match must be not_contains or count:N")
        # `focus` (llm) takes the same values as `target` (regex).
        for key in ("target", "focus"):
            if key in fields:
                t = fields[key][0]
                if t not in TARGETS:
                    file_target(f"{where} ({key})", t)
        for key in REGEX_KEYS:
            if key in fields:
                check_regex(f"{where} ({key})", fields[key][0])
        # tool_order's before/after: a tool name, or { tool, input_match }.
        for key in ("before", "after"):
            if key in fields:
                v = fields[key][0]
                if re.fullmatch(r"[A-Za-z_][\w:*.-]*", v):
                    continue
                m = re.fullmatch(r"\{\s*tool:\s*([A-Za-z_][\w:*.-]*)\s*(?:,\s*input_match:\s*(.+?))?\s*\}", v)
                assert m, f"{where}: {key} must be a tool name or {{ tool, input_match }}, got {v!r}"
                if m.group(2):
                    check_regex(f"{where} ({key}.input_match)", scalar(m.group(2))[0])
print("eval cases: case.yaml, prompt.md, and grader keys are documented; every grader regex compiles")

def values(path):
    """A grader's frontmatter as plain strings."""
    return {k: v for k, (v, _) in frontmatter(path)[0].items()}

skills = sorted(d for d in os.listdir(f"{plugin}/skills") if os.path.isdir(f"{plugin}/skills/{d}"))

def graders(case):
    d = f"{evals}/{case}/graders"
    return {f: values(f"{d}/{f}") for f in sorted(os.listdir(d)) if f.endswith(".md")}

# The no-skill-fired grader catches every qual skill, prefixed or bare.
g = values(f"{evals}/no-qual-files/graders/no-skill-fired.md")
assert g["type"] == "tool_used" and g["tool"] == "Skill", "no-skill-fired must be a Skill tool_used grader"
assert (g.get("min"), g.get("max"), g.get("arm")) == ("0", "0", "both"), "no-skill-fired must be min 0, max 0, arm both"
pat = re.compile(g["input_match"])
for name in skills:
    for form in (f"qual:{name}", name):
        for call in ({"skill": form}, {"skill": form, "args": "src/net.rs"}):
            encoded = json.dumps(call)
            assert pat.search(encoded), f"no-skill-fired misses {encoded}"
for other in ("superpowers:brainstorming", "code-review", "other:closing-the-loop", "closing-the-loop-extra"):
    encoded = json.dumps({"skill": other})
    assert not pat.search(encoded), f"no-skill-fired must not match {encoded}"

# Every case with a no-record grader also forbids Write and Edit calls on a
# .qual file, in the same negation form.
write_inputs = {
    "Write": lambda p: {"file_path": p, "content": '{"metabox":"1"}\n'},
    "Edit": lambda p: {"file_path": p, "old_string": "a", "new_string": "b", "replace_all": False},
}
qual_paths = ["/tmp/repo/src/.qual", "src/.qual", "/tmp/repo/docs/cache-design.md.qual", ".qual"]
other_paths = ["/tmp/repo/docs/cache-design.md", "/tmp/repo/.qualifier", "/tmp/repo/src/.qual.bak",
               "/tmp/repo/.qual/notes.md"]
cases = [c for c in sorted(os.listdir(evals)) if os.path.isdir(f"{evals}/{c}/graders")]
with_no_record = [c for c in cases if "no-record.md" in graders(c)]
assert {"quiet-typo", "quiet-question", "quiet-explore"} <= set(with_no_record), with_no_record
for case in with_no_record:
    gs = graders(case)
    base = gs["no-record.md"]
    for tool, make in write_inputs.items():
        found = [f for f, g in gs.items() if g.get("type") == "tool_used" and g.get("tool") == tool]
        assert len(found) == 1, f"{case}: expected one {tool} grader, found {found}"
        g = gs[found[0]]
        assert (g.get("min"), g.get("max")) == ("0", "0"), f"{case}/{found[0]}: must be min 0, max 0"
        assert g.get("arm") == base.get("arm"), f"{case}/{found[0]}: arm must match no-record.md"
        pat = re.compile(g["input_match"])
        for p in qual_paths:
            assert pat.search(json.dumps(make(p))), f"{case}/{found[0]} misses {tool} on {p}"
        for p in other_paths:
            assert not pat.search(json.dumps(make(p))), f"{case}/{found[0]} must not match {tool} on {p}"
print("eval graders: no-skill-fired matches every qual skill; no-record cases also forbid .qual writes")

# Bash graders match the JSON-encoded tool input, which also carries the
# call's free-text `description`, so each is anchored inside the `command`
# string and names real subcommands. The plugin keeps its qualifier binary
# off PATH (scripts/ensure-qualifier.sh): no-bare-qualifier catches a bare
# `qualifier <sub>` (which would fail outright) but not the wrapper form;
# no-record and wrote-record accept either form. Every copy of a grader is
# identical, and its subcommand list is checked against `qualifier --help`
# so it can't drift from the CLI.
WRAPPER = '"/plugin/scripts/ensure-qualifier.sh"'

def bash_call(command, description=None, description_first=False):
    call = {"command": command}
    if description is not None:
        call = {"description": description, **call} if description_first else {**call, "description": description}
    return json.dumps(call)

def copies(name):
    found = {c: f"{evals}/{c}/graders/{name}" for c in cases if os.path.isfile(f"{evals}/{c}/graders/{name}")}
    texts = {open(p).read() for p in found.values()}
    assert found and len(texts) == 1, f"every {name} must be identical: {sorted(found)}"
    g = values(next(iter(found.values())))
    assert g["type"] == "tool_used" and g["tool"] == "Bash", f"{name}: must be a Bash tool_used grader"
    return g, sorted(found)

def subcommands_of(g, name):
    m = re.search(r"\\s\+\(\?:([a-z|]+)\)\\b$", g["input_match"])
    assert m, f"{name}: must end in a (?:sub|sub...)\\b subcommand group"
    return set(m.group(1).split("|"))

def check_calls(name, pat, hits, misses):
    for call in hits:
        assert pat.search(call), f"{name} misses {call}"
    for call in misses:
        assert not pat.search(call), f"{name} wrongly matches {call}"

# Calls that name qualifier subcommands only outside the command string,
# or run something that is not a qualifier subcommand.
DESCRIPTION_ONLY = [
    bash_call(f"{WRAPPER} exec threads --format json", "List qualifier threads for this review"),
    bash_call("ls src", "Read qualifier records in src/.qual"),
    bash_call("git status", "qualifier record check before commit"),
    bash_call("ls src", "Run qualifier reply on the blocker", description_first=True),
    bash_call("cargo install qualifier --version 0.8.0 --locked"),
    bash_call("grep -rn 'verified' src/.qual"),
]

no_bare, no_bare_cases = copies("no-bare-qualifier.md")
assert (no_bare.get("min"), no_bare.get("max")) == ("0", "0"), "no-bare-qualifier must be min 0, max 0"
no_record, no_record_cases = copies("no-record.md")
wrote, wrote_cases = copies("wrote-record.md")

if not qbin:
    print("skip: no qualifier binary; Bash grader subcommand and fixture checks skipped")
else:
    top = subprocess.run([qbin, "--help"], capture_output=True, text=True, check=True).stdout
    all_subs = set(re.findall(r"^  ([a-z][a-z-]*)\s{2,}\S", top, re.M))
    section = re.search(r"^Record observations:\n((?:  .*\n)+)", top, re.M)
    assert section, "`qualifier --help` has no 'Record observations:' section"
    write_subs = set(re.findall(r"^  ([a-z][a-z-]*)\s{2,}", section.group(1), re.M))
    assert {"record", "reply", "resolve", "threads"} <= all_subs, all_subs
    assert subcommands_of(no_bare, "no-bare-qualifier") == all_subs, (
        f"no-bare-qualifier subcommands must be `qualifier --help`'s: {sorted(all_subs)}")
    assert subcommands_of(no_record, "no-record") == write_subs, (
        f"no-record subcommands must be the 'Record observations' ones: {sorted(write_subs)}")
    assert subcommands_of(wrote, "wrote-record") == write_subs - {"emit"}, (
        f"wrote-record subcommands must be {sorted(write_subs - {'emit'})}")

    # Escaped quotes earlier in the command must not stop the match.
    QUOTED = 'git commit -m "fix: \\"retry\\" budget" && '
    pat = re.compile(no_bare["input_match"])
    check_calls("no-bare-qualifier", pat,
        [bash_call(f"qualifier {sub} --help") for sub in sorted(all_subs)] + [
            bash_call('qualifier record blocker src/net.rs:1 "msg"'),
            bash_call("cd /tmp/repo && qualifier threads --all"),
            bash_call(QUOTED + 'qualifier reply 1a2b3c4d "ok"'),
            bash_call('qualifier record concern src/net.rs:1 "a \\"quoted\\" word"', "Record a finding"),
            # A call starting a line of a multi-line command follows a JSON `\n`.
            bash_call("cd /tmp/repo\nqualifier threads --all"),
            bash_call("cd /tmp/repo\n\tqualifier record blocker src/net.rs:1 \"msg\""),
        ],
        DESCRIPTION_ONLY + [
            bash_call(f'{WRAPPER} exec record blocker src/net.rs:1 "msg"'),
            bash_call(f"{WRAPPER} pinned-version"),
            bash_call(QUOTED + f"{WRAPPER} exec threads"),
            bash_call("/opt/bin/qualifier threads"),
            bash_call(f"cd /tmp/repo\n{WRAPPER} exec record blocker src/net.rs:1 \"msg\""),
            bash_call("cd /tmp/repo\nls qualifier-notes"),
        ])
    for name, g, subs in (("no-record", no_record, write_subs), ("wrote-record", wrote, write_subs - {"emit"})):
        pat = re.compile(g["input_match"])
        check_calls(name, pat,
            [bash_call(f'qualifier {sub} src/net.rs "m"') for sub in sorted(subs)]
            + [bash_call(f'{WRAPPER} exec {sub} src/net.rs "m"') for sub in sorted(subs)] + [
                bash_call(QUOTED + f'{WRAPPER} exec record concern src/net.rs:1 "m"', "Record it"),
                bash_call(QUOTED + 'qualifier reply 1a2b3c4d "ok"'),
                bash_call("/repo/target/debug/qualifier record --stdin < /tmp/x/batch.jsonl"),
                bash_call('cd /tmp/repo\nqualifier reply 1a2b3c4d "ok"'),
                bash_call(f'cd /tmp/repo\n{WRAPPER} exec record concern src/net.rs:1 "m"'),
            ],
            DESCRIPTION_ONLY + [
                bash_call(f"{WRAPPER} exec threads --format json", "Record qualifier reply targets"),
                bash_call("qualifier threads --all"),
                bash_call("qualifier records"),
                bash_call(f"{WRAPPER} exec agents batch"),
            ])
    print(f"eval graders: Bash graders match only the command, and name `qualifier --help`'s "
          f"subcommands (no-bare-qualifier: {len(no_bare_cases)}, no-record: {len(no_record_cases)}, "
          f"wrote-record: {len(wrote_cases)} cases)")

# verified-tag grades the file the verifier's replies land in, not the
# trace (which also holds the loaded skill and the filled verifier brief,
# both quoting the verdict tags). Scaffold the fixture, then write replies
# the way verifier-prompt.md documents; the grader must match the file only
# after they land.
g = values(f"{evals}/review-subsystems/graders/verified-tag.md")
assert g["type"] == "regex" and g.get("match", "contains") == "contains", g
target = file_target("verified-tag", g.get("target", ""))
pat = re.compile(g["pattern"])
for text in ("verified:maybe", '"verified:maybe"', "verified:confirmed"):
    assert not pat.search(text), f"verified-tag must not match {text!r}"
brief = open(f"{plugin}/skills/reviewing-into-qualifier/verifier-prompt.md").read()
templates = re.findall(r"^`(\{.*\})`$", brief, re.M)
verdicts = sorted(t for line in templates for t in json.loads(line).get("tags", []))
assert verdicts == ["verified:confirmed", "verified:downgraded", "verified:refuted"], verdicts
if qbin:
    work = f"{sandbox}/verified-tag-fixture"
    os.makedirs(work)
    env = {**os.environ, "QUALIFIER_BIN": qbin, "GIT_CONFIG_GLOBAL": os.devnull}
    scaffold = os.path.abspath(f"{evals}/_fixture/scaffold.sh")
    subprocess.run(["bash", scaffold], cwd=work, env=env, check=True, capture_output=True)
    def q(*args, stdin=None):
        return subprocess.run([qbin, *args], cwd=work, env=env, input=stdin, check=True,
                              capture_output=True, text=True).stdout
    for i, path in enumerate(("src/net.rs", "src/auth.rs", "src/cache.rs")):
        q("record", "concern", f"{path}:1", f"finding {i}", "--tag", "review", "--tag", "review:fixture")
    graded = f"{work}/{target}"
    assert os.path.isfile(graded), f"verified-tag reads {target}, which the fixture does not have"
    assert not pat.search(open(graded).read()), "verified-tag matches the fixture before any verdict"
    roots = [t["root"] for t in json.loads(q("threads", "--tag", "review:fixture", "--format", "json"))]
    assert len(roots) == len(templates) == 3, (roots, templates)
    for n, (root, line) in enumerate(zip(roots, templates), 1):
        reply = json.loads(line.replace("<root.subject>", root["subject"]).replace("<root.id>", root["id"]))
        reply = {k: (re.sub(r"<[^<>]+>", "sample", v) if isinstance(v, str) else v) for k, v in reply.items()}
        q("record", "--stdin", stdin=json.dumps(reply) + "\n")
        assert len(pat.findall(open(graded).read())) == n, (
            f"verified-tag misses a {reply['tags'][0]} reply in {target}")
    print(f"review-subsystems: verified-tag grades {target}, where the fixture's verdict replies land")
else:
    print("skip: no qualifier binary; verified-tag fixture check skipped")
PY
while IFS= read -r msg; do
    case "$msg" in
        skip:*) echo "$msg" ;;
        *) ok "$msg" ;;
    esac
done <"$SANDBOX/eval-checks"

# Nothing above may have written to ~/.local/bin.
[ ! -e "$HOME/.local/bin" ] || fail "the wrapper wrote to ~/.local/bin"
ok "nothing written to ~/.local/bin"

echo "test-plugin: $PASS checks passed"
