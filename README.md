# CTX - Opinionated Agentic Coding System

An opinionated coding system for autonomous agents: a strict Rust lint
regime the workspace itself is built under, a generated context tree
agents read before editing, and a sandbox for autonomous
runs. Doctrine lives in `.context/intent.md`; rationale in
`docs/DECISIONS.md`; current work in `docs/STATUS.md`.

## Architecture

Below is the generated root context rollup.
<!-- BEGIN GENERATED architecture (scripts/gen_readme_architecture.sh --write) -->
This is the CTX system root: an agentic-coding enforcement tool whose crates, scripts, docs, and template together implement the three standing goals — compiler-level enforcement (no `unsafe`/`#[allow]`), context-before-source (the `.context/` rollup/leaf chain this very file is part of), and (deferred) intent-divergence audit. A reader depending on the repo root gets a Cargo workspace of independent binaries plus the CI/doc machinery that keeps their behavior and documentation synchronized; nothing here is a shared library — coupling is by convention and file contract, not code.

The core cross-cutting coupling: `crates/` implements the `.context/` mirror (scan → summarize → serve/verify/brief), `scripts/` enforces the lint/architecture invariants those crates are supposed to embody (no-allow, no-cycles, retired-terms, workspace-lints) and also regenerates docs from the built binaries, and `template/` re-ships that same `scripts/` subtree into downstream projects — a convention change to `.context/` naming or layout must move through crates, scripts, and template's copy of scripts together, and none of these enforce that consistency mechanically across each other. `docs/` is the only place recording *why* and *what's not done*, and explicitly goes stale silently (ADRs superseded by later ADRs, STATUS.md line-refs unvalidated) — an editor must cross-check it, not trust it. `agents/` is the one live example of the external `Agent` boundary (stdin/stdout JSON contract) that `ctx-core`'s trait seam requires any backend to satisfy.

Invariant spanning the tree: every "must hold" architectural rule in intent.md (no unsafe, no allow, typed errors, injected boundaries, prompts as files, fail-open serving vs fail-closed gating) is enforced piecemeal by `scripts/` checks and crate-internal design, with no single compiler-checked source of truth tying them together — this is itself the acknowledged gap Layer 3 (audit) is meant to eventually close.

Layer 3 (architecture audit) is stated as deferred in intent.md and no crate or script here performs actual intent-vs-structure adjudication yet — consistent with intent, not a divergence.
<!-- END GENERATED architecture -->

## Tools

An agent works through the below binaries.

<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->
- **ctx-context** — ctx-context <path> prints the context chain an agent must read before touching <path>: the ancestor rollup.ctx + intent.md at each directory level, plus the file's own leaf .ctx for a file target (`.` targets the repo root). Read-only and fail-open — a missing node renders as an explicit `(absent: …)` marker, never an error; a served summary whose source changed since the last regen is prefixed `[STALE …]`, and one whose source exists but was never summarized `[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event from stdin and emits deduplicated additional-context for the session.
- **ctx-verify** — ctx-verify [crate] is the agent checkpoint: it applies `cargo fmt`, then builds, lints (clippy + rustdoc, warnings denied), tests, and runs the repo's script battery in one call; an optional crate name scopes the cargo-based checks. The default terse render prints the single word `pass` when every check passed, otherwise one FAIL:/ERROR:/SKIP: block per failing, errored, or skipped (missing-tool) check — a skipped check is never a silent pass — and the `{"status":"pass"}` JSON envelope is emitted only under `--json` and only when every check passed. Serving fails open; this gate fails closed.
- **ctx-scan** — ctx-scan <dir> maintains the `.context/` summary tree beside the source, using a content-hash tree (not git) to decide staleness. `--check` reports stale directories and leaves, expected summaries that are missing (never generated or hand-deleted), and orphaned artifacts (summaries/sidecars whose source was deleted or scoped out), without calling the model; `--prune` deletes the orphaned artifacts and sweeps emptied mirror directories, also model-free; `--update` prunes, regenerates only the stale leaves and rollups, then rewrites the hash sidecars (`intent.md` and `.context/.cache|.reports` are never pruned); `--dry-run` lists the files in scope; `--stop-hook` reports staleness as a Claude Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration is a post-session concern — the hook never bills the model.
- **ctx-cage** — ctx-cage <target> runs an agent subprocess in an offline sandbox over the target project — bwrap with a masked filesystem, fresh namespaces, and no egress except a proxied API relay — and guarantees teardown. Billed modes (`--task`/`--task-file`, or the interactive default) require `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`; `--self-test stub` is the always-available no-spend, no-network containment probe.
- **ctx-brief** — ctx-brief [--headless] <request> turns a `docs/STATUS.md` backlog item — matched as a case-insensitive substring of the task column, or the raw text when nothing matches — into a self-contained task brief for `ctx-cage --task-file`. It runs two subscription-billed `claude` stages inside the target repo so that repo's own context hooks ground every read: a cheap read-only gather pass (`--gather-model`, default haiku) produces a verified dossier (state, constraints, waypoints, unknowns), then a plan pass composes the brief — interviewing the human on open decisions by default, or (`--headless`) adjudicating tactical decisions itself and escalating doctrinal ones. The brief is written to `.context/.reports/briefs/<slug>.md` (never pruned by ctx-scan) unless `--out` overrides it, and its path is printed for the `ctx-cage` hand-off.
<!-- END GENERATED tool-contracts -->

Summaries are regenerated by `ctx-scan --update`, which drives the
summarization agent over `prompts/summarizer-leaf.md` and
`prompts/summarizer-rollup.md`.
