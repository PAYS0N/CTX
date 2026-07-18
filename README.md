# CTX - Opinionated Agentic Coding System

An opinionated coding system for autonomous agents: a strict Rust lint
regime the workspace itself is built under, a generated context tree
agents read before editing, and a safety-only sandbox for autonomous
runs. Doctrine lives in `.context/intent.md`; rationale in
`docs/DECISIONS.md`; current work in `docs/STATUS.md`.

## Architecture

<!-- BEGIN GENERATED architecture (scripts/gen_readme_architecture.sh --write) -->
`.` is the CTX system's repository root: an agentic-coding toolchain that makes bad code uncompilable and forces context-reading before edits, plus the docs, prompts, scripts, and scaffolding that support it. Depending on the repo root gets you the whole toolchain — five Rust crates under `crates/`, prompt files consumed by `ctx-summarize`, verification scripts wired through `ctx-verify`, and a `template/` scaffold for stamping new projects — but no shared code lives at this top level; it's organizational only.

The core lifecycle spans four subtrees and must be read as a chain, not independently: `crates/` implements the binaries; `prompts/` supplies the fixed system-prompt contracts those binaries' agents (`agents/`) feed to an LLM; `scripts/` implements the FAIL:-line checks that `ctx-verify` parses; `docs/` records why decisions were made and what's pending. A convention, not a compiler, holds these together — `ctx-summarize`, `ctx-scan`, and `ctx-context` all read/write the same `.context/` tree independently, and `agents/`'s stdin/stdout JSON contract is mirrored, not shared, between `prompts/`'s auditor.md and any runner.

`README.md`'s generated blocks (tool contracts, architecture) are regenerated from `crates/`'s `--contract` output and this rollup respectively — editing this rollup's shape can ripple into README regeneration. `sandbox/` and parts of `docs/retired/` are retired: functionality fully migrated into `crates/ctx-cage`, kept only as documentation/history. `template/`'s scaffold assumes a sibling tooling repo (`install-tools.sh`) supplies the binaries it wires up, an external dependency not visible elsewhere in this tree.

Layer 3 (architecture audit) is still deferred per intent.md — `prompts/auditor.md` and the `intent_divergence:` label exist, but no automated run of the audit against live rollups is wired into CI yet.

intent_divergence: intent states Layer 3 (audit) hooks "exist" but only the prompt and label convention are present — no CI or script actually invokes the auditor against generated rollups.
<!-- END GENERATED architecture -->

## Tools

An agent works through four binaries.

<!-- BEGIN GENERATED tool-contracts (scripts/gen_tool_contracts.sh --write) -->
- **ctx-context** — ctx-context <path> prints the context chain an agent must read before touching <path>: the ancestor rollup.ctx + intent.md at each directory level, plus the file's own leaf .ctx for a file target (`.` targets the repo root). Read-only and fail-open — a missing node renders as an explicit `(absent: …)` marker, never an error; a served summary whose source changed since the last regen is prefixed `[STALE …]`, and one whose source exists but was never summarized `[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event from stdin and emits deduplicated additional-context for the session.
- **ctx-verify** — ctx-verify [crate] is the agent checkpoint: it applies `cargo fmt`, then builds, lints (clippy + rustdoc, warnings denied), tests, and runs the repo's script battery in one call; an optional crate name scopes the cargo-based checks. The default terse render prints the single word `pass` when every check passed, otherwise one FAIL:/ERROR: block per failing check — the `{"status":"pass"}` JSON envelope is emitted only under `--json`. Serving fails open; this gate fails closed.
- **ctx-scan** — ctx-scan <dir> maintains the `.context/` summary tree beside the source, using a content-hash tree (not git) to decide staleness. `--check` reports stale directories and leaves, expected summaries that are missing (never generated or hand-deleted), and orphaned artifacts (summaries/sidecars whose source was deleted or scoped out), without calling the model; `--prune` deletes the orphaned artifacts and sweeps emptied mirror directories, also model-free; `--update` prunes, regenerates only the stale leaves and rollups, then rewrites the hash sidecars (`intent.md` and `.context/.cache|.reports` are never pruned); `--dry-run` lists the files in scope; `--stop-hook` reports staleness as a Claude Code Stop `systemMessage` and always exits 0 (fail-open). Regeneration is a post-session concern — the hook never bills the model.
- **ctx-cage** — ctx-cage <target> runs an agent subprocess in an offline sandbox over the target project — bwrap with a masked filesystem, fresh namespaces, and no egress except a proxied API relay — and guarantees teardown. Billed modes (`--task`/`--task-file`, or the interactive default) require `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`; `--self-test stub` is the always-available no-spend, no-network containment probe.
<!-- END GENERATED tool-contracts -->

Summaries are regenerated by `ctx-scan --update`, which drives the
summarization agent over `prompts/summarizer-leaf.md` and
`prompts/summarizer-rollup.md`.
