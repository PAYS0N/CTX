# Unimplemented Items

What this MVP scaffolding contains versus what still needs building. Items
are grouped by layer and tagged with rough effort: S (hours), M (days),
L (weeks).

## What IS in this scaffolding

- Cargo workspace template with all lint configs (`template/`).
- Pre-commit and CI scripts for the mechanical checks (`scripts/`).
- Agent prompts as plain files in `prompts/`, decoupled from any code.
- Full sealed spec (`docs/SPEC.md`).
- Deferred dylint rule list (`docs/DYLINT_RULES.md`).
- This document.

## What is NOT implemented

### Layer 1

**`cargo-deny` configuration validation (S).** A `deny.toml` is provided in
the template; it has not been tested against a real dependency graph and
the license allowlist may need expansion when the reference project pulls
in real crates.

**Custom dylint crate (L).** None of the 9 rules in `DYLINT_RULES.md` are
implemented. The pre-commit scripts cover rule 1's line-count portion only.

**Verification that all listed clippy lints exist and behave as expected on
the pinned Rust version (S).** The list was compiled from current clippy
documentation but should be smoke-tested against the actual toolchain version
chosen at template instantiation. Some `restriction` lints have changed
names across Rust versions.

### Layer 2

**`ctx-access` CLI (M).** This is the load-bearing piece. The spec is
complete; no code exists. Required behavior:

- Subcommands: `read`, `write`, `list`, `init-task`, `end-task`.
- Per-task cache management at `.context/.cache/<task-id>.json`.
- Chain computation from repo root to target path.
- Step-by-step serving with separate tool calls (do not bundle).
- Stale banner logic based on cache `paths_written`.
- Enforcement that `write` requires prior `read` of the same path in the
  same task.
- Sandboxing instructions for the host environment (the CLI cannot enforce
  the sandbox itself — that is a deployment concern).

Suggested implementation: Rust binary, ~800-1500 lines, using `clap` for
arg parsing and `serde_json` for the cache. Built under the same lint rules
as everything else (eat the dogfood).

**Sandbox configuration (M, deployment-specific).** The agent's shell must
block direct reads of paths under configured source roots. This is not
something the CLI can do — it requires container, seccomp, or
shell-wrapper configuration at the agent runtime layer. Document but do
not implement here.

**Summarization-agent runner (M).** A script that:

- Reads the task-end cache to find `paths_written`.
- For each modified source file, calls the summarization agent with the
  prompt from `prompts/summarizer-leaf.md` and the file's current contents.
- Writes the resulting `.ctx` file.
- For each directory containing a modified file or modified child rollup,
  calls the summarization agent with `prompts/summarizer-rollup.md` and
  the current children summaries.
- Walks leaf-up to the root.
- Cleans up the per-task cache when done.

This is where prompt iteration will be heaviest. The runner should be a
thin shell over the prompt files, with prompt content never embedded in
code.

**`ctx-audit` script (S).** Compares each modified `rollup.ctx` against its
sibling `intent.md`. Probably implemented as another small agent call using
`prompts/auditor.md`. Produces JSON at `.context/.reports/<task-id>.json`.

**`ctx-resummarize <path>` recovery CLI (S).** Wraps the summarization
runner for manual invocation on a specific subtree, used after a merge
conflict in `.context/`. Trivial once the runner exists.

**Custom git merge driver (S, optional).** The `.gitattributes` declares
`merge=ctx-regenerate`; the driver itself is not built. Auto-invocation of
`ctx-resummarize` on merge. Optional because manual invocation works fine.

### Layer 3

**Everything.** Architecture audit is deferred entirely. Hooks present:
- `intent.md` files exist and carry versions.
- Divergence reports are produced as JSON.

Future work includes the "is this file doing too much" semantic check and
periodic full-tree architecture review.

### Concurrency

**Real multi-agent-on-shared-filesystem support (M-L).** Currently the
foundation supports branch-based concurrency (each agent on its own
worktree, merge later). Shared-filesystem concurrent agents would require:

- Cache file locking.
- Some form of task-affinity scheduling so two agents don't try to write
  the same file at once.
- Coordination on chain reads (probably trivial — chain reads are
  idempotent).

Not needed at MVP. Foundations (task IDs, per-task cache files) are in
place to add this without a redesign.

### Reference project

**The reference project itself (M).** `reference-project/` is a placeholder
directory. The intent is a CLI config validator/pretty-printer for a
non-trivial format (e.g., a TOML-superset with schema validation). Building
it end-to-end under the lint rules is the main MVP smoke test and will
surface what is intolerable in the rule set before broader use.

### Prompt deployment

**Decide on prompt-file location strategy when implementing the runner.**
Currently prompts live at the repo root in `prompts/`. Options for how
projects-instantiated-from-template find them:

1. Copy prompts into each project at template-init time. Each project owns
   its prompts and can diverge. Pro: isolation. Con: bug fixes don't
   propagate.
2. Reference prompts from a central installed location (e.g., a per-host
   `~/.config/agent-coding/prompts/`). Pro: central iteration. Con:
   versioning per project becomes a global config concern.
3. Vendor prompts via git submodule. Pro: explicit version pinning. Con:
   submodules.

Recommend option 1 for MVP simplicity, revisit when prompt iteration
across multiple real projects becomes painful.

## Suggested order of implementation

1. Smoke-test the lint config on a hello-world crate. Adjust list to match
   what current Rust/clippy actually accept.
2. Build `ctx-access` CLI. This unblocks everything else.
3. Build the summarization runner with the existing prompt files.
4. Build the reference project under the full toolchain. Iterate on prompts
   and lint thresholds as pain surfaces.
5. Build `ctx-audit`.
6. Custom dylint crate (rules in priority order from `DYLINT_RULES.md`).
7. Deployment-layer sandbox.
8. Architecture-audit layer.
