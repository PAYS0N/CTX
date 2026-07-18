Doctrine: `.context/intent.md` + `docs/SPEC.md`. Rationale:
`docs/DECISIONS.md`. Current focus: `docs/STATUS.md`.

Call `target/debug/ctx-context .`. Do not skip this. 

- Source: native Read/Edit/Grep — never `cat` for file reads. The context
  chain is hook-injected on read; request it on demand with
  `target/debug/ctx-context <path>` (dir or `.`), and read a directory's
  chain before changing its contents. Go only as deep as needed. Never
  hand-edit `.context/`.
- Do not run cargo yourself; use `target/debug/ctx-verify [crate]`. If
  cargo is unavoidable: `-q --message-format=short`, never paste build dumps.

The tool contracts below are generated from each binary's `--contract`
output — the single source of truth. Regenerate with
`scripts/gen_tool_contracts.sh --write`; `ctx-verify`'s `contracts` check
fails if they drift. Never edit between the markers.

<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->
- **ctx-context** — ctx-context <path> prints the context chain an agent must read before touching <path>: the ancestor rollup.ctx + intent.md at each directory level, plus the file's own leaf .ctx for a file target (`.` targets the repo root). Read-only and fail-open — a missing node renders as an explicit `(absent: …)` marker, never an error; a served summary whose source changed since the last regen is prefixed `[STALE …]`, and one whose source exists but was never summarized `[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event from stdin and emits deduplicated additional-context for the session.
- **ctx-verify** — ctx-verify [crate] is the agent checkpoint: it applies `cargo fmt`, then builds, lints (clippy + rustdoc, warnings denied), tests, and runs the repo's script battery in one call; an optional crate name scopes the cargo-based checks. The default terse render prints the single word `pass` when every check passed, otherwise one FAIL:/ERROR: block per failing check — the `{"status":"pass"}` JSON envelope is emitted only under `--json`. Serving fails open; this gate fails closed.
- **ctx-scan** — ctx-scan <dir> maintains the `.context/` summary tree beside the source, using a content-hash tree (not git) to decide staleness. `--check` reports stale directories and leaves, plus expected summaries that are missing (never generated or hand-deleted), without calling the model; `--update` regenerates only the stale leaves and rollups, then rewrites the hash sidecars; `--dry-run` lists the files in scope; `--stop-hook` reports staleness as a Claude Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration is a post-session concern — the hook never bills the model.
- **ctx-cage** — ctx-cage <target> runs an agent subprocess in an offline sandbox over the target project — bwrap with a masked filesystem, fresh namespaces, and no egress except a proxied API relay — and guarantees teardown. Billed modes (`--task`/`--task-file`, or the interactive default) require `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`; `--self-test stub` is the always-available no-spend, no-network containment probe.
<!-- END GENERATED tool-contracts -->

- Lints: `#[allow]` is banned. unwrap/expect compile only inside
  `#[test]`/`#[cfg(test)]` bodies — test helpers outside them must
  return `Result`. A 30-line fn / 250-line file: refactor first; a
  single-line `// rationale:` directly above (fn) or after `//!`
  (file) is the last resort, and multi-line is not recognized.
- `.env` holds the summarizer key: never feed it to a model, never
  commit it.
- `template/` and root workspace lint configs mirror each other;
  change both.
- Retiring a tool/identifier: add it to `scripts/retired_terms_check.sh`'s
  `BANNED` array and record the ADR in `docs/DECISIONS.md`.
- when debugging, always ensure that the thing you are changing is correct. 
  Do not reach a logical conclusion and change code; verify the issue.
