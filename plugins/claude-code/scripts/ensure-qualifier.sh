#!/usr/bin/env bash
# Locate the qualifier binary, installing the pinned release if needed.
#
# Usage:
#   ensure-qualifier.sh                 print the absolute binary path on stdout
#   ensure-qualifier.sh exec <args...>  resolve, then run `qualifier <args...>`
#   ensure-qualifier.sh min-version     print MIN_VERSION (resolves nothing)
#   ensure-qualifier.sh pinned-version  print PINNED_VERSION (resolves nothing)
#
# Everything except the resolved path / exec'd command output goes to stderr.
#
# Resolution order:
#   1. $QUALIFIER_BIN, when set and usable
#   2. `qualifier` on PATH that identifies as this CLI
#   3. $QUALIFIER_INSTALL_DIR/qualifier (default ~/.local/bin/qualifier)
#   4. Download the PINNED_VERSION release for this platform, verify it
#      against the checksum embedded below, and install it to
#      $QUALIFIER_INSTALL_DIR.
#
# Environment variables:
#   QUALIFIER_BIN          Absolute path to a specific qualifier, ahead of every
#                          other candidate (for a working-tree build such as
#                          target/release/qualifier). Set but unusable is a
#                          warning, not a silent fall-through.
#   QUALIFIER_INSTALL_DIR  Install directory (default ~/.local/bin)

set -euo pipefail

REPO="empathic/qualifier"
INSTALL_DIR="${QUALIFIER_INSTALL_DIR:-$HOME/.local/bin}"
# Oldest release whose CLI surface the plugin's skills are written against.
MIN_VERSION="0.8.0"
# The release this plugin installs, and the sha256 of its tarball per target.
# Bumped together, in a plugin release, after the skills are checked against
# the new CLI. The checksums live here rather than being fetched from the
# release so a tampered release is detected. Empty means no verified binary
# for that target yet.
PINNED_VERSION="0.8.0"
SHA256_AARCH64_APPLE_DARWIN="9f4bdfd7c7742948c4c24f89c5f5beebf414632ba5a4d95028294b20912f8c72"
SHA256_X86_64_UNKNOWN_LINUX_MUSL="f39f525234cc7e473d20d88b5fca64b6b9c514de248cb7b7efe8958a8796f1ec"
SHA256_AARCH64_UNKNOWN_LINUX_GNU="1317d7cc42d8bba10e5c684604aae15e174f8994f0f3278600aec40110864fc9"
TMPDIR_CLEANUP=""

log() { echo "$@" >&2; }

is_qualifier() {
    [ -x "$1" ] && [ "$("$1" --version </dev/null 2>/dev/null | awk '{print $1}')" = "qualifier" ]
}

warn_if_old() {
    local version
    version="$("$1" --version </dev/null 2>/dev/null | awk '{print $2}')" || return 0
    [ -n "$version" ] || return 0
    if [ "$(printf '%s\n%s\n' "$MIN_VERSION" "$version" | sort -V | head -1)" != "$MIN_VERSION" ]; then
        log "warning: qualifier ${version} is older than ${MIN_VERSION}; some qual skills may not work."
        log "Upgrade: cargo install qualifier (or remove it so the plugin can install a release)"
    fi
}

resolve_existing() {
    local candidate dir
    if [ -n "${QUALIFIER_BIN:-}" ]; then
        # Only an absolute path is eligible; a relative one is treated the
        # same as an unusable one below (its meaning depends on $PWD).
        case "$QUALIFIER_BIN" in
            /*)
                if is_qualifier "$QUALIFIER_BIN"; then
                    echo "$QUALIFIER_BIN"
                    return 0
                fi
                ;;
        esac
        log "warning: \$QUALIFIER_BIN is set to '${QUALIFIER_BIN}' but is not a usable qualifier; ignoring it."
    fi
    # Walk every `qualifier` on PATH, skipping foreign binaries with the same
    # name and any non-absolute entry (including a bare `.`), so the printed
    # path is always absolute. Globbing is disabled so an unquoted $PATH
    # entry containing a glob character is never expanded.
    local IFS=:
    set -f
    for dir in $PATH; do
        case "$dir" in
            /*) ;;
            *) continue ;;
        esac
        candidate="$dir/qualifier"
        if is_qualifier "$candidate"; then
            set +f
            echo "$candidate"
            return 0
        fi
    done
    set +f
    if is_qualifier "$INSTALL_DIR/qualifier"; then
        echo "$INSTALL_DIR/qualifier"
        return 0
    fi
    return 1
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

refuse_if_install_dir_occupied() {
    if [ -e "${INSTALL_DIR}/qualifier" ]; then
        log "Error: ${INSTALL_DIR}/qualifier already exists and is not a usable qualifier."
        log "Set QUALIFIER_INSTALL_DIR to a different directory, or remove ${INSTALL_DIR}/qualifier, then retry."
        exit 1
    fi
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

download_and_verify() {
    local target="$1" tmpdir="$2" expected actual
    local tarball="qualifier-${target}.tar.gz"
    local url="https://github.com/${REPO}/releases/download/v${PINNED_VERSION}/${tarball}"

    expected="$(expected_sha256 "$target")"
    [ -n "$expected" ] || cargo_fallback "no verified qualifier ${PINNED_VERSION} binary for ${target}"

    log "Downloading qualifier ${PINNED_VERSION} (${target})..."
    curl -fsSL --connect-timeout 5 --max-time 120 "$url" -o "${tmpdir}/${tarball}" \
        || cargo_fallback "could not download ${url}"

    actual="$(sha256_of "${tmpdir}/${tarball}")"
    if [ "$actual" != "$expected" ]; then
        log "Error: checksum mismatch for ${tarball} (expected ${expected}, got ${actual})."
        exit 1
    fi
    tar -xzf "${tmpdir}/${tarball}" -C "$tmpdir"
}

path_hint() {
    case ":$PATH:" in
        *":$1:"*) ;;
        *)
            log ""
            log "Note: $1 is not on your PATH. To use qualifier from your shell, add:"
            log "  export PATH=\"$1:\$PATH\""
            ;;
    esac
}

# Sets $RESOLVED_BIN. Not called via $(...): subshells don't inherit `set -e`
# on macOS's bash 3.2, and a failed download or checksum must abort.
install_qualifier() {
    refuse_if_install_dir_occupied
    check_dependencies
    local target
    target="$(resolve_target)"

    TMPDIR_CLEANUP="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR_CLEANUP"' EXIT

    download_and_verify "$target" "$TMPDIR_CLEANUP"
    mkdir -p "$INSTALL_DIR"
    mv "${TMPDIR_CLEANUP}/qualifier" "${INSTALL_DIR}/qualifier"
    chmod +x "${INSTALL_DIR}/qualifier"
    rm -rf "$TMPDIR_CLEANUP"

    log "Installed qualifier ${PINNED_VERSION} to ${INSTALL_DIR}/qualifier"
    path_hint "$INSTALL_DIR"
    RESOLVED_BIN="${INSTALL_DIR}/qualifier"
}

main() {
    case "${1:-}" in
        min-version)
            echo "$MIN_VERSION"
            return 0
            ;;
        pinned-version)
            echo "$PINNED_VERSION"
            return 0
            ;;
    esac

    local bin
    if ! bin="$(resolve_existing)"; then
        install_qualifier
        bin="$RESOLVED_BIN"
    fi
    warn_if_old "$bin"

    case "${1:-}" in
        exec)
            shift
            exec "$bin" "$@"
            ;;
        "")
            echo "$bin"
            ;;
        *)
            log "usage: ensure-qualifier.sh [exec <args...> | min-version | pinned-version]"
            exit 2
            ;;
    esac
}

main "$@"
