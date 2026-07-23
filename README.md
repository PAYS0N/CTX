# CTX - Opinionated Agentic Coding System

An opinionated coding system for autonomous agents: a strict Rust lint
regime the workspace itself is built under, a generated context tree
agents read before editing, and a sandbox for autonomous
runs. Doctrine lives in `.context/intent.md`; rationale in
`docs/DECISIONS.md`; current work in `docs/STATUS.md`.

The core context management system walks the full repo (ignoring .ctxignore)
and creates a context file for each leaf file. Then for every
directory, a context file is created using every leaf and dir summary
within it.

# Architecture

Below is the final context file for the whole repository.

<!-- BEGIN GENERATED architecture (scripts/gen_readme_architecture.sh --write) -->
`.` is the workspace root of CTX: an agentic coding system that (1)
compiles its own lint/compiler regime hard enough to make bad code
uncompilable rather than merely flagged, and (2) generates and serves a
`.ctx`/`rollup.ctx` context tree so agents read top-down before touching
source. The root itself carries no logic — it's the lint contract, the
crate family, and the supporting scripts/docs/template that implement
those two goals.

`Cargo.toml` and `clippy.toml` together are the enforcement layer: the
workspace lint table (`unsafe_code` forbid, `unwrap_used`/`panic`/`exit`
deny, no suppression) and the thresholds clippy alone can't express
(soft-tier line/complexity limits deferred to `scripts/rationale_check.py`
and a future dylint crate). `crates/` is where that regime is dogfooded —
eight crates implementing the context pipeline (scan → summarize → serve)
plus `ctx-cage` for sandboxed agent execution — sharing on-disk contracts
(`.ctx` layout, `docs/status.json`) rather than code. `agents/` is the
model-invocation seam those crates call through, one adapter deep so far.
`scripts/` backs `ctx-verify` with independently-invocable checks sharing
a `FAIL:`-output/exit-code protocol. `template/` re-exports that same
`scripts/` contract plus `.context/` merge policy into every downstream
project. `docs/` is the ADR log and forward-looking lint spec, read
directly rather than rolled up further.

Editing the lint table means mirroring `Cargo.toml`'s `[workspace.lints]`
into `template/Cargo.toml` by hand — no automation enforces that mirror.
Changing the `.ctx`/`rollup.ctx` on-disk format touches `ctx-scan`,
`ctx-summarize`, `ctx-context`, and `template/.gitattributes`'s merge
driver patterns simultaneously; there is no single owner. `ctx-cage`'s
`.cagevars` config is the one file governing agent sandbox environment,
separate from `.env`'s single `ANTHROPIC_API_KEY`.
<!-- END GENERATED architecture -->

## Tools

An agent works through the below binaries.

<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->
- **ctx-context** — ctx-context <path> prints the context chain an agent must read before touching <path>: the ancestor rollup.ctx + intent.md at each directory level, plus the file's own leaf .ctx for a file target (`.` targets the repo root). Read-only and fail-open — a missing node renders as an explicit `(absent: …)` marker, never an error; a served summary whose source changed since the last regen is prefixed `[STALE …]`, and one whose source exists but was never summarized `[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event from stdin and emits deduplicated additional-context for the session.
- **ctx-verify** — ctx-verify [crate] is the agent checkpoint: it applies `cargo fmt`, then builds, lints (clippy + rustdoc, warnings denied), tests, and runs the repo's script battery in one call; an optional crate name scopes the cargo-based checks. The default terse render prints the single word `pass` when every check passed, otherwise one FAIL:/ERROR:/SKIP: block per failing, errored, or skipped (missing-tool) check — a skipped check is never a silent pass — and the `{"status":"pass"}` JSON envelope is emitted only under `--json` and only when every check passed. Serving fails open; this gate fails closed.
- **ctx-scan** — ctx-scan <dir> maintains the `.context/` summary tree beside the source, using a content-hash tree (not git) to decide staleness. `--check` reports stale directories and leaves, expected summaries that are missing (never generated or hand-deleted), and orphaned artifacts (summaries/sidecars whose source was deleted or scoped out), without calling the model; `--prune` deletes the orphaned artifacts and sweeps emptied mirror directories, also model-free; `--update` prunes, regenerates only the stale leaves and rollups, then rewrites the hash sidecars (`intent.md` and `.context/.cache|.reports` are never pruned); `--dry-run` lists the files in scope; `--stop-hook` reports staleness as a Claude Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration is a post-session concern — the hook never bills the model.
- **ctx-cage** — ctx-cage <target> runs an agent subprocess in an offline sandbox over the target project — bwrap with a masked filesystem, fresh namespaces, and no egress except a proxied API relay — and guarantees teardown. Billed modes (`--task`/`--task-file`, or the interactive default) require `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`; `--self-test stub` is the always-available no-spend, no-network containment probe.
- **ctx-brief** — ctx-brief [--headless] <request> turns a `docs/STATUS.md` backlog item — matched as a case-insensitive substring of the task column, or the raw text when nothing matches — into a self-contained task brief for `ctx-cage --task-file`. It runs two subscription-billed `claude` stages inside the target repo so that repo's own context hooks ground every read: a cheap read-only gather pass (`--gather-model`, default haiku) produces a verified dossier (state, constraints, waypoints, unknowns), then a plan pass composes the brief — interviewing the human on open decisions by default, or (`--headless`) adjudicating tactical decisions itself and escalating doctrinal ones. The brief is written to `.context/.reports/briefs/<slug>.md` (never pruned by ctx-scan) unless `--out` overrides it, and its path is printed for the `ctx-cage` hand-off.
- **ctx-status** — ctx-status list prints the current backlog from the JSON store at `docs/status.json` (source of truth), sorted by impact (high → low) then difficulty (easy → hard) within each band — the on-demand way an agent surfaces priorities, no hook required. `ctx-status add-task <description> --task <title> --impact <high|medium|low> --difficulty <easy|medium|hard>` appends one row — never reordering, editing, or deleting existing ones, preserving operator curation authority — and regenerates `docs/STATUS.md` from the store in the same step, so the human-readable view can never drift from what the store holds. `--task` is required: omitting it would otherwise duplicate the full description into the task column too.
<!-- END GENERATED tool-contracts -->

Summaries are regenerated by `ctx-scan --update`, which drives the
summarization agent over `prompts/summarizer-leaf.md` and
`prompts/summarizer-rollup.md`.
