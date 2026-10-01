#!/usr/bin/env bash
# Run the qual plugin's eval suite (`claude plugin eval`) the way it needs to
# run: from the repository root, with the scaffold scripts enabled, the gated
# tools granted, and the qualifier built from this checkout first on PATH (the
# scaffold scripts receive only PATH, so QUALIFIER_BIN can't reach them).
#
#   scripts/eval-plugin.sh [--with <plugin-dir>]... [claude plugin eval options]
#
# --with loads another plugin alongside qual in every case (for example
# superpowers). `claude plugin eval` only loads plugins from inside the plugin
# under test, so this runs the suite from a temporary copy of
# plugins/claude-code with each --with plugin copied under .eval-plugins/ and
# added to every case's `plugins:` list. Reports still land in
# plugins/claude-code/evals/results/.
#
# Every run calls the model and costs money. A --max-cost-usd ceiling of 40 is
# passed unless you give one.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"
PLUGIN="$ROOT/plugins/claude-code"

with=()
args=()
while [ $# -gt 0 ]; do
    case "$1" in
        --with)
            [ $# -ge 2 ] || { echo "eval-plugin.sh: --with needs a plugin directory" >&2; exit 2; }
            [ -d "$2" ] || { echo "eval-plugin.sh: --with $2: not a directory" >&2; exit 2; }
            with+=("$(cd "$2" && pwd)")
            shift 2
            ;;
        *)
            args+=("$1")
            shift
            ;;
    esac
done

ceiling=(--max-cost-usd 40)
for a in ${args[@]+"${args[@]}"}; do
    case "$a" in --max-cost-usd|--max-cost-usd=*) ceiling=() ;; esac
done

cargo build --quiet --bin qualifier
PATH="$ROOT/target/debug:$PATH"
export PATH

common=(--scaffold --allow-tools Write Edit Bash)

if [ ${#with[@]} -eq 0 ]; then
    exec claude plugin eval "$PLUGIN" "${common[@]}" ${ceiling[@]+"${ceiling[@]}"} \
        ${args[@]+"${args[@]}"}
fi

stage="$(mktemp -d "${TMPDIR:-/tmp}/qual-eval.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/plugins"
cp -R "$PLUGIN" "$stage/plugins/claude-code"
rm -rf "$stage/plugins/claude-code/evals/results"
extra='"../.."'
for dir in "${with[@]}"; do
    name="$(basename "$dir")"
    # A versioned cache directory (…/superpowers/6.4.1) is named for its
    # version; use the plugin's directory name instead.
    case "$name" in [0-9]*) name="$(basename "$(dirname "$dir")")" ;; esac
    mkdir -p "$stage/plugins/claude-code/.eval-plugins"
    cp -R "$dir" "$stage/plugins/claude-code/.eval-plugins/$name"
    extra="$extra, \"../../.eval-plugins/$name\""
done
for prompt in "$stage"/plugins/claude-code/evals/*/prompt.md; do
    head -n 1 "$prompt" | grep -qx -- '---' \
        || { echo "eval-plugin.sh: $prompt has no frontmatter" >&2; exit 1; }
    if grep -q '^plugins:' "$prompt"; then
        echo "eval-plugin.sh: $prompt already sets plugins:" >&2
        exit 1
    fi
    { echo '---'; echo "plugins: [$extra]"; tail -n +2 "$prompt"; } >"$prompt.new"
    mv "$prompt.new" "$prompt"
done

# The copy is a directory this script just made from this checkout, so it is
# trusted on the same terms as the checkout itself.
claude plugin eval "$stage/plugins/claude-code" "${common[@]}" --trust-plugin \
    --output-dir "$PLUGIN/evals/results/$(date -u +%Y-%m-%dT%H-%M-%SZ)-with" \
    ${ceiling[@]+"${ceiling[@]}"} ${args[@]+"${args[@]}"}
