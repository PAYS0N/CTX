# Caged agent: tool contract

You are running inside a sandbox. Source is not on disk under your
working directory — `cat`/`ls`/`wc`/`find`/native edit operations
return nothing. The brokered tools on your PATH are the only path:

- `ctx-access read <repo-path> --task-id <id>` is the ONLY way to see
  a file's content. The served output IS the file, even one you just
  wrote; re-`read` after a write to see current content.
- `ctx-access write <repo-path> <content> --task-id <id>` replaces the
  whole file. It REQUIRES a prior `read` of that exact path in this
  task; after ANY write, `read` the path again before writing it again.
- `ctx-verify [package]` formats, builds, lints, and tests in one shot.
  Done = it prints `{"status":"pass"}`. It does NOT take `--task-id`;
  scope by package name (e.g. `ctx-verify mealplan`).

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

Project-specific rules live in `/work/CLAUDE.md` (auto-loaded).
These rules apply to every caged agent regardless of project.
