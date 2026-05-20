# Working here

Doctrine is `.context/intent.md` (read it via ctx-access) and
`docs/SPEC.md`; rationale is `docs/DECISIONS.md`; current focus + next
step is `docs/STATUS.md`. This file is only the operating rules not in
those. Reasons live in DECISIONS.md.

- Read/write source through `ctx-access` (`target/debug/ctx-access`) —
  not the editor's Read/Edit or `cat`. `read <path> --task-id` serves the
  chain; `write` needs a prior read of that path.
- Verify with `ctx-verify` — it *formats, builds, lints, and tests* in
  one shot and emits one capped JSON report (`{"status":"pass"}` = done;
  anything else lists only the failures). It is the whole checkpoint;
  do not assemble `cargo fmt`/`build`/`test` yourself. Example:
  `target/debug/ctx-verify` (whole workspace) or
  `target/debug/ctx-verify ctx-access` (scope to a crate). Raw `cargo`
  only if truly unavoidable: `-q --message-format=short`; never paste
  build dumps.
- `#[allow]` is banned (CI-grepped). When a lint bites: unwrap/expect in
  tests only via `clippy.toml` (covers `#[test]`/`#[cfg(test)]` bodies
  only — build `tests/` fixtures with public struct literals, not
  `Ctor::new(..).expect()`); add `const fn` when asked. Length tiers:
  **refactor first** (extract/split to get under 30-line fn / 250-line
  file); a single-line `// rationale:` (directly before the fn, or atop
  the file after `//!`) is the *backup* only when genuinely irreducible,
  not the default fix. Multi-line rationale is not recognized.
- Real model calls (`CTX_AGENT_CMD`/adapter) and any outward or
  irreversible action: only on explicit user go. `.env` holds the key —
  never feed it to a model, never commit it.
- `template/` lint configs and the root workspace mirror each other;
  change both.
