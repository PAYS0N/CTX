#!/usr/bin/env bash
# Inject the repo root's generated rollup (.context/rollup.ctx raw bytes)
# into README.md's architecture block. Mirrors gen_tool_contracts.sh's
# --write/--check marker-replacement shape, but sources .context/rollup.ctx
# directly off disk instead of shelling a --contract call — the rollup is
# already the generated artifact, there is no binary to invoke. README.md
# only: this is repo-specific landing prose, not something
# template-scaffolded projects need.
#
# Usage: gen_readme_architecture.sh [--write|--check] [ROOT]
#   --write  (default) rewrite the block in README.md in place
#   --check  emit `FAIL:` lines (ctx-verify format) if README.md is stale

set -euo pipefail

MODE="--write"
ROOT="."
for arg in "$@"; do
    case "$arg" in
    --write | --check) MODE="$arg" ;;
    *) ROOT="$arg" ;;
    esac
done
cd "$ROOT"

BEGIN='<!-- BEGIN GENERATED architecture (scripts/gen_readme_architecture.sh --write) -->'
END='<!-- END GENERATED architecture -->'
DOC="README.md"
ROLLUP=".context/rollup.ctx"

FAIL=0

# Emit a FAIL line (check mode) or die (write mode) with a message.
fail_or_die() {
    if [[ "$MODE" == "--check" ]]; then
        echo "FAIL: gen_readme_architecture.sh: $1" >&2
        FAIL=1
    else
        echo "gen_readme_architecture.sh: $1" >&2
        exit 1
    fi
}

if [[ ! -f "$ROLLUP" ]]; then
    fail_or_die "$ROLLUP: missing"
    exit $FAIL
fi

if [[ ! -f "$DOC" ]]; then
    fail_or_die "$DOC: missing"
    exit $FAIL
fi

if ! grep -qF "$BEGIN" "$DOC" || ! grep -qF "$END" "$DOC"; then
    fail_or_die "$DOC: architecture markers not found"
    exit $FAIL
fi

BLOCK="$BEGIN"$'\n'"$(cat "$ROLLUP")"$'\n'"$END"

# Extract the committed block (markers inclusive) from the doc, or empty.
committed_block() {
    awk -v b="$BEGIN" -v e="$END" '
        index($0, b) { f = 1 }
        f { print }
        index($0, e) { f = 0 }
    ' "$1"
}

if [[ "$MODE" == "--check" ]]; then
    if [[ "$(committed_block "$DOC")" != "$BLOCK" ]]; then
        echo "FAIL: $DOC:1: architecture block is stale — run scripts/gen_readme_architecture.sh --write" >&2
        FAIL=1
    fi
    exit $FAIL
fi

# --write: replace the marked region with the fresh block.
tmp="$(mktemp)"
printf '%s\n' "$BLOCK" >"$tmp.block"
awk -v bf="$tmp.block" -v b="$BEGIN" -v e="$END" '
    BEGIN { while ((getline line < bf) > 0) blk = blk line "\n"; sub(/\n$/, "", blk) }
    index($0, b) { print blk; skip = 1; next }
    index($0, e) { skip = 0; next }
    !skip { print }
' "$DOC" >"$tmp"
mv "$tmp" "$DOC"
rm -f "$tmp.block"

exit $FAIL
