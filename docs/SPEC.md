# MVP Specification — Opinionated Agentic Coding System

Status: sealed. Changes require explicit re-spec.

Revision 2 (2026-05-16, owner-authorized re-spec). Changes from rev 1:
sandbox dependency made explicit (Layer 2 advisory until deployed; see
`docs/SANDBOX.md`); module-cycle detection corrected to deferred; rollup
budget set to 15/40; `ctx-access read` changed from N step-calls to one
bundled call; task lifecycle (`init-task`/`end-task`) and per-task cache
schema specified; audit report schema aligned to `prompts/auditor.md`;
crate-root deny mechanism corrected to workspace lints; `cargo doc`
enforcement mechanism corrected; prompt-file references corrected.

Revision 3 (2026-05-17, owner-authorized). `started_at` is unix epoch
seconds (not RFC3339); `--shallow` serves through the leaf and stops
before source; `write` evicts the path's leaf+source from `served_nodes`
so the stale banner is reachable; `clippy.toml` gains
`allow-unwrap/expect-in-tests` (the only mechanism for the spec's test
exemption given `#[allow]` is banned). New component `ctx-verify` (a
token-frugal verification broker) is sequenced BEFORE the reference
project; see `docs/UNIMPLEMENTED.md`.

## Goals

1. Make bad code unrepresentable or uncompilable, not merely discouraged.
2. Force agents to read project context top-down before acting on any file.
3. Lay foundations for later architectural auditing and concurrent multi-agent
   use without paying those costs now.

## Non-goals (MVP)

- Human ergonomics. This is for autonomous agents.
- Lint suppression/appeal mechanism.
- Editing `intent.md` after directory creation.

## Layer 1 — Language and Enforcement

### Toolchain

- Rust, stable channel, pinned via `rust-toolchain.toml`.
- Cargo workspace with workspace-wide lint config in `clippy.toml`.
- All checks block in CI. No local overrides.

### Compiler

- `RUSTFLAGS="-D warnings"`.
- `missing_docs` and `unsafe_code` are denied/forbidden via the
  `[workspace.lints]` table in the workspace `Cargo.toml`, not via
  per-crate-root `#![...]` attributes. This table only binds to member
  crates that declare `[lints] workspace = true`. Every member crate MUST
  declare it; CI fails any member that does not (see
  `scripts/workspace_lints_check.sh`). This is the single source of truth —
  do not also add crate-root attributes.

### Clippy

Enabled groups (deny): `clippy::all`, `clippy::pedantic`, `clippy::nursery`.

Selected `restriction` lints (deny):

- `unwrap_used`, `expect_used`
- `panic`, `todo`, `unimplemented`, `unreachable`
- `dbg_macro`, `print_stdout`, `print_stderr`
- `as_conversions`
- `float_arithmetic`
- `mem_forget`, `exit`, `get_unwrap`, `indexing_slicing`
- `missing_docs_in_private_items`
- `string_slice`
- `shadow_unrelated`, `shadow_same`

Exceptions:

- `unwrap_used`, `expect_used` allowed in `#[cfg(test)]` modules and `tests/`.
- `print_stdout`, `print_stderr` allowed in `bin` crate `main.rs` only.
- No `#[allow(...)]` attributes elsewhere. Enforced by grep in CI.

### Thresholds

- Function length: 80 lines hard fail (`too_many_lines` = 80).
- Cognitive complexity: 25 hard fail (`cognitive_complexity` = 25).
- Function length 30+ requires `// rationale: <text>` immediately preceding
  the function. Enforced by pre-commit script.
- File length: 400 lines hard fail, 250+ requires `// rationale:` at file top.
  Both enforced by pre-commit script.
- Complexity soft tier (15) is deferred to the custom dylint crate; the
  hard tier (25) is the only complexity enforcement at MVP.

### Dependency policy

- `cargo-deny` with allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause,
  ISC, Unicode-DFS-2016. Zero advisories. No duplicate versions.
- `cargo-machete`: no unused dependencies.
- No circular module deps within a crate. NOT enforced at MVP: the
  `scripts/cycle_check.sh` script only verifies `cargo-modules` runs
  cleanly; it does not parse the dependency graph for cycles. Real cycle
  detection is deferred to custom dylint rule 4 (see
  `docs/DYLINT_RULES.md`). This policy is therefore aspirational until that
  rule lands.

### Formatting and docs

- `cargo fmt --check` in CI.
- `cargo doc --no-deps --workspace` in CI with
  `RUSTDOCFLAGS="-D warnings"`. The specific rustdoc denies
  (`broken_intra_doc_links`, `missing_crate_level_docs`) are set in the
  `[workspace.lints.rustdoc]` table, which binds only to crates that opt
  into workspace lints (see Compiler section). `RUSTDOCFLAGS="-D warnings"`
  is belt-and-suspenders so any escaped rustdoc warning still fails CI.

### Deferred custom dylint rules (specified, not built)

See `docs/DYLINT_RULES.md`.

## Layer 2 — Context Tree

### Layout

Mirror tree at repo root, gitignored from production builds, committed in dev.

```
repo/
├── src/
│   └── ...
└── .context/
    ├── intent.md
    ├── src/
    │   ├── rollup.ctx
    │   ├── intent.md
    │   └── <file>.ctx
    └── ...
```

### File formats

**Leaf `<file>.ctx`** (≤10 lines target, ≤40 hard ceiling):

```
file: <repo-relative source path>
purpose: <1-3 sentences>
invariants:
  - <bullet>
external_deps:
  - <crate or module>: <non-obvious reason it is used>
functions:
  - name: <fn_name>
    signature: <one line>
    purpose: <one sentence>
    notes: <optional>
```

**Non-leaf `rollup.ctx`** (≤15 lines target, ≤40 hard ceiling). A
directory whose rollup cannot fit in 40 lines has too much surface area;
emit anyway and let the auditor flag it. This budget must match
`prompts/summarizer-rollup.md` exactly; changing one requires changing the
other.

```
directory: <repo-relative dir path>
summary: <2-5 sentences on what this subtree provides>
children:
  - <name>: <one-line summary>
key_invariants:
  - <bullet>
```

**`intent.md`** (frozen after creation at MVP):

```
---
intent_version: 1
---
<prose>
```

### Access protocol

The `ctx-access` CLI is the intended only path to source. All invocations
take `--task-id <uuid>`. Enforcement that the agent cannot read source
directly is a *deployment* concern (the sandbox), not something the CLI
does itself. **Until the sandbox in `docs/SANDBOX.md` is deployed, Layer 2
is advisory: a determined or lazy agent can bypass `ctx-access` by reading
source directly.** The CLI is built with an internal `cli` / `enforcement`
/ `transport` seam so the sandbox broker split is a later transport change,
not a rewrite.

#### Task lifecycle

A task is an explicit, bounded unit of work with its own cache and report.

- `ctx-access init-task --task-id <uuid>` — validates `<uuid>`, refuses if
  a cache for it already exists (no clobber), writes the initial per-task
  cache. Must be called before any `read`/`write`/`list` for that task;
  those commands fail with a clear error if the cache is absent.
- `ctx-access end-task --task-id <id>` — the only command that mutates
  generated context files. It hands `paths_written` to the summarization
  runner (leaf-up), then to `ctx-audit`, writes
  `.context/.reports/<id>.json`, then deletes the cache file. Errors if no
  cache exists for `<id>`.

An orphaned cache (task crashed before `end-task`) is harmless: it is
gitignored and uniquely keyed by task id. `init-task --force` reclaims one.

#### Per-request commands

- `ctx-access read <path> --task-id <id>` — computes the chain from repo
  root to `<path>` (each directory contributes its `rollup.ctx` then its
  `intent.md`, top-down; then `<path>`'s leaf `<file>.ctx`; then source)
  and returns, in one response, every chain node not already in the task's
  `served_nodes`, in order, followed by the source contents. Nodes already
  served this task are omitted (prefix-cached). A second read in the same
  subtree therefore returns only the delta — often just the new leaf and
  source. `--shallow` serves the unserved chain nodes up to and including
  the target's leaf `<file>.ctx` but stops before source, for
  explore-without-edit. There is one tool call per `read`, not one per
  chain node. Context scaffolding is sparse by nature (`intent.md` is
  owner-authored, not one-per-directory; a `rollup.ctx`/leaf may not
  exist yet): a missing `rollup.ctx`/`intent.md`/leaf is served as an
  explicit one-line `(absent: no <kind> at this level)` marker so every
  chain level is still surfaced and counts as served. A missing **source**
  file is the only hard error (`MissingNode`).
- `ctx-access write <path> <content> --task-id <id>` — requires that a
  non-`--shallow` `read` of the same `<path>` succeeded earlier in the same
  task (i.e. its full chain incl. source is in `served_nodes`). Appends
  `<path>` to `paths_written`. No in-tree stale flags.
- `ctx-access list <path> --task-id <id>` — directory listing; requires
  that directory's `rollup.ctx` to be in `served_nodes`.

Per-task state lives at `.context/.cache/<task-id>.json` (gitignored).
`served_nodes` is the set of context-tree node identifiers (chain nodes and
source paths) already returned to the agent this task; it fully expresses
chain progress, so no per-step counter is needed:

```json
{
  "task_id": "...",
  "started_at": "<unix-epoch-seconds>",
  "served_nodes": ["..."],
  "paths_written": ["..."]
}
```

`write` evicts the written path's leaf `<file>.ctx` and its source from
`served_nodes`. The next `read` of that path therefore re-serves those two
nodes, and the leaf `<file>.ctx` carries a `STALE — modified in current
task` banner (its summary predates the edit; the summarizer runs only at
`end-task`). Ancestor `rollup.ctx`/`intent.md` nodes stay cached and
un-bannered — subtree summaries are regenerated wholesale at `end-task`,
not per-file. Without this eviction the banner would be unreachable: any
path writable in a task has, by the write-needs-read rule, already had its
entire chain served and cached, so it would never be re-served to carry
the banner.

### Summarization agent

Separate from the editing agent. Invoked once at task end. Operates leaf-up
over modified paths.

Prompts: `prompts/summarizer-leaf.md` (per source file) and
`prompts/summarizer-rollup.md` (per directory). Decoupled from any code
that invokes them; the runner loads them at runtime and passes dynamic data
in the user message only.

The runner (`ctx-summarize`) is model-agnostic: it shells a
deployment-configured command (`CTX_AGENT_CMD`) speaking a fixed contract
(stdin JSON `{"system","user"}` -> stdout completion text). The adapter
that calls a concrete LLM is a non-Rust, non-linted edge like the prompts;
a reference adapter lives in `agents/` (see `agents/README.md`).

Never edits `intent.md`. If the new rollup contradicts intent, the rollup
notes the disagreement in a `intent_divergence:` field for the audit step.

### Intent divergence audit

After summarization, `ctx-audit` produces a JSON report at
`.context/.reports/<task-id>.json`:

```json
{
  "task_id": "...",
  "completed_at": "...",
  "divergences": [
    {
      "path": "...",
      "verdict": "consistent|divergent",
      "severity": "none|low|medium|high",
      "rationale": "..."
    }
  ]
}
```

Each `divergences` entry is the verbatim JSON object emitted by the
auditor agent for one directory (see `prompts/auditor.md`); the runner
wraps them with `task_id`/`completed_at` and does not reshape them. The
array includes both `consistent` and `divergent` verdicts so the report is
a complete record, not only the flagged subset.

No CI failure on divergence at MVP. Report is informational.

## Layer 3 — Architecture Audit

Deferred. Hooks: intent files exist, divergence reports produced. Future
home of the "is this file/module doing too much" semantic check.

## Concurrency foundations

- Task IDs threaded through all `ctx-access` calls.
- Per-task cache and report files keyed by task id; no cross-task collision.
- `intent.md` carries `intent_version`; concurrent edits conflict on the
  version line in git.
- `.gitattributes` declares merge policy for generated context files.
  `ctx-resummarize <path>` is the manual recovery path on merge conflict.
- Divergence reports are JSON for later aggregation.
- Shared-filesystem concurrency (cache locking, task-affinity) is a
  `ctx-broker` concern (see `docs/SANDBOX.md`), not solved at MVP. MVP
  concurrency is branch/worktree-per-agent; the per-task keying above is
  what makes the later broker addition non-breaking.

## Reference project

(Rev 3, owner-decided 2026-05-17. Replaces the rev-1 config-validator,
which was a single linear pipeline; the meal planner has more module
surface, a real error taxonomy, persistence, and an external-call
boundary, so it exercises the context tree depth and the lint regime
harder. The rev-1 "no networking/async" constraint is intentionally
dropped — the network/dependency-policy stress is now part of the test.)

A CLI **meal planner**. Captures a user nutritional profile (weight, age,
gender, allergies, etc.) and persists it; generates a one-week plan close
to the WHO/FAO dietary guidelines; saves favorite meals; revises the plan
from user feedback; emits a full shopping list from the plan.

Hard design constraints (these are what make it a valid system test):

- **Numeric model is integer/fixed-point.** kcal as integers, nutrients
  as integer milligrams, ratios as basis points, explicit rounding. No
  raw float math — `float_arithmetic = deny` is honored, not relaxed.
  Whether this is tolerable for a real numeric domain is a primary thing
  the reference project is meant to learn.
- **The LLM ideation step sits behind a trait seam** (e.g.
  `MealIdeator`), with a deterministic fake for tests — mirroring the
  `Env`/`Summarizer` seams in `ctx-access`. Non-determinism stays out of
  the testable core.
- The HTTP/LLM client is quarantined behind that trait so the core stays
  pure; expect the dependency policy (`cargo-deny` `multiple-versions`,
  license allowlist) to require deliberate widening — that signal is
  wanted, since `deny.toml` is otherwise untested.
- Built and verified exclusively through `ctx-access` and `ctx-verify`.
