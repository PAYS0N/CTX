# Unimplemented Items

What this MVP scaffolding contains versus what still needs building. Items
are grouped by layer and tagged with rough effort: S (hours), M (days),
L (weeks).

## What IS in this scaffolding

- Cargo workspace template with all lint configs (`template/`).
- Pre-commit and CI scripts for the mechanical checks (`scripts/`),
  including `workspace_lints_check.sh` (every member must opt into the
  workspace lint table).
- Agent prompts as plain files in `prompts/`, decoupled from any code.
- Full sealed spec, rev 2 (`docs/SPEC.md`).
- Deferred dylint rule list (`docs/DYLINT_RULES.md`).
- Sandbox deployment design (`docs/SANDBOX.md`).
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

**`ctx-access` CLI — IMPLEMENTED.** Built and dogfooded at the repo root
(`crates/ctx-access`, `cargo_access` lib + bin) under the full lint regime
(clippy pedantic/nursery/restriction + `-D warnings`, fmt, rustdoc,
rationale, workspace-lints, no-allow all green; 8 in-memory behavioral
tests pass; end-to-end CLI smoke verified). The `cli` / `enforce` / `env`
seam from `docs/SANDBOX.md` is in place. Remaining within this piece: the
`end-task` summarizer is a `NoopSummarizer` (the agent-driven runner is the
separate item below); a real `transport`-to-`ctx-broker` is deployment
work. Original required behavior, all now satisfied:

- Subcommands: `init-task`, `read`, `write`, `list`, `end-task`.
- Per-task cache management at `.context/.cache/<task-id>.json` with the
  rev-2 schema (`served_nodes` set, not `chains_read`).
- Chain computation from repo root to target path.
- Single-call chain serving: one `read` returns every not-yet-served
  ancestor node plus source, in order; `served_nodes` is the prefix
  cache. `--shallow` stops before source. NOT step-by-step / one call
  per node (that was rev 1; rev 2 bundles).
- Stale banner logic based on cache `paths_written`.
- Enforcement that `write` requires a prior non-`--shallow` `read` of the
  same path in the same task.
- Mandatory internal seam: `cli` (argv/io) / `enforcement` (pure logic
  over injected fs+clock) / `transport` (MVP: in-process; future: UDS to
  `ctx-broker`). The sandbox depends on this seam — it is not optional.
  See `docs/SANDBOX.md`.

Implemented as: Rust lib+bin (~700 LoC), `clap` + `serde_json` + `thiserror`.
The `enforce` module avoids `indexing_slicing`/`string_slice`/
`as_conversions`/`unwrap` throughout (typed `thiserror` enums, `.get()`,
`strip_prefix`, no print macros — output via injected `Write`).

**Dogfood findings (Phase 0/1), recorded for the reference-project notes:**

- Toolchain pin was a stale, untested `1.83.0`; bumped to the installed
  current stable `1.95.0`. The full clippy restriction list is valid on
  1.95.0 with zero renames.
- `rustfmt.toml` set 7 nightly-only options silently ignored on stable
  (`imports_granularity`/`group_imports` are the impactful losses).
  Removed; documented as deferred. Classic "looks-enforced-but-isn't".
- The spec exempts `unwrap`/`expect` in tests but no mechanism expressed
  it (`#[allow]` is banned); added `allow-unwrap-in-tests` /
  `allow-expect-in-tests` to `clippy.toml`.
- rustfmt's 80-col expansion inflates physical line count, so the 30-line
  soft tier bites cohesive functions that are ~22 logical lines. Handled
  by small helper extraction; one genuinely-linear test uses the spec's
  `// rationale:` escape hatch as intended.
- `rationale_check.py` mis-measured bodyless trait-method signatures
  (brace heuristic ran past them); fixed to skip `;`-terminated decls. It
  still only recognizes a *single-line* `// rationale:` immediately
  preceding — multi-line rationale blocks are not supported (minor).
- Latent, not yet resolved: the spec grants a `print_stdout`/`print_stderr`
  exception for bin `main.rs`, but the workspace clippy table denies them
  globally and `#[allow]` is banned, making the exception unusable. Side-
  stepped here by writing through `Write` handles (never print macros), so
  no re-spec was required — but the dead exception should be struck on the
  next spec touch.

Still pending (separate S items, unchanged): `cargo-deny` validation
against the real dependency graph, `cargo-machete`, and `cargo-modules`
are not installed in this environment, so those CI phases do not yet run.
`Cargo.lock` is now committed so the dep graph is pinnable.

**Sandbox configuration (M, deployment-specific).** The agent's shell must
block direct reads of paths under configured source roots. This is not
something the CLI can do. Design is specified in `docs/SANDBOX.md`
(`ctx-broker` daemon as the only source-reading identity + mount-namespace
or container isolation of the agent). Until deployed, **Layer 2 is
advisory, not enforced** — the spec now says so explicitly. The only part
built at MVP is the `ctx-access` internal seam that makes the later broker
split a transport swap rather than a rewrite.

**Summarization-agent runner — DONE.** `crates/ctx-summarize` (lib+bin,
dogfooded through `ctx-verify`, 6 tests incl. real-subprocess agent):

- `from-cache --task-id` reads `paths_written` from the task cache;
  `paths <p>...` takes explicit targets.
- Each source file -> `prompts/summarizer-leaf.md` as system prompt, file
  contents as user message -> writes `.context/<path>.ctx`.
- Each affected directory -> `prompts/summarizer-rollup.md` + assembled
  children summaries + the dir's `intent.md` -> writes `rollup.ctx`.
- Walks leaf-up (deepest dir first, repo root last).
- Does NOT touch the per-task cache: per SPEC rev 2, `ctx-access
  end-task` is the cache's sole deleter (this corrects the rev-1 wording
  that had the runner clean it up). `ctx-summarize` also never writes
  `intent.md`.
- LLM is behind an `Agent` seam; the real `SubprocessAgent` speaks a
  model-agnostic JSON-on-stdin contract via `CTX_AGENT_CMD` (no SDK
  dependency; the network/dep-policy stress lands in the reference
  project as intended).

Prompt iteration will still be heaviest here; the runner is a thin shell
with prompt content never embedded in code.

KNOWN DEBT (`ctx-core` extraction): the repo-relative path-safety + the
`.context` mirror mapping now exist in BOTH `ctx-access`
(`repo_path`/`chain`) and `ctx-summarize` (`cpath`), kept deliberately
small and identical in spirit. Post-MVP these (and the task-cache view)
should move to a shared `ctx-core` crate so the security-sensitive path
validation has a single source of truth. Recorded so the
architecture-audit layer flags it rather than it being silent drift.

**`ctx-audit` script (S).** Compares each modified `rollup.ctx` against its
sibling `intent.md` via an agent call using `prompts/auditor.md`. Produces
JSON at `.context/.reports/<task-id>.json`. The per-directory auditor JSON
(`{path, verdict, severity, rationale}`) is passed through verbatim into
the `divergences` array; the runner only adds the `task_id`/`completed_at`
wrapper and does not reshape entries (rev-2 schema).

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

**The reference project — DONE.** Relocated OUT of this repo to
`../meal-planning/` (standalone, so the sandbox can isolate its source).
A template-instantiated cargo workspace with `crates/mealplan` (a CLI meal
planner; integer/fixed-point nutrition math, LLM behind a trait seam).
Built end-to-end under the full regime: `ctx-verify` PASS on all six gates,
13 hermetic tests, offline CLI verified (every day scores 0 — exact
WHO/FAO band fit), and `ctx-summarize` generated the mirrored `.context`
tree (loop closed). Findings — including the headline result that the
`float_arithmetic` ban is *not* intolerable for a real numeric domain —
are recorded in `../meal-planning/README.md` § Findings. Open items
(real chain-read latency, summary usability) need a real-`CTX_AGENT_CMD`
agent loop, not the build itself.

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

1. ~~Smoke-test the lint config~~ — DONE (Phase 0; toolchain 1.95.0).
2. ~~Build `ctx-access` CLI~~ — DONE (Phase 1; dogfooded).
3. ~~Build `ctx-verify`~~ — DONE. Token-frugal verification broker
   (`crates/ctx-verify`): wraps clippy/doc/fmt/rationale/workspace_lints/
   no_allow via a `Runner` seam, parses structured + `FAIL:`-line output,
   returns one capped JSON report (audit-report schema family); missing
   tools => `skipped`, not `fail`. Dogfooded (full workspace verifies as
   one ~35-line JSON `pass`); 8 hermetic tests incl. failure paths
   (capping, failed-command-without-diagnostics, raw rustc compile error);
   green on the full regime. Does NOT wrap `cargo test` — it mirrors
   `ci.sh`'s gate set, which excludes the test suite (compile/lint
   failures still surface via the `clippy`/`doc` checks).
4. ~~Build the summarization runner~~ — DONE (`crates/ctx-summarize`).
4b. ~~Reference agent adapter~~ — DONE. `agents/summarizer-claude.py`
   (Anthropic API, python3 stdlib, Sonnet default, prompt-cached system
   block) + `agents/README.md` documenting the model-agnostic
   stdin-JSON->stdout contract. Non-Rust, non-linted edge (like
   `prompts/`). It is the default `CTX_AGENT_CMD`. Error paths smoke-
   tested; the live Anthropic HTTP call is exercised by the reference
   project (cannot be smoke-tested without billing). Hard prerequisite
   for step 5 — the reference-project loop needs a real agent.
5. ~~Build the reference project~~ — DONE (`../meal-planning/`,
   `crates/mealplan`). ctx-verify PASS, 13 tests, loop closed; findings in
   `../meal-planning/README.md` § Findings.
6. Build `ctx-audit`.
7. Custom dylint crate (rules in priority order from `DYLINT_RULES.md`).
8. Deployment-layer sandbox.
9. Architecture-audit layer.
