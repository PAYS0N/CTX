#!/usr/bin/env bash
# Detect circular module dependencies within a crate.
#
# Cargo itself rejects circular *crate* deps. This script catches circular
# *module* deps within a single crate, which cargo allows but the spec
# forbids.
#
# Uses `cargo modules` (the cargo-modules subcommand). If not installed:
#     cargo install cargo-modules
#
# This will move to dylint rule 4 (see docs/DYLINT_RULES.md).

set -euo pipefail

if ! command -v cargo-modules >/dev/null 2>&1; then
    echo "FAIL: cargo-modules not installed; run: cargo install cargo-modules" >&2
    exit 1
fi

FAIL=0

# Run for each crate in the workspace. The `--no-fns --no-types --no-traits`
# flags would keep output focused on module-level deps; current cargo-modules
# versions vary. The presence of any output in `--cycles` mode is failure.
if ! out=$(cargo modules dependencies --no-externs --no-sysroot --no-uses 2>&1); then
    echo "FAIL: cargo modules failed:" >&2
    echo "$out" >&2
    FAIL=1
fi

# cargo-modules does not natively report "cycle: yes/no". A graph-cycle
# detector would parse the DOT output. For MVP, this script is a stub that
# verifies cargo-modules runs cleanly; cycle detection itself is part of the
# deferred work in dylint rule 4.
echo "NOTE: cycle detection itself is not implemented at MVP." >&2
echo "      Only confirming cargo-modules runs successfully." >&2

exit $FAIL
