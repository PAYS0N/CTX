Doctrine: `.context/intent.md` + `docs/SPEC.md`. Rationale:
`docs/DECISIONS.md`. Current focus: `docs/STATUS.md`.

Call `target/debug/ctx-context .`. Do not skip this. 

- Source: native Read/Edit/Grep. Do not use cat for file reads. The context chain is hook-injected on
  read. To use on demand: `target/debug/ctx-context <path>` (dir or `.`). 
  Use the tool on a directory when you want to know more about a directories contents.
  When gathering information before doing code changes, use the tool to retrieve 
  compacted context instead of reading the raw files. Only go as deep into a directory as needed.
  Never hand-edit `.context/`.
- `ctx-scan <dir> --check` shows what context is stale.
- Checkpoint: `target/debug/ctx-verify [crate]` — formats, builds,
  lints, and tests in one call; done = `{"status":"pass"}`. Do not run
  cargo yourself; if unavoidable: `-q --message-format=short`, never
  paste build dumps.
- Lints: `#[allow]` is banned. unwrap/expect compile only inside
  `#[test]`/`#[cfg(test)]` bodies — test helpers outside them must
  return `Result`. A 30-line fn / 250-line file: refactor first; a
  single-line `// rationale:` directly above (fn) or after `//!`
  (file) is the last resort, and multi-line is not recognized.
- `.env` holds the summarizer key: never feed it to a model, never
  commit it.
- `template/` and root workspace lint configs mirror each other;
  change both.
