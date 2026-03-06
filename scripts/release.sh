#!/usr/bin/env bash
set -euo pipefail

# Release the qualifier crate to crates.io
#
# Prerequisites:
#   cargo login <your-api-token>
#
# Usage:
#   ./scripts/release.sh                         # dry-run (default)
#   ./scripts/release.sh --execute               # actually publish
#   ./scripts/release.sh --allow-dirty            # dry-run, skip dirty check
#   ./scripts/release.sh --execute --allow-dirty  # publish with uncommitted changes

DRY_RUN=true
ALLOW_DIRTY=""

for arg in "$@"; do
    case "$arg" in
        --execute) DRY_RUN=false ;;
        --allow-dirty) ALLOW_DIRTY="--allow-dirty" ;;
    esac
done

echo "==> Running tests..."
cargo test --all-features

echo "==> Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "==> Verifying package..."
cargo publish --dry-run $ALLOW_DIRTY

if $DRY_RUN; then
    echo ""
    echo "Dry run complete. To publish for real, run:"
    echo "  ./scripts/release.sh --execute"
else
    echo "==> Publishing to crates.io..."
    cargo publish $ALLOW_DIRTY
    echo "==> Published!"
fi
