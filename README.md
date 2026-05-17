# Opinionated Agentic Coding System — MVP Scaffolding

This repository contains the MVP scaffolding for an opinionated coding system
designed for autonomous agents. It is **not a complete implementation**. It
provides the configs, prompts, scripts, and structural decisions that are
expensive to change later, along with a clearly-scoped TODO list of the parts
that still need to be built.

See `docs/SPEC.md` for the full sealed specification.
See `docs/UNIMPLEMENTED.md` for the list of items that still need work.

## Layout

```
template/         # cargo workspace template — copy this to start a new project
prompts/          # agent prompts as plain files, decoupled from code
scripts/          # CI and pre-commit scripts (bash, no Rust dep)
docs/             # spec and unimplemented list
reference-project/  # placeholder for the reference project (config validator)
```

## Quickstart (once implemented)

1. Copy `template/` to a new repo.
2. Initialize the `.context/` tree.
3. Run agent tasks via the (unimplemented) `ctx-access` CLI with `--task-id`.
4. On task completion, run the summarization agent with the prompt from
   `prompts/summarizer.md`.
