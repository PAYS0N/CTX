#!/usr/bin/env bash
# Bootstrap this project's ctx-* tool binaries.
#
# template/ is a scaffold for the *project's own* crates; the tooling
# (ctx-context, ctx-scan, ctx-verify, ctx-cage, ctx-run, ctx-summarize)
# lives in the CTX repo this template was copied from. The Claude Code
# hook and CLAUDE.md both call these by their target/debug path, so a
# freshly copied project needs them built and placed there before first
# use. ctx-scan and ctx-summarize also read their summarizer prompt files
# from ./prompts at runtime (cwd-relative), so those are copied in too.
# Both binaries and prompts are refreshed on every run, so re-run this
# once after copying template/, and again whenever the tooling repo
# updates.
#
# Usage:
#   scripts/install-tools.sh [path-to-CTX-repo]
#
# The path can also be set via CTX_REPO. Defaults to ../CTX (the CTX repo
# as a sibling of this project) if neither is given.

set -euo pipefail

CTX_REPO="${1:-${CTX_REPO:-../CTX}}"

if [[ ! -f "$CTX_REPO/crates/ctx-context/Cargo.toml" ]]; then
    echo "FAIL: '$CTX_REPO' does not look like the CTX tooling repo (no crates/ctx-context/Cargo.toml)." >&2
    echo "      Pass its path: scripts/install-tools.sh /path/to/CTX" >&2
    echo "      or set CTX_REPO=/path/to/CTX" >&2
    exit 1
fi

# ctx-cage's Cargo.toml produces two binaries (ctx-cage, ctx-run) from one
# package, so the -p list (packages) and the binary list (files) differ.
PACKAGES=(ctx-context ctx-scan ctx-verify ctx-cage ctx-summarize)
BINARIES=(ctx-context ctx-scan ctx-verify ctx-cage ctx-run ctx-summarize)

CARGO_ARGS=()
for pkg in "${PACKAGES[@]}"; do
    CARGO_ARGS+=(-p "$pkg")
done

echo "Building tools from $CTX_REPO..."
(cd "$CTX_REPO" && cargo build --quiet "${CARGO_ARGS[@]}")

mkdir -p target/debug
for bin in "${BINARIES[@]}"; do
    cp "$CTX_REPO/target/debug/$bin" "target/debug/$bin"
    echo "OK: installed target/debug/$bin"
done

# Summarizer prompts: ctx-scan/ctx-summarize load these from ./prompts at
# runtime, resolved against cwd (the project root), not the tooling repo.
PROMPTS=(summarizer-leaf.md summarizer-rollup.md)

mkdir -p prompts
for prompt in "${PROMPTS[@]}"; do
    cp "$CTX_REPO/prompts/$prompt" "prompts/$prompt"
    echo "OK: installed prompts/$prompt"
done
