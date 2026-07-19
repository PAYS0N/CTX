# CTX - Opinionated Agentic Coding System

An opinionated coding system for autonomous agents: a strict Rust lint
regime the workspace itself is built under, a generated context tree
agents read before editing, and a safety-only sandbox for autonomous
runs. Doctrine lives in `.context/intent.md`; rationale in
`docs/DECISIONS.md`; current work in `docs/STATUS.md`.

## Architecture

<!-- BEGIN GENERATED architecture (scripts/gen_readme_architecture.sh --write) -->
`.` is the CTX project root: an agentic coding system whose crates,
scripts, docs, and templates jointly enforce compiler-level defect
prevention, context-before-code ordering, and (eventually) intent-audit
divergence detection. Depending on the repo root gets you the full
pipeline — sandboxed execution (`ctx-cage`), context-chain
resolution/serving (`ctx-context`), staleness scanning (`ctx-scan`),
LLM summarization (`ctx-summarize`), check verification (`ctx-verify`),
their shared library (`ctx-core`), the CI/lint scripts that back
`ctx-verify`, the reference agent adapter, project docs, and the
project-scaffold template.

The core coupling spans three subtrees, not one: `crates/` produces the
binaries; `scripts/` is what `ctx-verify` (in `crates/`) actually
invokes, sharing the `FAIL: path:line: message` convention documented in
`scripts/rollup.ctx`; `template/` re-packages both — it wires
`.claude/settings.json` to `ctx-context` and CI to `scripts/*.sh` — for
downstream projects. A change to a script's failure-line format or a
crate's `--contract` output must be checked against both `ctx-verify`'s
parser and `gen_tool_contracts.sh`. `agents/` supplies the external
`CTX_AGENT_CMD` implementation that `ctx-summarize` shells out to; its
stdin/stdout JSON contract is independent of the crate/script coupling
above but is exercised by the same pipeline. `docs/` and `Cargo.lock` are
inert with respect to this coupling — reference material and dependency
pins only.

Workspace-wide invariants enforced across `crates/`: no `unsafe`, no
`#[allow]`, typed errors only, injected I/O boundaries (`Env`/`Fs`/`Agent`/
`Runner`/clock) — `scripts/no_allow_check.sh` and `workspace_lints_check.sh`
are the actual enforcement mechanism for the first two, making them
load-bearing for the stated Layer 1 goal, not just CI hygiene.

Layer 3 (intent-audit) is stated as deferred in `intent.md`; no crate,
script, or doc here currently performs intent-vs-structure divergence
detection beyond this manually-authored rollup process itself — consistent
with intent, not a divergence.
<!-- END GENERATED architecture -->

## Tools

An agent works through four binaries.

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
