# CTX - Opinionated Agentic Coding System

An opinionated coding system for autonomous agents: a strict Rust lint
regime the workspace itself is built under, a generated context tree
agents read before editing, and a sandbox for autonomous
runs. Doctrine lives in `.context/intent.md`; rationale in
`docs/DECISIONS.md`; current work in `docs/STATUS.md`.

## Architecture

<!-- BEGIN GENERATED architecture (scripts/gen_readme_architecture.sh --write) -->
`.` is the CTX toolchain itself: an agentic-coding enforcement system built as a Rust workspace (`crates/`) plus supporting layers — an agent adapter (`agents/`), rationale/planning docs (`docs/`), CI/lint scripts (`scripts/`), and a project scaffold for consumers (`template/`). Depending on this root means picking one of the published binaries (`ctx-scan`, `ctx-summarize`, `ctx-context`, `ctx-verify`, `ctx-cage`, `ctx-brief`) or adopting the `template/` scaffold in a new project; there is no root-level code, only workspace wiring (`Cargo.lock`).

The load-bearing cross-cutting coupling is the `.ctx`/`rollup.ctx` file format: produced by `ctx-summarize`, tracked for staleness by `ctx-scan`, served read-only by `ctx-context`, and consumed by hooks in `template/.claude/settings.json` — a format or path change must be checked against all four. A second cross-cutting convention, independent of the first, is the `FAIL: path:line: message` stderr contract shared by everything under `scripts/` and mirrored by `template/scripts/`, both parsed by `ctx-verify`'s `split_loc`.

`docs/DECISIONS.md` is the append-only source of truth for *why* the architecture looks like this (e.g. the cage→hook-injected sandboxing pivot); `docs/STATUS.md` tracks current state against it and can go stale independently. `agents/` defines the external `CTX_AGENT_CMD` contract (stdin/stdout JSON, non-zero exit on failure) that `ctx-brief` and any future agent-backed crate must honor.

Layer 1 (no-`unsafe`/no-`allow`/typed-errors) and Layer 2 (context-before-source, regenerated rollups) from intent.md are both live and enforced across `crates/`; Layer 3 (intent-divergence audit) is explicitly deferred, matching intent.md's own admission — no divergence to report.
<!-- END GENERATED architecture -->

## Tools

An agent works through the below binaries.

<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->
- **ctx-context** — ctx-context <path> prints the context chain an agent must read before touching <path>: the ancestor rollup.ctx + intent.md at each directory level, plus the file's own leaf .ctx for a file target (`.` targets the repo root). Read-only and fail-open — a missing node renders as an explicit `(absent: …)` marker, never an error; a served summary whose source changed since the last regen is prefixed `[STALE …]`, and one whose source exists but was never summarized `[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event from stdin and emits deduplicated additional-context for the session.
- **ctx-verify** — ctx-verify [crate] is the agent checkpoint: it applies `cargo fmt`, then builds, lints (clippy + rustdoc, warnings denied), tests, and runs the repo's script battery in one call; an optional crate name scopes the cargo-based checks. The default terse render prints the single word `pass` when every check passed, otherwise one FAIL:/ERROR: block per failing check — the `{"status":"pass"}` JSON envelope is emitted only under `--json`. Serving fails open; this gate fails closed.
- **ctx-scan** — ctx-scan <dir> maintains the `.context/` summary tree beside the source, using a content-hash tree (not git) to decide staleness. `--check` reports stale directories and leaves, expected summaries that are missing (never generated or hand-deleted), and orphaned artifacts (summaries/sidecars whose source was deleted or scoped out), without calling the model; `--prune` deletes the orphaned artifacts and sweeps emptied mirror directories, also model-free; `--update` prunes, regenerates only the stale leaves and rollups, then rewrites the hash sidecars (`intent.md` and `.context/.cache|.reports` are never pruned); `--dry-run` lists the files in scope; `--stop-hook` reports staleness as a Claude Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration is a post-session concern — the hook never bills the model.
- **ctx-cage** — ctx-cage <target> runs an agent subprocess in an offline sandbox over the target project — bwrap with a masked filesystem, fresh namespaces, and no egress except a proxied API relay — and guarantees teardown. Billed modes (`--task`/`--task-file`, or the interactive default) require `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`; `--self-test stub` is the always-available no-spend, no-network containment probe.
- **ctx-brief** — ctx-brief [--headless] <request> turns a `docs/STATUS.md` backlog item — matched as a case-insensitive substring of the task column, or the raw text when nothing matches — into a self-contained task brief for `ctx-cage --task-file`. It runs two subscription-billed `claude` stages inside the target repo so that repo's own context hooks ground every read: a cheap read-only gather pass (`--gather-model`, default haiku) produces a verified dossier (state, constraints, waypoints, unknowns), then a plan pass composes the brief — interviewing the human on open decisions by default, or (`--headless`) adjudicating tactical decisions itself and escalating doctrinal ones. The brief is written to `.context/.reports/briefs/<slug>.md` (never pruned by ctx-scan) unless `--out` overrides it, and its path is printed for the `ctx-cage` hand-off.
<!-- END GENERATED tool-contracts -->

Summaries are regenerated by `ctx-scan --update`, which drives the
summarization agent over `prompts/summarizer-leaf.md` and
`prompts/summarizer-rollup.md`.
