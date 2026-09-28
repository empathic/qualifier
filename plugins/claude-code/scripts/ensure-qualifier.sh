#!/usr/bin/env bash
# Resolve the qualifier binary this plugin runs: the release it pins,
# installed in a directory the plugin owns. A `qualifier` on PATH is never
# probed, used, or modified.
#
# Usage:
#   ensure-qualifier.sh                 print the absolute binary path on stdout
#   ensure-qualifier.sh exec <args...>  resolve, then run `qualifier <args...>`
#   ensure-qualifier.sh pinned-version  print PINNED_VERSION (resolves nothing)
#
# Everything except the resolved path / exec'd command output goes to stderr.
#
# Resolution order:
#   1. $QUALIFIER_BIN, when set to the absolute path of a qualifier.
#   2. The managed install $PLUGIN_HOME/$PINNED_VERSION/qualifier, when its
#      `--version` is exactly `qualifier $PINNED_VERSION`.
#   3. Otherwise download the PINNED_VERSION release for this platform,
#      verify it against the checksum embedded below, and install it as the
#      managed install: extracted into a staging directory inside
#      $PLUGIN_HOME, then renamed to $PLUGIN_HOME/$PINNED_VERSION. After a
#      successful install, version directories (N.N.N) older than
#      PINNED_VERSION are removed; never a newer version, and nothing else.
#      A failed download or checksum installs and removes nothing.
#
# Every run that resolves also removes staging leftovers of an interrupted
# install that are more than an hour old. Cleanup is best-effort and never
# changes the exit status.
#
# A plugin update that bumps PINNED_VERSION therefore installs the new
# release on its next use.
#
# Environment variables:
#   QUALIFIER_BIN          Absolute path to a specific qualifier, ahead of the
#                          managed install (for a working-tree build such as
#                          target/release/qualifier). Set but unusable is a
#                          warning, not a silent fall-through.
#   QUALIFIER_PLUGIN_HOME  Directory holding the managed installs (default
#                          $XDG_DATA_HOME/qualifier/plugin, else
#                          ~/.local/share/qualifier/plugin). It, and
#                          XDG_DATA_HOME, must be an absolute path other
#                          than /; otherwise it is a warning and the
#                          ~/.local/share default is used.

set -euo pipefail

REPO="empathic/qualifier"
# The release this plugin runs, and the sha256 of its tarball per target.
# Bumped together, in a plugin release, after the skills are checked against
# the new CLI. The checksums live here rather than being fetched from the
# release so a tampered release is detected. Empty means no verified binary
# for that target yet.
PINNED_VERSION="0.8.0"
SHA256_AARCH64_APPLE_DARWIN="9f4bdfd7c7742948c4c24f89c5f5beebf414632ba5a4d95028294b20912f8c72"
SHA256_X86_64_UNKNOWN_LINUX_MUSL="f39f525234cc7e473d20d88b5fca64b6b9c514de248cb7b7efe8958a8796f1ec"
SHA256_AARCH64_UNKNOWN_LINUX_GNU="1317d7cc42d8bba10e5c684604aae15e174f8994f0f3278600aec40110864fc9"
STAGING=""

log() { echo "$@" >&2; }

# Succeeds when $1 can hold the managed installs: an absolute path other
# than the root directory.
usable_home() {
    local trimmed="$1"
    case "$trimmed" in /*) ;; *) return 1 ;; esac
    while [ "${trimmed%/}" != "$trimmed" ]; do
        trimmed="${trimmed%/}"
    done
    [ -n "$trimmed" ]
}

# Sets PLUGIN_HOME. QUALIFIER_PLUGIN_HOME, else XDG_DATA_HOME, when set but
# unusable (relative, or /) is a warning and falls back to the default
# $HOME/.local/share/qualifier/plugin. Exits 1 when that is unusable too
# (HOME unset, empty, or relative), before anything is created or removed.
plugin_home() {
    PLUGIN_HOME=""
    if [ -n "${QUALIFIER_PLUGIN_HOME:-}" ]; then
        if usable_home "$QUALIFIER_PLUGIN_HOME"; then
            PLUGIN_HOME="$QUALIFIER_PLUGIN_HOME"
        else
            log "warning: \$QUALIFIER_PLUGIN_HOME is set to '${QUALIFIER_PLUGIN_HOME}', which is not an absolute path other than /; ignoring it."
        fi
    elif [ -n "${XDG_DATA_HOME:-}" ]; then
        if usable_home "$XDG_DATA_HOME"; then
            PLUGIN_HOME="$XDG_DATA_HOME/qualifier/plugin"
        else
            log "warning: \$XDG_DATA_HOME is set to '${XDG_DATA_HOME}', which is not an absolute path other than /; ignoring it."
        fi
    fi
    [ -n "$PLUGIN_HOME" ] && return 0
    local home="${HOME:-}"
    if [ -n "$home" ]; then
        # Trailing slashes dropped, so HOME=/ gives /.local/share/...
        while [ "${home%/}" != "$home" ]; do
            home="${home%/}"
        done
        if usable_home "$home/.local/share/qualifier/plugin"; then
            PLUGIN_HOME="$home/.local/share/qualifier/plugin"
            return 0
        fi
    fi
    log "Error: HOME is unset or not an absolute path, so there is no default plugin home."
    log "Set QUALIFIER_PLUGIN_HOME to an absolute directory for the plugin's qualifier installs."
    exit 1
}

is_qualifier() {
    [ -f "$1" ] && [ -x "$1" ] \
        && [ "$("$1" --version </dev/null 2>/dev/null | awk '{print $1}')" = "qualifier" ]
}

is_pinned() {
    [ -f "$1" ] && [ -x "$1" ] \
        && [ "$("$1" --version </dev/null 2>/dev/null)" = "qualifier $PINNED_VERSION" ]
}

# Prints $QUALIFIER_BIN when it is set to the absolute path of a qualifier.
# Set but unusable is a warning.
resolve_override() {
    [ -n "${QUALIFIER_BIN:-}" ] || return 1
    # Only an absolute path is eligible; a relative one is treated the same
    # as an unusable one below (its meaning depends on $PWD).
    case "$QUALIFIER_BIN" in
        /*)
            if is_qualifier "$QUALIFIER_BIN"; then
                echo "$QUALIFIER_BIN"
                return 0
            fi
            ;;
    esac
    log "warning: \$QUALIFIER_BIN is set to '${QUALIFIER_BIN}' but is not a usable qualifier; ignoring it."
    return 1
}

# Prints the managed install when it reports exactly the pinned version.
resolve_managed() {
    is_pinned "$PLUGIN_HOME/$PINNED_VERSION/qualifier" || return 1
    echo "$PLUGIN_HOME/$PINNED_VERSION/qualifier"
}

check_dependencies() {
    local missing=()
    for cmd in curl tar; do
        command -v "$cmd" >/dev/null 2>&1 || missing+=("$cmd")
    done
    if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
        missing+=("sha256sum or shasum")
    fi
    if [ ${#missing[@]} -gt 0 ]; then
        log "Error: required commands not found: ${missing[*]}"
        exit 1
    fi
}

cargo_fallback() {
    log "Error: $1."
    log "Install qualifier manually instead: cargo install qualifier --version ${PINNED_VERSION}"
    exit 1
}

resolve_target() {
    case "$(uname -s)-$(uname -m)" in
        Darwin-arm64)              echo "aarch64-apple-darwin" ;;
        Linux-x86_64)              echo "x86_64-unknown-linux-musl" ;;
        Linux-aarch64|Linux-arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) cargo_fallback "no prebuilt binary for $(uname -s)-$(uname -m)" ;;
    esac
}

expected_sha256() {
    case "$1" in
        aarch64-apple-darwin)      echo "$SHA256_AARCH64_APPLE_DARWIN" ;;
        x86_64-unknown-linux-musl) echo "$SHA256_X86_64_UNKNOWN_LINUX_MUSL" ;;
        aarch64-unknown-linux-gnu) echo "$SHA256_AARCH64_UNKNOWN_LINUX_GNU" ;;
    esac
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

# Downloads, verifies, and extracts the release into $STAGING, leaving only
# the release's contents there.
download_and_verify() {
    local target="$1" expected actual
    local tarball="qualifier-${target}.tar.gz"
    local url="https://github.com/${REPO}/releases/download/v${PINNED_VERSION}/${tarball}"

    log "Downloading qualifier ${PINNED_VERSION} (${target})..."
    curl -fsSL --connect-timeout 5 --max-time 120 "$url" -o "${STAGING}/${tarball}" \
        || cargo_fallback "could not download ${url}"

    actual="$(sha256_of "${STAGING}/${tarball}")"
    expected="$(expected_sha256 "$target")"
    if [ "$actual" != "$expected" ]; then
        log "Error: checksum mismatch for ${tarball} (expected ${expected}, got ${actual})."
        exit 1
    fi
    tar -xzf "${STAGING}/${tarball}" -C "$STAGING"
    rm -f "${STAGING}/${tarball}"
    [ -f "${STAGING}/qualifier" ] || { log "Error: ${tarball} does not contain qualifier."; exit 1; }
    chmod +x "${STAGING}/qualifier"
    is_pinned "${STAGING}/qualifier" \
        || { log "Error: the downloaded binary does not report qualifier ${PINNED_VERSION}."; exit 1; }
}

# Splits $1 into VERSION_PARTS (three numbers) when it is exactly N.N.N
# with 1-9 digits per component; fails otherwise.
split_version() {
    local v="$1" rest part
    VERSION_PARTS=()
    rest="${v#*.}"
    [ "$rest" != "$v" ] || return 1
    VERSION_PARTS[0]="${v%%.*}"
    VERSION_PARTS[1]="${rest%%.*}"
    VERSION_PARTS[2]="${rest#*.}"
    [ "${VERSION_PARTS[2]}" != "$rest" ] || return 1
    for part in "${VERSION_PARTS[@]}"; do
        # More than 9 digits is not a version this script compares.
        case "$part" in ''|*[!0-9]*|??????????*) return 1 ;; esac
    done
}

# Succeeds when $1 and $2 are both N.N.N versions and $1 is strictly older,
# comparing each component numerically. Anything else fails, so a name
# that isn't a plain version is never treated as older.
version_older_than() {
    local a b i
    split_version "$2" || return 1
    b=("${VERSION_PARTS[@]}")
    split_version "$1" || return 1
    a=("${VERSION_PARTS[@]}")
    for i in 0 1 2; do
        # 10# keeps a leading zero from being read as octal.
        if [ $((10#${a[i]})) -lt $((10#${b[i]})) ]; then return 0; fi
        if [ $((10#${a[i]})) -gt $((10#${b[i]})) ]; then return 1; fi
    done
    return 1
}

# Removes staging and stale directories an interrupted install left behind
# more than an hour ago (a younger one may belong to a concurrent install).
# Best-effort: never affects the exit status.
remove_leftovers() {
    [ -d "$PLUGIN_HOME" ] || return 0
    find "$PLUGIN_HOME" -mindepth 1 -maxdepth 1 -type d \
        \( -name '.staging.*' -o -name '.stale.*' \) -mmin +60 \
        -exec rm -rf {} + >/dev/null 2>&1 || true
}

# Removes version directories older than the pinned one (never a newer one,
# which a session running a newer plugin may be using), then leftovers.
# Nothing else under $PLUGIN_HOME is touched. Best-effort: never affects
# the exit status.
cleanup_plugin_home() {
    local dir
    for dir in "$PLUGIN_HOME"/[0-9]*.[0-9]*.[0-9]*; do
        [ -d "$dir" ] || continue
        if version_older_than "${dir##*/}" "$PINNED_VERSION"; then
            rm -rf "$dir" >/dev/null 2>&1 || true
        fi
    done
    remove_leftovers
}

# Sets $RESOLVED_BIN. Not called via $(...): subshells don't inherit `set -e`
# on macOS's bash 3.2, and a failed download or checksum must abort.
install_qualifier() {
    local target dest="$PLUGIN_HOME/$PINNED_VERSION" stale
    target="$(resolve_target)"
    [ -n "$(expected_sha256 "$target")" ] \
        || cargo_fallback "no verified qualifier ${PINNED_VERSION} binary for ${target}"
    check_dependencies

    # Staging lives inside $PLUGIN_HOME so the final rename stays on one
    # filesystem. Its dot-prefixed name never looks like a version.
    mkdir -p "$PLUGIN_HOME"
    STAGING="$(mktemp -d "$PLUGIN_HOME/.staging.XXXXXX")"
    trap 'rm -rf "$STAGING"' EXIT
    download_and_verify "$target"

    if [ -e "$dest" ] && ! is_pinned "$dest/qualifier"; then
        # An invalid install (wrong version, or damaged): set it aside, so
        # the rename below can put the new one in its place.
        # Best-effort: a concurrent session may have moved it already; the
        # checks below settle the outcome either way.
        if stale="$(mktemp -d "$PLUGIN_HOME/.stale.XXXXXX" 2>/dev/null)"; then
            mv "$dest" "$stale/" >/dev/null 2>&1 || true
            rm -rf "$stale" >/dev/null 2>&1 || true
        fi
    fi
    if [ ! -e "$dest" ]; then
        mv "$STAGING" "$dest" || true
    fi
    # If a concurrent session renamed its install into place between the
    # check and the rename above, mv nested the staging directory inside it
    # (or left it where it was); either way, discard this run's copy.
    rm -rf "${dest:?}/${STAGING##*/}" "$STAGING"
    if ! is_pinned "$dest/qualifier"; then
        # A concurrent session's rename may land a moment after ours
        # failed: look once more before giving up.
        sleep 1
        if ! is_pinned "$dest/qualifier"; then
            log "Error: could not install qualifier ${PINNED_VERSION} to ${dest}."
            exit 1
        fi
    fi

    cleanup_plugin_home
    log "Installed qualifier ${PINNED_VERSION} to ${dest}/qualifier"
    RESOLVED_BIN="$dest/qualifier"
}

main() {
    case "${1:-}" in
        pinned-version)
            echo "$PINNED_VERSION"
            return 0
            ;;
        exec|"") ;;
        *)
            log "usage: ensure-qualifier.sh [exec <args...> | pinned-version]"
            exit 2
            ;;
    esac

    # $QUALIFIER_BIN first: it needs no plugin home, so a missing HOME
    # doesn't stop it.
    local bin
    if ! bin="$(resolve_override)"; then
        plugin_home
        if bin="$(resolve_managed)"; then
            remove_leftovers
        else
            install_qualifier
            bin="$RESOLVED_BIN"
        fi
    fi

    if [ "${1:-}" = exec ]; then
        shift
        exec "$bin" "$@"
    fi
    echo "$bin"
}

main "$@"
