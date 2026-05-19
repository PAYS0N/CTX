# Status & Active Plan

The live "where we are, what's next, and why." `SPEC.md` is the frozen
design; `UNIMPLEMENTED.md` is the backlog; `DECISIONS.md` is the rationale
log; this file is the moving part. Update it whenever the active focus
changes.

Last updated: 2026-05-19.

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
- **Static checks made deterministic** (ADR-025): `rationale_check.py`,
  `workspace_lints_check.sh`, `no_allow_check.sh` enumerate via
  `git ls-files` (no FS walk) — root cause of a phantom
  `{status:fail,count:0}` triple (walk racing a `target/` writer);
  ADR-024 retained as defense-in-depth. Validated under heavy concurrent
  `target/` churn (36 invocations, all clean) + detection intact.
- **The cage (`sandbox/`) — DONE & proven** (Cage C/D below); `CAGE D
  PASS`, no spend, reference tree unmutated.
- **Agent-run wiring — DONE & proven no-spend** (ADR-028):
  `sandbox/agent-demo.sh` → `AGENT RUN PASS`. Generalized broker
  `{ctx-access, ctx-verify}`, egress 1a, headless + PTY-isolated
  interactive, `stub-claude.sh` closes the full loop. Only the billed
  run remains (explicit go).

## Active focus: the real billed run (explicit-go only)

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
- **Cage C — DONE.** `sandbox/` cage: `bwrap` mount+net ns,
  `../meal-planning` mounted **read-only** with `crates/mealplan/{src,
  tests}`+`target/` as empty `tmpfs` (source absent), `--unshare-net`.
  In-cage `ctx-access` is a UNIX-socket forwarder to a host-side broker
  running the real binary in the real tree — SANDBOX.md's client/broker
  **transport seam** at the proving minimum (ADR-026; production
  `ctx`-uid/cache-owning broker stays deferred).
- **Cage D — DONE.** `sandbox/cage-demo.sh`: init-task (no spend) →
  broker up → cage runs `stub-agent.sh` (reachability: `cat`/`find`/
  `grep -r` all blocked, only `ctx-access` serves — 36-entry manifest,
  profile.rs 4450 B) → cage runs `cage-adversary.sh` under a **fresh
  zero-read task** (enforcement preserved through the forwarder: secret/
  repo-boundary/blind-write/bogus-task all denied host-side) → integrity
  gate (reference tree unmutated) → broker down → shutdown. `CAGE D
  PASS`, no model called. An unsound first adversary draft overwrote
  real source (recovered via git); fixed + integrity net added (ADR-027;
  prereq: meal-planning is now a git repo, ADR-023/025).

**The wiring is built and proven; one gated step remains.** The full
agent loop runs end-to-end with **no spend** via `sandbox/agent-demo.sh`
(`AGENT RUN PASS`): init-task → caged agent → brokered
`ctx-access`+`ctx-verify` → shutdown → host acceptance. Decided &
implemented (ADR-028): egress **1a** (caged `claude` calls the API
directly, `--net`+key); `ctx-verify` brokered too (cage needs no
toolchain/source); blinded brief; dual **headless** (validity-bearing)
and **`--interactive`** (PTY-isolated, observation only) modes — both
proven with the no-spend `stub-claude.sh`. The only thing not done is
flipping the switch.

**To do the real billed run:** `CTX_CAGE_ALLOW_SPEND=1
ANTHROPIC_API_KEY=… sandbox/agent-demo.sh` (add `--interactive` to
watch). Task: add a `profile edit` command to `mealplan` (reads
`profile.rs`/`cli` via the chain, writes, keeps `ctx-verify` green).
Spend boundaries: the agent loop (that env switch) and `end-task`
(audit→summarize). **Requires explicit user go.**

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
