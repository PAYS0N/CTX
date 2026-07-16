#!/usr/bin/env bash
# Reject unused dependencies via cargo-machete.
#
# cargo-machete parses each crate's Cargo.toml and greps its sources for a
# use of every declared dependency; one never referenced is reported as
# unused. It reads only manifests + source (no `cargo metadata`, no
# network), so it runs in the same offline context as the other static
# checks — unlike `cargo deny`, which needs the full resolved graph.
#
# A missing tool is a loud FAIL, not a skip: "no unused dependencies" is
# stated project policy (docs/SPEC.md, Dependency policy), so silently
# mapping the missing binary to `skipped` would make ctx-verify's pass
# misleading. Same posture as cycle_check.sh.

set -euo pipefail

ROOT="${1:-.}"
cd "$ROOT"

if ! command -v cargo-machete >/dev/null 2>&1; then
    echo "FAIL: cargo-machete not installed; run: cargo install cargo-machete" >&2
    exit 1
fi

# cargo-machete exits non-zero both when it finds unused deps and on a real
# error. Capture output either way, then translate its per-dependency report
# into one FAIL: line each. Report shape:
#     <crate> -- <path/Cargo.toml>:
#     \t<unused-dep>
rc=0
out="$(cargo machete 2>&1)" || rc=$?

FAIL=0
while IFS= read -r line; do
    echo "FAIL: $line" >&2
    FAIL=1
done < <(
    awk '
        /^[^[:space:]].* -- .*:$/ { sub(/:$/, "", $0); path = $NF; next }
        /^\t/ {
            gsub(/^\t+/, "", $0)
            if (path != "") print path " unused dependency \x27" $0 "\x27"
        }
    ' <<<"$out"
)

# cargo-machete failed but produced no parseable unused-dependency line
# (e.g. a manifest it could not read): surface a summary so the failure is
# never silent, consistent with the other wrapped checks.
if [[ $rc -ne 0 && $FAIL -eq 0 ]]; then
    echo "FAIL: cargo machete failed (see: cargo machete)" >&2
    FAIL=1
fi

exit $FAIL
