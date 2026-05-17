#!/usr/bin/env bash
# Run all Layer 1 checks. This is the entry point for CI and pre-commit.
#
# Any failure exits non-zero. Output is the union of all failing checks
# (it does not stop at the first failure), so an agent sees the complete
# picture in one run.

set -uo pipefail

ROOT="${1:-.}"
cd "$ROOT"

OVERALL=0
PHASE() { echo; echo "=== $1 ==="; }
RUN() {
    local name=$1; shift
    if "$@"; then
        echo "OK: $name"
    else
        echo "FAIL: $name" >&2
        OVERALL=1
    fi
}

SCRIPTS_DIR="$(cd "$(dirname "$0")" && pwd)"

PHASE "Formatting"
RUN "cargo fmt --check" cargo fmt --all -- --check

PHASE "Compiler + clippy"
RUN "cargo clippy" env RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features -- -D warnings

PHASE "Documentation"
RUN "cargo doc" env RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace

PHASE "Dependency policy"
RUN "cargo deny check" cargo deny check
RUN "cargo machete" cargo machete

PHASE "Rationale comments (soft tiers)"
RUN "rationale_check.py" python3 "$SCRIPTS_DIR/rationale_check.py" .

PHASE "Workspace lints opt-in"
RUN "workspace_lints_check.sh" bash "$SCRIPTS_DIR/workspace_lints_check.sh" .

PHASE "No #[allow] attributes"
RUN "no_allow_check.sh" bash "$SCRIPTS_DIR/no_allow_check.sh" .

PHASE "Module cycle check"
RUN "cycle_check.sh" bash "$SCRIPTS_DIR/cycle_check.sh"

echo
if [[ $OVERALL -eq 0 ]]; then
    echo "All checks passed."
else
    echo "One or more checks failed." >&2
fi
exit $OVERALL
