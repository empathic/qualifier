#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [ ! -d node_modules ]; then
  echo "Installing dependencies..."
  pnpm install
fi

exec pnpm exec eleventy --serve --port 0
