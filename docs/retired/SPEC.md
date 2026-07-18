# MVP Specification — Opinionated Agentic Coding System

Status: **sealed.** This document states what is true *now*, as one
coherent spec — not a base layer plus amendments. Changing it is a
process event, not an edit: it requires an owner-authorized re-spec
recorded as an ADR in `docs/DECISIONS.md`. The revision history (what
each past revision changed and why) lives there, not here — most
recently ADR-035/036 (the lead-by-hooks pivot) and ADR-051 (this
rewrite, which deleted the retired-mechanism prose the pivot had left
stranded in the body). For *why* a choice was made, read DECISIONS; for
*current state and open work*, read `docs/STATUS.md` and the regenerated
root rollup (`ctx-context .`).

## Goals

1. Make bad code unrepresentable or uncompilable, not merely discouraged.
2. Force agents to read project context top-down before acting on any file.
3. Lay foundations for later architectural auditing and concurrent multi-agent
   use without paying those costs now.

## Non-goals (MVP)

- Human ergonomics. This is for autonomous agents.
- Lint suppression/appeal mechanism.
- Editing `intent.md` after directory creation (it is frozen; a change
  is an owner decision, not a routine edit).

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
- No `#[allow(...)]` attributes elsewhere. Enforced by grep in CI
  (`scripts/no_allow_check.sh`).

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
  ISC, Unicode-DFS-2016. Zero advisories. No duplicate versions. Not run
  inside `ctx-verify` (needs network); a CI/offline concern (ADR-047).
- `cargo-machete`: no unused dependencies. Run by `ctx-verify`.
- No circular module deps within a crate. NOT enforced at MVP: the
  `scripts/cycle_check.sh` script only verifies `cargo-modules` runs
  cleanly; it does not parse the dependency graph for cycles. Real cycle
  detection is deferred to custom dylint rule 4 (see
  `docs/DYLINT_RULES.md`). This policy is therefore aspirational until that
  rule lands.

### Formatting and docs

- `cargo fmt --check` in CI (`ctx-verify` applies `cargo fmt` itself).
- `cargo doc --no-deps --workspace` in CI with
  `RUSTDOCFLAGS="-D warnings"`. The specific rustdoc denies
  (`broken_intra_doc_links`, `missing_crate_level_docs`) are set in the
  `[workspace.lints.rustdoc]` table, which binds only to crates that opt
  into workspace lints (see Compiler section). `RUSTDOCFLAGS="-D warnings"`
  is belt-and-suspenders so any escaped rustdoc warning still fails CI.

### The checkpoint

`ctx-verify [crate]` is the single verification gate: it applies
`cargo fmt`, then builds, lints (clippy + rustdoc), tests, and runs the
`scripts/` battery, emitting one capped report. Its terse render prints
the bare word `pass`; the `{"status":"pass"}` envelope is behind `--json`
(see the tool's own `--contract`). Serving context fails open; this gate
fails closed. The parallel `ci.sh`/`template/.github` stack is retired —
`ctx-verify` is the sole check (ADR-047).

### Deferred custom dylint rules (specified, not built)

See `docs/DYLINT_RULES.md`.

## Layer 2 — Context Tree

### Layout

Mirror tree at repo root, committed in dev. Each source directory has a
parallel `.context/<dir>/` holding that directory's `rollup.ctx`, an
optional owner-authored `intent.md`, a `<file>.ctx` leaf per source file,
and a `hashes.json` freshness sidecar.

```
repo/
├── src/
│   └── ...
└── .context/
    ├── intent.md
    ├── src/
    │   ├── rollup.ctx
    │   ├── intent.md
    │   ├── hashes.json
    │   └── <file>.ctx
    └── ...
```

### File formats

Generated context is **prose, not a fixed schema** — a few tight
paragraphs written the way a competent engineer orients a teammate about
to edit a file they haven't opened. The authoritative definition of the
format is the generator prompts (`prompts/summarizer-leaf.md`,
`prompts/summarizer-rollup.md`); this section states only the invariant
shape and budgets.

- **Leaf `<file>.ctx`** (≤10 lines target, ≤40 hard ceiling): leads with
  what the file does for the system in domain terms, then only what an
  editor would get wrong cold — non-obvious behavior, coupling to other
  files/wire formats, the signatures that matter, verifiable invariants,
  non-obvious dependencies. No history, no diffs, no filler.
- **Non-leaf `rollup.ctx`** (≤15 lines target, ≤40 hard ceiling): leads
  with what the subtree provides one level up, then the coupling its
  children share, each child's role from the parent's view, and
  subtree-spanning invariants — never re-deriving facts already in a
  child's `.ctx`. A directory whose rollup cannot fit in 40 lines has too
  much surface area; emit anyway and let the audit flag it. If the subtree
  contradicts its `intent.md`, the rollup ends with a single
  `intent_divergence:` line (that literal label is read by the auditor).
  `prompts/summarizer-rollup.md` refers to this budget; keep them
  consistent.
- **`intent.md`** (frozen after creation at MVP): YAML front matter with
  `intent_version`, then prose stating goals, non-goals, and invariants —
  things that survive a change of mechanism. It never describes current
  mechanism (that lives in the regenerated rollups); it is owner-authored
  and not regenerated by the tooling.

```
---
intent_version: 1
---
<prose: goals, non-goals, invariants>
```

### Context delivery

Agents read and edit source with their **native tools** — there is no
brokered access path and no task lifecycle. Context is *led*, not forced:
a Claude Code `PostToolUse` hook (`ctx-context --hook`, fail-open) reads
the tool event and injects the target's context chain as additional
context, deduplicated per session via
`.context/.cache/hook-<session-id>.json` (ADR-035/036/037).

`ctx-context <path>` is the read-only chain server behind that hook and
available directly. It prints, root→target, each directory's `rollup.ctx`
then `intent.md`, and for a file target the file's leaf `<file>.ctx`; `.`
or a directory target stops at that directory (directory summaries on
demand). It never serves source bytes. Context scaffolding is sparse by
nature (`intent.md` is owner-authored, not one-per-directory; a rollup or
leaf may not exist yet), so a missing node is served as an explicit
one-line `(absent: no <kind> at this level)` marker — every chain level
stays visible, and only a genuinely unreadable present file is an error.
The hook fails open, loudly: a chain error injects a `(chain unavailable)`
marker rather than blocking the read (ADR-037).

Each served summary is also checked against its directory's `hashes.json`
sidecar (below) and, when its content is untrustworthy, prefixed with a
one-line freshness marker: `[STALE …]` when the source has changed since
the recorded hash, `[NEVER GENERATED …]` when the source exists but no
summary is on record. These are distinct from `(absent: …)`, which means
"no such node exists to have" (e.g. a sparse `intent.md`); the freshness
markers mean "a node exists but its content is outdated or was never
built." The check is local to each node's own sidecar (fail-open: no
record ⇒ no marker), not a whole-tree recompute, so it stays cheap enough
to run on every hooked read.

### Freshness and regeneration

Freshness is a **content-hash tree** (CACT-style), not git state, so it
catches gitignore-invisible edits. Each mirrored directory carries a
`hashes.json` sidecar: a leaf entry is the hash of its source file, a
directory node the hash of its sorted children, so any change propagates
to the root. Scope is governed by `.ctxignore` (gitignore syntax, seeded
once from `.gitignore`, then the sole authority; ADR-044/045); the
`ctx-core` secret/binary deny is not overridable.

- `ctx-scan <dir> --check` recomputes the tree and reports stale
  directories and leaves, plus expected `.ctx`/`rollup.ctx` artifacts that
  are missing (deleted or never generated — freshness ≠ integrity); it
  never calls the model.
- `ctx-scan <dir> --update` regenerates only the stale leaves and rollups
  (leaf-up, so parents see fresh children), removes orphaned leaves, then
  rewrites the sidecars. Behind the `--approve` blast-radius gate.
- `ctx-scan <dir> --stop-hook` reports staleness as a Claude Code Stop
  `systemMessage` and exits 0. It never regenerates: regeneration has one
  owner, and it is post-session (ADR-043), because the Stop event fires
  every turn and would race the session.

### Summarization agent

Separate from the editing agent, invoked only by `ctx-scan --update`,
leaf-up over stale paths. Prompts (`prompts/summarizer-leaf.md`,
`prompts/summarizer-rollup.md`) are decoupled from the code that invokes
them; the runner loads them at runtime and passes dynamic data in the
user message only.

The runner (`ctx-summarize`) is model-agnostic: it shells a
deployment-configured command (`CTX_AGENT_CMD`) speaking a fixed contract
(stdin JSON `{"system","user"}` → stdout completion text). The adapter
that calls a concrete LLM is a non-Rust, non-linted edge like the prompts;
a reference adapter lives in `agents/` (see `agents/README.md`).

It never edits `intent.md`. If a new rollup contradicts intent, the rollup
notes it in a trailing `intent_divergence:` line for the audit step.

### Intent divergence audit

The `intent_divergence:` signal exists today (emitted by the rollup
prompt); the consuming auditor (`ctx-audit`, `prompts/auditor.md`) and its
JSON report are **deferred to Layer 3**. When built, the report records
one verdict object per directory (both `consistent` and `divergent`, so
it is a complete record), wrapped with `task_id`/`completed_at`. No CI
failure on divergence at MVP — the report is informational.

## Layer 3 — Architecture Audit

Deferred. Hooks: `intent.md` files exist and rollups already emit
`intent_divergence:`. Future home of the "is this file/module doing too
much" semantic check and of wiring the auditor into `ctx-scan --update`
(audit each regenerated rollup against its intent).

## Concurrency foundations

- `intent.md` carries `intent_version`; concurrent edits conflict on the
  version line in git.
- `.gitattributes` declares a merge policy for generated context files;
  manual recovery on a merge conflict is a re-run of `ctx-scan --update`
  over the affected path.
- Divergence reports (when built) are JSON for later aggregation.
- MVP concurrency is branch/worktree-per-agent. A shared-filesystem
  multi-agent broker is not solved at MVP and is out of scope for the
  current design; the path-keyed mirror tree is what makes a later
  addition non-breaking.

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
  `Env`/`Agent` seams in the CTX tooling. Non-determinism stays out of
  the testable core.
- The HTTP/LLM client is quarantined behind that trait so the core stays
  pure; expect the dependency policy (`cargo-deny` `multiple-versions`,
  license allowlist) to require deliberate widening — that signal is
  wanted, since `deny.toml` is otherwise untested.
- Built with native tools and verified through `ctx-verify`, with context
  led by the `ctx-context` hook and regenerated by `ctx-scan`.
