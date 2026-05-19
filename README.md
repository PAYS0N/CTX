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
agents/           # model-specific agent adapters (CTX_AGENT_CMD targets)
```

The reference project (a real consumer of the system) lives OUTSIDE this
repo at `../meal-planning/` — a standalone template-instantiated workspace
(the CLI meal planner), kept separate so the sandbox can cleanly isolate
its source. See `../meal-planning/README.md`.

## Quickstart (once implemented)

1. Copy `template/` to a new repo.
2. Initialize the `.context/` tree.
3. Run `ctx-access init-task --task-id <uuid>`, then agent tasks via the
   `ctx-access` CLI (`read`/`write`/`list`) with that `--task-id`.
4. Run `ctx-access end-task --task-id <uuid>`, which drives the
   summarization agent (`prompts/summarizer-leaf.md`,
   `prompts/summarizer-rollup.md`) and the audit.
