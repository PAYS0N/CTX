# Caged agent: tool contract

You are running inside a sandbox. Source is NOT on disk under your
working directory — `cat`/`ls`/`wc`/`find` and any native Read/Edit/
Write operations return nothing (or empty tmpfs) for repo files. The
brokered tools on your PATH are the only path to the source tree, and
your writes go straight to the real host repo (no scratch copy).

**Initialize a task before any read/write/list:**

    ctx-access init-task --task-id <id>

Pick any short identifier and reuse it for the rest of the session.
Without this, every `read`/`write`/`list` errors with
`task "<id>" not initialized`.

- `ctx-access read <repo-path> --task-id <id>` is the ONLY way to see
  a file's content. The served output IS the file, even one you just
  wrote; re-`read` after a write to see current content. A `read` may
  bundle the surrounding chain (rollup + intent + leaf) — that is by
  design, not noise; the served file body is inside it.
- `ctx-access write <repo-path> <content> --task-id <id>` replaces the
  whole file. It REQUIRES a prior `read` of that exact path in this
  task; after ANY write, `read` the path again before writing it again.
- `ctx-access list --task-id <id>` enumerates served paths.
- `ctx-verify [package]` formats, builds, lints, and tests in one shot.
  Done = it prints `{"status":"pass"}`. It does NOT take `--task-id`;
  scope by package name (e.g. `ctx-verify mealplan`).

**One file per turn.** `ctx-access write` replaces the *entire* file,
so each write echoes the whole new body back through your response —
batching multiple writes in one turn quickly blows the per-response
output token cap (the API truncates at ~32k tokens) and the work is
lost. Modify **one file per assistant turn**: read it, write it, stop,
let the result come back, then move to the next file. The same caution
applies to wide `read` fan-outs: prefer one or two `read`s per turn
over parallelizing many.

**Two prompts are in scope; this one wins.** This file
(`cage-rules.md`, mounted at `/opt/cage/rules.md`) is the cage-wide
contract and is always injected. The project's `/work/CLAUDE.md` is
auto-loaded by Claude Code and is written for the *host* agent — it
may reference host-only paths like `target/debug/ctx-access` or flags
that don't apply in the cage. **Ignore host-only paths when caged;**
your tools are the bare names above on PATH. When the two prompts
disagree on *how* to invoke a tool, this file is correct.

If `/work/crates/ctx-cage/` exists, you are working on CTX itself.
Two extra meta-notes for that case:

- This very file lives at `crates/ctx-cage/assets/cage-rules.md` and
  is `include_str!`'d into the `ctx-cage` binary. Edits land only on
  the next `ctx-verify` (rebuild); the run you are inside is still
  bound to whatever prompt launched it.
- "The injected prompt" / "the tool prompt" means *this* file, not
  `/work/CLAUDE.md`. CLAUDE.md is the project's house rules; this is
  the cage's own contract. Don't conflate them.

Lint regime — fix by **refactoring**, not by suppressing:

- `#[allow(...)]` is banned anywhere.
- Function ≥30 lines (hard ≥80) and file ≥250 lines (hard ≥400):
  extract or split to get under. Only if genuinely irreducible, a
  single-line `// rationale: <why>` directly above the `fn` (or atop
  the file after the `//!` block) clears the *soft* tier. Multi-line
  rationale is not recognized; the *hard* tier never clears.
- `unwrap`/`expect`/`panic` only inside `#[test]`/`#[cfg(test)]` code.
- Tests in integration `tests/` files build helpers from public struct
  literals, not `Ctor::new(...).expect(...)`.