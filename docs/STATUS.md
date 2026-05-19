# Status & Active Plan

The live "where we are, what's next, and why." `SPEC.md` is the frozen
design; `UNIMPLEMENTED.md` is the backlog; `DECISIONS.md` is the rationale
log; this file is the moving part. Update it whenever the active focus
changes.

Last updated: 2026-05-18.

## Done and verified

- **Layer 1** lint regime: smoke-tested on toolchain 1.95.0 (ADR-004),
  rustfmt nightly-options stripped (ADR-005), test exemptions via
  clippy.toml (ADR-006). CTX dogfoods it.
- **`ctx-core`** (`crates/ctx-core`): dependency-free leaf crate; the
  single source of truth for the security-sensitive access gate
  (secret/binary/gitignored deny) shared by ctx-access + ctx-summarize
  (ADR-023). Unit-tested.
- **`ctx-access`** (`crates/ctx-access`): init/read/write/list/end-task,
  single-call bundled chain (ADR-007), `served_nodes` cache (ADR-008),
  STALE-on-rewrite (ADR-009), soft absent-markers for missing scaffolding
  (ADR-010). 8 tests; chain/verbosity/staleness verified by hand.
- **`ctx-verify`** (`crates/ctx-verify`, was `ctx-check`): THE agent
  checkpoint (ADR-011, scope ADR-022) — formats(apply)+builds+lints+
  tests in one call; optional crate-name scopes cargo via `-p`;
  all-pass collapses to `{"status":"pass"}` (per-check map omitted).
  9 tests; dogfooded green; immediately caught a broken doc link the
  earlier tight runs skipped.
- **`ctx-summarize`** (`crates/ctx-summarize`): leaf-up, prompts from
  files, subprocess agent contract (ADR-012); 6 tests.
- **`agents/summarizer-claude.py`**: Anthropic API adapter, prompt-cached
  system block (ADR-013); `.env` mechanism (gitignored).
- **Prompts**: richer `files(9)/` set adopted (ADR-014); rollup
  no-fence fix applied.
- **Reference project**: `../meal-planning/` (relocated, ADR-015) — CLI
  meal planner, integer/fixed-point (ADR-003), `ctx-verify` PASS, 13
  tests, offline CLI exact, loop closed. Findings in
  `../meal-planning/README.md` § Findings.
- **Two real (billed) summarize+audit runs**: the reference project and
  CTX's own tooling source. Summaries accurate; gaps surfaced (no
  doctrine in tree without `intent.md` — now addressed for CTX).
- **Handoff docs**: `CLAUDE.md`, `docs/DECISIONS.md`, this file,
  `.context/intent.md`.

## Active focus: build the sandbox cage (deferred spend)

The genuine MVP validation: a blinded agent extends the reference project
through `ctx-access` only. **Validity is environmental, not behavioral**
(ADR-016): an uncaged agent is never a valid subject; the builder cannot
be the caged subject; the agent is a headless `claude -p` inside `bwrap`,
`ctx-access` its only path to source. Tooling present: `bwrap` 0.8.0,
`unshare`, unprivileged userns, `claude` CLI, uid 1000.

- **Cage A — DONE.** Reference project relocated to `../meal-planning/`;
  CTX doc refs fixed; tooling verified from the new location.
- **Cage B — DONE (all of it).** Deny-by-default gate
  (secret/binary/gitignored, refused even if explicitly passed; ADR-017)
  single-sourced in the new **`ctx-core`** crate (ADR-023, security
  single-source) and wired into BOTH `ctx-access` (read/write/list +
  `Env::tracked_files`/`is_ignored` + `manifest` module/command
  materialized at `init-task`) and `ctx-summarize` (per-target gate +
  `Fs::is_ignored` + `--approve` >`MAX_TARGETS` scope gate). e2e
  verified on this repo: manifest excludes `.env`; `read`/`summarize`
  of `.env`→denied(secret), gitignored→denied. Side work: rationale
  guidance refactor-first (ADR-021); `ctx-verify` rename+extend
  (ADR-022); `ctx-verify` `errored`≠`fail`/never-silent + deterministic
  (ADR-024). Whole 4-crate workspace `{"status":"pass"}` via ctx-verify.
- **Cage C.** `bwrap` launcher: tmpfs over `../meal-planning/crates/*/src`,
  read-only binds of `.context/` + manifest + the `ctx-access` binary,
  no network. Prove source is unreachable except via the tool.
- **Cage D.** Lifecycle harness (init-task = startup, end-task =
  shutdown → summarize), then a **stub-agent dry-run** (no spend)
  proving a caged process cannot `cat` source and must use `ctx-access`.
  The real billed run (a headless `claude` doing the task) is gated on
  explicit user go.

**Chosen test task for the eventual real run:** add a `profile edit`
command to `mealplan` (reads `profile.rs`/`cli` via the chain, writes,
keeps `ctx-verify` green).

## Open decisions / risks

- Spend gating: real model calls only on explicit go (ADR-013/016).
- `ctx-core` extraction debt is recorded, not done (ADR-020).
- Layer 2 is advisory until Cage C/D land — stated in SPEC and intent.md.
- The CTX `.context` tree exists from a real summarize but predates
  `.context/intent.md`; re-running the rollup summarizer would let the
  root rollup align to the new doctrine (optional, costs a few calls).

## How to verify anything

`target/debug/ctx-verify` from the relevant directory (optionally a
crate name to scope). It formats+builds+lints+**tests** in one call;
`{"status":"pass"}` = done. Don't run `cargo fmt`/`build`/`test`
yourself; never raw `cargo` dumps (see `CLAUDE.md`).
