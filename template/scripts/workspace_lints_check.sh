#!/usr/bin/env bash
# Assert every member crate opts into the workspace lint table.
#
# The workspace Cargo.toml's [workspace.lints] table is the single source
# of truth for the entire restriction lint set. That table binds ONLY to
# member crates that declare:
#
#     [lints]
#     workspace = true
#
# A member crate missing this line silently receives NONE of the lint
# enforcement. This check fails CI for any such crate.
#
# Heuristic, consistent with the other scripts here: any Cargo.toml that
# declares a [package] table (i.e. is a real crate, not the virtual
# workspace root) must contain a [lints] table whose body sets
# `workspace = true`. The eventual dylint pipeline does not replace this;
# it is a manifest property, not a source property.
#
# Manifests are enumerated via `git ls-files`, not a filesystem walk, so
# the inspected set is a pure function of repo state and never races a
# concurrent `target/` writer (see no_allow_check.sh for the rationale).

set -euo pipefail

ROOT="${1:-.}"
cd "$ROOT"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "FAIL: workspace_lints_check.sh: '$ROOT' is not a git repository" >&2
    exit 1
fi

FAIL=0

mapfile -d '' -t manifests < <(
    git ls-files -z --cached --others --exclude-standard \
        -- 'Cargo.toml' '*/Cargo.toml'
)

for manifest in ${manifests[@]+"${manifests[@]}"}; do
    [[ "$(basename "$manifest")" == Cargo.toml ]] || continue

    if ! grep -qE '^\[package\]' "$manifest"; then
        # Virtual workspace root or non-package manifest; skip.
        continue
    fi

    # State machine: confirm a `workspace = true` line appears while inside
    # the [lints] table specifically (not [workspace] or any other table).
    if ! awk '
        /^\[lints\]/                 { in_lints = 1; next }
        /^\[/                        { in_lints = 0 }
        in_lints && /^[[:space:]]*workspace[[:space:]]*=[[:space:]]*true/ { found = 1 }
        END                          { exit(found ? 0 : 1) }
    ' "$manifest"; then
        echo "FAIL: $manifest missing '[lints]\\nworkspace = true'" >&2
        FAIL=1
    fi
done

exit $FAIL
