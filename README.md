# CTX - Opinionated Agentic Coding System

An opinionated coding system for autonomous agents: a strict Rust lint
regime the workspace itself is built under, a generated context tree
agents read before editing, and a sandbox for autonomous
runs. Doctrine lives in `.context/intent.md`; rationale in
`docs/DECISIONS.md`; current work in `docs/STATUS.md`.

## Architecture

Below is the generated root context rollup.
<!-- BEGIN GENERATED architecture (scripts/gen_readme_architecture.sh --write) -->
`.` is the root of the CTX workspace: a Rust multi-crate system plus supporting docs/scripts/template that together enforce agentic top-down context reading and (eventually) compiler-level defect prevention. Depending on the repo root means getting the full pipeline — summarize, verify, sandbox, scan, brief — as independent crates, wired together only through file/subprocess boundaries and the on-disk `.context` mirror convention, plus the docs and scripts that keep that pipeline honest.

The load-bearing cross-cutting fact is the `.context` mirror-path convention: `ctx-summarize` writes it, `ctx-scan` independently tracks staleness against it, and `ctx-context` reads it read-only to serve/inject — a layout change here must be checked in all three, not just one crate's rollup. `template/` declares merge-driver ownership over `.context/**/*.ctx` via `.gitattributes` even though no driver implementation exists yet; that gap is real and unresolved. `scripts/` and `crates/ctx-verify` are the enforcement layer — lint scripts use a shared git-enumeration and `FAIL: path:line:` output convention that `ctx-verify`'s parser depends on. `docs/` is pure rationale/status memory, never executed. `agents/` is the one live implementation of the external agent contract (`CTX_AGENT_CMD`) that `ctx-summarize`/`ctx-brief` invoke.

Two invariants declared in intent.md are not yet structurally enforced anywhere visible at this level: no `#[allow]` and no `unsafe` (a workspace-wide lint posture, not verified from this rollup alone) and the Layer 3 architecture-audit is explicitly "deferred" per intent.md itself, so its absence isn't divergence — it's stated as not-yet-built.

Cargo.lock is a pure resolution snapshot with no behavior of its own.

intent_divergence: intent.md states Layer 3 (audit) is deferred and its hooks "exist," but no crate or script in this tree currently produces or consumes a divergence report — the audit mechanism itself is absent, not merely dormant.
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
