# MVP Specification — Opinionated Agentic Coding System

Status: sealed. Changes require explicit re-spec.

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
- Every crate root: `#![deny(missing_docs)]`, `#![forbid(unsafe_code)]`.

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
- `cargo-modules` + custom script: no circular module deps within a crate.

### Formatting and docs

- `cargo fmt --check` in CI.
- `cargo doc --no-deps -D rustdoc::broken_intra_doc_links -D
  rustdoc::missing_crate_level_docs` in CI.

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

**Non-leaf `rollup.ctx`** (≤10 lines target, ≤40 hard ceiling):

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

The `ctx-access` CLI is the only path to source. All invocations take
`--task-id <uuid>`. The agent's shell is sandboxed to block direct reads of
source paths.

Commands:

- `ctx-access read <path> --task-id <id>` — serves the chain step-by-step.
  Each step requires a separate tool call. Per-task cache skips already-read
  chain prefixes within the same task. Final step returns source contents.
- `ctx-access write <path> <content> --task-id <id>` — requires that `read`
  has been called for the same path under the same task id. Records the
  write in the per-task cache (no in-tree stale flags).
- `ctx-access list <path> --task-id <id>` — directory listing, requires the
  directory's `rollup.ctx` to have been read first.

Per-task state lives at `.context/.cache/<task-id>.json` (gitignored):

```json
{
  "task_id": "...",
  "started_at": "...",
  "chains_read": ["..."],
  "paths_written": ["..."]
}
```

A read of a file whose path appears in `paths_written` for the current task
prepends a `STALE — modified in current task` banner to the served context
files.

### Summarization agent

Separate from the editing agent. Invoked once at task end. Operates leaf-up
over modified paths.

Prompt: `prompts/summarizer.md`. Decoupled from any code that invokes it.

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
      "intent_summary": "...",
      "rollup_summary": "...",
      "severity": "low|medium|high"
    }
  ]
}
```

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

## Reference project

A CLI tool that parses, validates, and pretty-prints a non-trivial config
format. Chosen to exercise modules, errors, public API, and IO without
needing networking or async.
