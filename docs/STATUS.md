# Status

Last updated: 2026-07-18. Architecture is derived: `ctx-context .`.
Rationale: `docs/DECISIONS.md`.

| task | description | difficulty |
|---|---|---|
| phase 5 e2e smoke fixture | throwaway workspace, buggy file + failing test, generated tree, `--stub` and billed modes; asserts hook injection, native Edit, `ctx-verify` pass, post-session rollup regen, no egress beyond proxy, no writes outside workspace | hard |
| surface api errors from cage | user reports occasional issues with the cage; add clearer errors as a first step | easy |
| fix ctx-scan flags | --dry-run and --check return different vals. Should --dry-run be retired? What is the difference? | medium |
