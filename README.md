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
`.` is the workspace root for CTX, a dogfood system enforcing two layers: an uncompromising Rust lint/test regime applied to its own tooling, and a `.ctx`/`rollup.ctx` context pipeline that agents must read top-down before touching source. This directory itself holds only workspace-level config (Cargo.toml, clippy.toml) plus the top-level subtrees; no source code lives directly here.

`Cargo.toml` defines the lint table that `template/Cargo.toml` must mirror byte-for-byte — the two are locked together by the dogfood invariant, and drift between them means the template stops enforcing what the real workspace enforces. `clippy.toml` is the single source of truth for numeric lint thresholds, with soft tiers (line count, cognitive complexity) enforced instead by `scripts/rationale_check.py` and a not-yet-built dylint crate — three enforcement points for two thresholds, documented in `docs/DYLINT_RULES.md`.

`crates/` is where the actual `.context/` tooling lives (ctx-summarize, ctx-scan, ctx-context, ctx-verify, ctx-cage, ctx-brief, ctx-status, ctx-core), each an independent crate with hermetic fakes in its own tests. `scripts/` backs `ctx-verify` with standalone FAIL-format checks and README/CLAUDE.md doc-generators. `agents/` is the model-invocation boundary the summarization pipeline calls through a subprocess JSON contract. `template/` is the scaffold distributed to downstream projects, carrying a copy of the check-script/generator contract plus `.gitattributes` merge policy that must stay in sync with however `.ctx` layout is actually produced. `docs/` holds the ADR log and forward-looking lint spec, read directly rather than summarized further. `.cagevars.example.ctx` documents the sandbox's env-var contract for `ctx-cage`.

A change to the lint regime starts at `Cargo.toml` and must propagate to `template/Cargo.toml`; a change to `.ctx`/`rollup.ctx` file layout starts in `ctx-scan`/`ctx-summarize` and must propagate to `template/.gitattributes`'s merge-driver patterns and `scripts/gen_readme_architecture.sh`'s prerequisites. The cli-over-core-over-injected-boundaries invariant and the fail-open-context/fail-closed-gate invariant are both stated in intent.md and implemented per-crate in `crates/`, not re-asserted here.
<!-- END GENERATED architecture -->

## Tools

An agent works through the below binaries.

<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->
- **ctx-context** — ctx-context <path> prints the context chain an agent must read before touching <path>: the ancestor rollup.ctx + intent.md at each directory level, plus the file's own leaf .ctx for a file target (`.` targets the repo root). Read-only and fail-open — a missing node renders as an explicit `(absent: …)` marker, never an error; a served summary whose source changed since the last regen is prefixed `[STALE …]`, and one whose source exists but was never summarized `[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event from stdin and emits deduplicated additional-context for the session.
- **ctx-verify** — ctx-verify [crate] is the agent checkpoint: it applies `cargo fmt`, then builds, lints (clippy + rustdoc, warnings denied), tests, and runs the repo's script battery in one call; an optional crate name scopes the cargo-based checks. The default terse render prints the single word `pass` when every check passed, otherwise one FAIL:/ERROR:/SKIP: block per failing, errored, or skipped (missing-tool) check — a skipped check is never a silent pass — and the `{"status":"pass"}` JSON envelope is emitted only under `--json` and only when every check passed. Serving fails open; this gate fails closed.
- **ctx-scan** — ctx-scan <dir> maintains the `.context/` summary tree beside the source, using a content-hash tree (not git) to decide staleness. `--check` reports stale directories and leaves, expected summaries that are missing (never generated or hand-deleted), and orphaned artifacts (summaries/sidecars whose source was deleted or scoped out), without calling the model; `--prune` deletes the orphaned artifacts and sweeps emptied mirror directories, also model-free; `--update` prunes, regenerates only the stale leaves and rollups, then rewrites the hash sidecars (`intent.md` and `.context/.cache|.reports` are never pruned); `--dry-run` lists the files in scope; `--stop-hook` reports staleness as a Claude Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration is a post-session concern — the hook never bills the model.
- **ctx-cage** — ctx-cage <target> runs an agent subprocess in an offline sandbox over the target project — bwrap with a masked filesystem, fresh namespaces, and no egress except a proxied API relay — and guarantees teardown. Billed modes (`--task`/`--task-file`, or the interactive default) require `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`; `--self-test stub` is the always-available no-spend, no-network containment probe.
- **ctx-brief** — ctx-brief [--headless] (<request> | --id <id>) turns a backlog item into a self-contained task brief for `ctx-cage --task-file`. `<request>` is matched as a case-insensitive substring of `docs/STATUS.md`'s task column, or used as the raw text when nothing matches; `--id <id>` instead looks the row up directly by its stable id in `docs/status.json` (the only place that id lives), erroring if none matches. Exactly one of `<request>`/`--id` is required. It runs two subscription-billed `claude` stages inside the target repo so that repo's own context hooks ground every read: a cheap read-only gather pass (`--gather-model`, default haiku) produces a verified dossier (state, constraints, waypoints, unknowns), then a plan pass composes the brief — interviewing the human on open decisions by default, or (`--headless`) adjudicating tactical decisions itself and escalating doctrinal ones. The brief is written to `.context/.reports/briefs/<slug>.md` (`item-<id>.md` when resolved by `--id`; never pruned by ctx-scan) unless `--out` overrides it, and its path is printed for the `ctx-cage` hand-off.
- **ctx-status** — ctx-status list prints the current backlog from the JSON store at `docs/status.json` (source of truth), each row prefixed with its id, sorted by impact (high → low) then difficulty (easy → hard) within each band — the on-demand way an agent surfaces priorities, no hook required. `ctx-status add-task <description> --task <title> --impact <high|medium|low> --difficulty <easy|medium|hard>` appends one row under a freshly assigned id — never reordering or editing existing ones — and regenerates `docs/STATUS.md` from the store in the same step, so the human-readable view can never drift from what the store holds. `--task` is required: omitting it would otherwise duplicate the full description into the task column too. `ctx-status delete-task <id>` removes the row with that id — run `list` first to find it — refusing if no row has it, and regenerates `docs/STATUS.md` in the same step. Ids are internal to `ctx-status`: they never appear in `docs/STATUS.md` itself, whose 4-column shape stays shared with `ctx-brief`.
<!-- END GENERATED tool-contracts -->

Summaries are regenerated by `ctx-scan --update`, which drives the
summarization agent over `prompts/summarizer-leaf.md` and
`prompts/summarizer-rollup.md`.
