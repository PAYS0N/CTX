#!/usr/bin/env bash
# Assemble the canonical "tool contracts" doc block from each binary's own
# `--contract` output, and either write it into the docs (`--write`) or
# verify the committed docs still match (`--check`, used by ctx-verify).
#
# The single source of truth is the binary: each of ctx-context /
# ctx-verify / ctx-scan / ctx-cage prints a one-paragraph agent-facing
# contract under `--contract`. A contract change that doesn't regenerate
# the docs fails the `contracts` battery check — the doc block cannot
# drift from the code, symmetric with how the repo treats every other
# "must". Never hand-edit the block between the markers.
#
# Usage: gen_tool_contracts.sh [--write|--check] [ROOT]
#   --write  (default) rewrite the block in each target doc in place
#   --check  emit `FAIL:` lines (ctx-verify format) if any doc is stale

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

BEGIN='<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->'
END='<!-- END GENERATED tool-contracts -->'

# Docs carrying the block. This is the template/scaffolded-project copy
# of this script, which intentionally diverges from the CTX repo's own
# scripts/gen_tool_contracts.sh here: a cloned project has a single
# CLAUDE.md and no template/ dir, and its README.md documents the
# project itself, not the CTX tooling — it should never carry this
# block. Keep this DOCS line as the one deliberate difference between
# the two copies; update-template.sh still overwrites everything else
# in this file unconditionally on every sync.
DOCS=(CLAUDE.md)

BIN_DIR="target/debug"
FAIL=0

# Emit a FAIL line (check mode) or die (write mode) with a message.
fail_or_die() {
    if [[ "$MODE" == "--check" ]]; then
        echo "FAIL: gen_tool_contracts.sh: $1" >&2
        FAIL=1
    else
        echo "gen_tool_contracts.sh: $1" >&2
        exit 1
    fi
}

# The five contract-bearing binaries must already be built and present at
# $BIN_DIR. This script never invokes cargo itself — install-tools.sh is
# the sole build/install step (see its own header comment), so this stays
# accurate whether these binaries are local workspace members (the CTX
# repo checking itself) or artifacts copied in from elsewhere (a
# scaffolded project, where they aren't buildable from $ROOT at all).
for bin in ctx-context ctx-verify ctx-scan ctx-cage ctx-brief; do
    if [[ ! -x "$BIN_DIR/$bin" ]]; then
        fail_or_die "$BIN_DIR/$bin: not found or not executable — run install-tools.sh (or, in the CTX repo itself, cargo build) first"
    fi
done
if [[ "$FAIL" -ne 0 ]]; then
    exit $FAIL
fi

# Assemble the generated block into $BLOCK.
one_contract() {
    local bin="$1" label="$2"
    local text
    text="$("$BIN_DIR/$bin" --contract)"
    printf -- '- **%s** — %s\n' "$label" "$text"
}

BLOCK="$BEGIN"$'\n'
BLOCK+="$(one_contract ctx-context ctx-context)"$'\n'
BLOCK+="$(one_contract ctx-verify ctx-verify)"$'\n'
BLOCK+="$(one_contract ctx-scan ctx-scan)"$'\n'
BLOCK+="$(one_contract ctx-cage ctx-cage)"$'\n'
BLOCK+="$(one_contract ctx-brief ctx-brief)"$'\n'
BLOCK+="$END"

# Extract the committed block (markers inclusive) from a doc, or empty.
committed_block() {
    awk -v b="$BEGIN" -v e="$END" '
        index($0, b) { f = 1 }
        f { print }
        index($0, e) { f = 0 }
    ' "$1"
}

for doc in "${DOCS[@]}"; do
    if [[ ! -f "$doc" ]]; then
        fail_or_die "$doc: missing"
        continue
    fi
    if ! grep -qF "$BEGIN" "$doc" || ! grep -qF "$END" "$doc"; then
        fail_or_die "$doc: tool-contract markers not found"
        continue
    fi
    if [[ "$MODE" == "--check" ]]; then
        if [[ "$(committed_block "$doc")" != "$BLOCK" ]]; then
            echo "FAIL: $doc:1: tool-contracts block is stale — run scripts/gen_tool_contracts.sh --write" >&2
            FAIL=1
        fi
        continue
    fi
    # --write: replace the marked region with the fresh block.
    tmp="$(mktemp)"
    printf '%s\n' "$BLOCK" >"$tmp.block"
    awk -v bf="$tmp.block" -v b="$BEGIN" -v e="$END" '
        BEGIN { while ((getline line < bf) > 0) blk = blk line "\n"; sub(/\n$/, "", blk) }
        index($0, b) { print blk; skip = 1; next }
        index($0, e) { skip = 0; next }
        !skip { print }
    ' "$doc" >"$tmp"
    mv "$tmp" "$doc"
    rm -f "$tmp.block"
done

exit $FAIL
