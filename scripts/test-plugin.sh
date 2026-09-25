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

# --- SessionStart hook -----------------------------------------------------

HOOK="$PWD/$PLUGIN/hooks/session-start"
bash -n "$HOOK" || fail "session-start does not parse"
command -v shellcheck >/dev/null 2>&1 && { shellcheck "$HOOK" || fail "shellcheck session-start"; }
python3 -c "import json; json.load(open('$PLUGIN/hooks/hooks.json'))" || fail "hooks.json is not valid JSON"

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

# H1. Silent in a repository without .qual files.
NOQUAL="$SANDBOX/noqual"
mkdir -p "$NOQUAL/.git"
out="$(run_hook "$NOQUAL" PATH="$STUB1:$PATH")"
[ -z "$out" ] || fail "hook must be silent without .qual files, got: $out"
ok "hook is silent in repositories without .qual files"

# H2. Injects the skill, binary, and summary in a repository with .qual files.
WITHQUAL="$SANDBOX/withqual"
mkdir -p "$WITHQUAL/.git" "$WITHQUAL/src"
echo '{}' >"$WITHQUAL/src/.qual"
out="$(run_hook "$WITHQUAL" PATH="$STUB1:$PATH")"
ctx="$(printf '%s' "$out" | context_of)" || fail "hook output is not the expected JSON: $out"
case "$ctx" in *"qual:recording-design-decisions"*) ;; *) fail "context lacks the using-qualifier map" ;; esac
case "$ctx" in *"1 blocker"*) ;; *) fail "context lacks the thread summary: $ctx" ;; esac
case "$ctx" in *"name: using-qualifier"*) fail "frontmatter must be stripped" ;; esac
# The backticks are a literal call-line quote, not command substitution.
# shellcheck disable=SC2016
case "$ctx" in *'Call it as `qualifier`'*) ;; *) fail "context must give the on-PATH call line: $ctx" ;; esac
case "$ctx" in *'ensure-qualifier.sh" exec'*) fail "an on-PATH binary must not route through the wrapper: $ctx" ;; esac
[ "${#ctx}" -lt 10000 ] || fail "context is ${#ctx} chars; the harness caps it at 10000"
ok "hook injects using-qualifier and the summary (${#ctx} chars)"

# H3. A binary off PATH is reached through the pre-approved wrapper.
out="$(run_hook "$WITHQUAL" QUALIFIER_BIN="$STUB_OVERRIDE/qualifier" PATH="/usr/bin:/bin")"
ctx="$(printf '%s' "$out" | context_of)"
case "$ctx" in *'ensure-qualifier.sh" exec'*) ;; *) fail "context must route an off-PATH binary through the wrapper: $ctx" ;; esac
ok "hook routes an off-PATH binary through the wrapper"

# H4. No binary and no network: still exit 0 with valid JSON.
FAILCURL="$SANDBOX/failcurl"
mkdir -p "$FAILCURL"
printf '#!/usr/bin/env bash\nexit 7\n' >"$FAILCURL/curl"
chmod +x "$FAILCURL/curl"
out="$(run_hook "$WITHQUAL" QUALIFIER_INSTALL_DIR="$SANDBOX/empty-bin" PATH="$FAILCURL:/usr/bin:/bin")" \
    || fail "hook must exit 0 when the binary cannot be installed"
ctx="$(printf '%s' "$out" | context_of)"
case "$ctx" in *"cargo install qualifier"*) ;; *) fail "context must say how to install: $ctx" ;; esac
ok "hook degrades gracefully without a binary or network"

# H5. Works without VCS markers (project dir is the root).
NOVCS="$SANDBOX/novcs"
mkdir -p "$NOVCS"
echo '{}' >"$NOVCS/.qual"
out="$(run_hook "$NOVCS" PATH="$STUB1:$PATH")"
printf '%s' "$out" | context_of >/dev/null || fail "hook must work outside a VCS"
ok "hook works outside a VCS"

# H6. Real git repositories: an untracked .qual is found; no .qual is silent.
GITQUAL="$SANDBOX/gitqual"
mkdir -p "$GITQUAL/src"
git -C "$GITQUAL" init -q
echo '{}' >"$GITQUAL/src/.qual"
out="$(run_hook "$GITQUAL" PATH="$STUB1:$PATH")"
printf '%s' "$out" | context_of >/dev/null || fail "an untracked .qual in a git repo must be found"
GITNOQUAL="$SANDBOX/gitnoqual"
mkdir -p "$GITNOQUAL"
git -C "$GITNOQUAL" init -q
echo "x" >"$GITNOQUAL/a.txt"
out="$(run_hook "$GITNOQUAL" PATH="$STUB1:$PATH")"
[ -z "$out" ] || fail "a git repo without .qual files must be silent, got: $out"
ok "hook gates git repositories through the index"

# H6b. A .qual file excluded by .gitignore must be silent: only the git
# index path (not a filesystem find fallback) is expected to honor it.
GITIGNORED="$SANDBOX/gitignored"
mkdir -p "$GITIGNORED/src"
git -C "$GITIGNORED" init -q
echo '*.qual' >"$GITIGNORED/.gitignore"
echo '{}' >"$GITIGNORED/src/.qual"
out="$(run_hook "$GITIGNORED" PATH="$STUB1:$PATH")"
[ -z "$out" ] || fail "a gitignored .qual must be silent, got: $out"
ok "hook honors .gitignore via the git index path"

# H7. An old binary puts an upgrade note into the context.
out="$(run_hook "$WITHQUAL" PATH="$STUB_OLD:$PATH")"
ctx="$(printf '%s' "$out" | context_of)"
case "$ctx" in *"older than 0.8.0"*) ;; *) fail "context must flag an old binary: $ctx" ;; esac
ok "hook flags a binary older than MIN_VERSION"

# H8. A slow summary is dropped instead of delaying the session.
SLOW="$SANDBOX/slow"
mkdir -p "$SLOW"
cat >"$SLOW/qualifier" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
    --version) echo "qualifier 9.9.9" ;;
    threads) sleep 5; echo "qualifier: too late" ;;
esac
EOF
chmod +x "$SLOW/qualifier"
start="$(date +%s)"
out="$(run_hook "$WITHQUAL" PATH="$SLOW:$PATH")"
elapsed=$(( $(date +%s) - start ))
[ "$elapsed" -lt 4 ] || fail "hook waited ${elapsed}s for a slow summary"
ctx="$(printf '%s' "$out" | context_of)"
case "$ctx" in *"too late"*) fail "a late summary must be dropped" ;; esac
ok "hook drops a summary that misses its one-second budget"

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
out="$(run_hook "$WITHQUAL" PATH="$CTRLCHARS:$PATH")"
ctx="$(printf '%s' "$out" | context_of)" || fail "control characters in the summary broke the JSON: $out"
case "$ctx" in *"1 blocker"*) ;; *) fail "context lost the summary text: $ctx" ;; esac
ok "hook strips control characters that would break the JSON"

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
    assert not re.search(r"\bTBD\b|(?<!\$)\{[A-Z_ ]+\}", body), f"{path}: placeholder text"
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
    # Fail-fast check, not if/then/else: either grep failing should fail.
    # shellcheck disable=SC2015
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

# 7. A foreign binary already at $QUALIFIER_INSTALL_DIR/qualifier is never
#    overwritten by an install.
make_fake_qualifier "$QUALIFIER_INSTALL_DIR/qualifier" "1.0" "something-else"
pinned_copy "$SANDBOX/ensure-foreign-install.sh" "$FIXTURE_SHA"
err="$(PATH="$SAFE_PATH" "$SANDBOX/ensure-foreign-install.sh" 2>&1 >/dev/null)" && fail "must refuse to overwrite a foreign binary"
case "$err" in *"QUALIFIER_INSTALL_DIR"*) ;; *) fail "expected an error naming QUALIFIER_INSTALL_DIR, got: $err" ;; esac
[ "$("$QUALIFIER_INSTALL_DIR/qualifier" --version)" = "something-else 1.0" ] || fail "the foreign binary must be left unchanged"
rm -f "$QUALIFIER_INSTALL_DIR/qualifier"
ok "refuses to overwrite a foreign binary in QUALIFIER_INSTALL_DIR"

# 8. No embedded checksum for this target: refuse to download, point at cargo.
pinned_copy "$SANDBOX/ensure-unverified.sh" ""
err="$(PATH="$SAFE_PATH" "$SANDBOX/ensure-unverified.sh" 2>&1 >/dev/null)" && fail "an unverified target must not install"
case "$err" in *"cargo install qualifier --version 9.9.9"*) ;; *) fail "expected the cargo fallback, got: $err" ;; esac
case "$err" in *"no verified"*) ;; *) fail "expected a 'no verified' message, got: $err" ;; esac
[ ! -e "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "nothing may be installed without a checksum"
ok "refuses to download a target without an embedded checksum"

# 9. Download the pinned release, verify, install.
pinned_copy "$SANDBOX/ensure-pinned.sh" "$FIXTURE_SHA"
out="$(PATH="$SAFE_PATH" "$SANDBOX/ensure-pinned.sh" 2>/dev/null)"
[ "$out" = "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "expected install to $QUALIFIER_INSTALL_DIR, got $out"
[ "$("$out" --version)" = "qualifier 9.9.9" ] || fail "installed binary does not run"
ok "downloads, verifies, and installs the pinned release"

# 10. A second run finds the installed binary without touching the network.
NOACCESS_STUB="$SANDBOX/curl-noaccess"
mkdir -p "$NOACCESS_STUB"
NOACCESS_MARKER="$SANDBOX/curl-was-called"
cat >"$NOACCESS_STUB/curl" <<EOF
#!/usr/bin/env bash
touch "$NOACCESS_MARKER"
echo "curl stub: unexpected network access" >&2
exit 1
EOF
chmod +x "$NOACCESS_STUB/curl"
rm -f "$NOACCESS_MARKER"
out="$(PATH="$NOACCESS_STUB:/usr/bin:/bin" "$SANDBOX/ensure-pinned.sh" 2>/dev/null)"
[ "$out" = "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "expected the installed binary, got $out"
[ ! -e "$NOACCESS_MARKER" ] || fail "reuse must not touch the network (curl was invoked)"
ok "reuses the installed binary without touching the network"

# 11. A checksum mismatch aborts and installs nothing.
rm -f "$QUALIFIER_INSTALL_DIR/qualifier"
pinned_copy "$SANDBOX/ensure-wrong-sha.sh" "0000000000000000000000000000000000000000000000000000000000000000"
err="$(PATH="$SAFE_PATH" "$SANDBOX/ensure-wrong-sha.sh" 2>&1 >/dev/null)" && fail "a checksum mismatch must fail"
case "$err" in *"checksum mismatch"*) ;; *) fail "expected a checksum mismatch message, got: $err" ;; esac
[ ! -e "$QUALIFIER_INSTALL_DIR/qualifier" ] || fail "nothing may be installed on checksum mismatch"
ok "checksum mismatch aborts the install"

echo "test-plugin: $PASS checks passed"
