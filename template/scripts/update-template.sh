#!/usr/bin/env bash
# Fully update this project's copy of the CTX tooling: sync
# install-tools.sh itself, then use it to rebuild/copy the ctx-*
# binaries, then sync a fixed set of pure-tooling files from the
# upstream CTX repo that install-tools.sh doesn't touch — the runtime
# prompt files, lint configs, the check-script battery, the Claude Code
# hook settings, and the tool-contracts block in CLAUDE.md.
# install-tools.sh alone can't pull in changes to those; this is the
# single command a scaffolded project runs to catch up fully.
#
# Usage:
#   scripts/update-template.sh [path-to-CTX-repo]
#
# The path can also be set via CTX_REPO. Defaults to ../CTX (the CTX repo
# as a sibling of this project) if neither is given, same as
# install-tools.sh. Every mirror-set file below is overwritten
# unconditionally on every run — no version check or skip-if-locally-
# modified logic, matching install-tools.sh's existing precedent.

set -euo pipefail

CTX_REPO="${1:-${CTX_REPO:-../CTX}}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Sync install-tools.sh itself first — otherwise this step would run
# whatever stale local copy the project happened to be scaffolded with,
# silently missing any packages/binaries added to it upstream since.
cp "$CTX_REPO/template/scripts/install-tools.sh" "$SCRIPT_DIR/install-tools.sh"
echo "OK: synced scripts/install-tools.sh"

# Delegates the CTX_REPO validity check to install-tools.sh itself (it
# fails fast with its own FAIL message on a bad path) rather than
# reimplementing that check here.
echo "Running install-tools.sh..."
"$SCRIPT_DIR/install-tools.sh" "$CTX_REPO"

# Mirror set: pure tooling/doctrine files with no expected local edits.
# Most of template/ (Cargo.toml package/deps/members, CLAUDE.md project
# prose) is project-owned and stays out of this list on purpose.
TOP_LEVEL_FILES=(clippy.toml rustfmt.toml deny.toml)
CHECK_SCRIPTS=(cycle_check.sh machete_check.sh no_allow_check.sh workspace_lints_check.sh rationale_check.py gen_readme_architecture.sh gen_tool_contracts.sh retired_terms_check.sh)
# Runtime prompt files: ctx-scan/ctx-summarize/ctx-brief load these from
# ./prompts at runtime, resolved against cwd (the project root), not the
# tooling repo — install-tools.sh only builds the binaries that consume
# them. auditor.md, prompts/README.md and prompts/manual/ are excluded:
# they're CTX-repo-internal (the Layer 3 auditor isn't wired up yet, and
# README/manual are doctrine, not runtime input).
PROMPTS=(summarizer-leaf.md summarizer-rollup.md briefer-gather.md briefer-plan.md briefer-plan-headless.md)

for f in "${TOP_LEVEL_FILES[@]}"; do
    cp "$CTX_REPO/template/$f" "$f"
    echo "OK: synced $f"
done

mkdir -p scripts
for f in "${CHECK_SCRIPTS[@]}"; do
    cp "$CTX_REPO/template/scripts/$f" "scripts/$f"
    echo "OK: synced scripts/$f"
done

mkdir -p prompts
for f in "${PROMPTS[@]}"; do
    cp "$CTX_REPO/prompts/$f" "prompts/$f"
    echo "OK: synced prompts/$f"
done

mkdir -p .claude
cp "$CTX_REPO/template/.claude/settings.json" ".claude/settings.json"
echo "OK: synced .claude/settings.json"

# CLAUDE.md tool-contracts block: splice only the marked region — a
# scaffolded project's CLAUDE.md carries project-specific content outside
# it, so this can't be a whole-file copy. Pulls the already-regenerated
# block out of the CTX repo's own template/CLAUDE.md (one of
# gen_tool_contracts.sh's DOCS targets, so it's always current there)
# instead of running gen_tool_contracts.sh locally — its DOCS array is
# hardcoded to the CTX repo's own three docs and doesn't fit this
# project's single-CLAUDE.md layout. Marker strings and splice technique
# copied verbatim from scripts/gen_tool_contracts.sh in the CTX repo.
BEGIN='<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->'
END='<!-- END GENERATED tool-contracts -->'

SRC_DOC="$CTX_REPO/template/CLAUDE.md"

if [[ ! -f "$SRC_DOC" ]]; then
    echo "FAIL: '$SRC_DOC' not found; cannot sync the CLAUDE.md tool-contracts block." >&2
    exit 1
fi
if ! grep -qF "$BEGIN" "$SRC_DOC" || ! grep -qF "$END" "$SRC_DOC"; then
    echo "FAIL: $SRC_DOC: tool-contract markers not found." >&2
    exit 1
fi
if [[ ! -f CLAUDE.md ]]; then
    echo "FAIL: CLAUDE.md not found in this project." >&2
    exit 1
fi
if ! grep -qF "$BEGIN" CLAUDE.md || ! grep -qF "$END" CLAUDE.md; then
    echo "FAIL: CLAUDE.md: tool-contract markers not found." >&2
    exit 1
fi

blockfile="$(mktemp)"
awk -v b="$BEGIN" -v e="$END" '
    index($0, b) { f = 1 }
    f { print }
    index($0, e) { f = 0 }
' "$SRC_DOC" >"$blockfile"

tmp="$(mktemp)"
awk -v bf="$blockfile" -v b="$BEGIN" -v e="$END" '
    BEGIN { while ((getline line < bf) > 0) blk = blk line "\n"; sub(/\n$/, "", blk) }
    index($0, b) { print blk; skip = 1; next }
    index($0, e) { skip = 0; next }
    !skip { print }
' CLAUDE.md >"$tmp"
mv "$tmp" CLAUDE.md
rm -f "$blockfile"
echo "OK: synced CLAUDE.md tool-contracts block"

# Sync this script itself last, after every other step above has already
# completed and install-tools.sh has already run. Bash reads a running
# script off disk in chunks as it executes, so overwriting the file
# mid-run isn't guaranteed safe for a multi-step script — this cp is the
# final action before exit.
cp "$CTX_REPO/template/scripts/update-template.sh" "$SCRIPT_DIR/update-template.sh"
echo "OK: synced scripts/update-template.sh"
