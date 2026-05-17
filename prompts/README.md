# Prompts

Agent prompts as plain markdown files. **Decoupled from any runner code.**

Any process that drives one of these agents loads the corresponding file at
runtime and passes its contents as the system prompt. The runner does not
embed prompt content. The runner does not append additional instructions
not in the file (input data goes in the user message, not the system
prompt).

## Files

- `summarizer-leaf.md` — produces a `<file>.ctx` summary for one source
  file.
- `summarizer-rollup.md` — produces a `rollup.ctx` summary for one
  directory.
- `auditor.md` — judges whether a directory's `rollup.ctx` is consistent
  with its `intent.md`.

## Iteration policy

These will need many revisions. Expect to rewrite them when running the
reference project surfaces problems. Use git history as the version log;
no in-file version numbers. When a prompt changes meaningfully, re-run the
summarizer over the existing tree to bring summaries up to the new
standard.

## What goes in a prompt file

- The agent's role.
- The exact input it will receive.
- The exact output format expected.
- Rules and constraints.
- Examples.

## What does NOT go in a prompt file

- Anything that is really code (parsing logic, validation of output, retry
  logic). That belongs in the runner.
- Information about the specific task being performed at runtime (file
  paths, diffs, anything dynamic). That goes in the user message the runner
  constructs.
- Anthropic-specific or model-specific instructions. The prompt should be
  portable across model upgrades within reason.
