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
- **Agent-run wiring — DONE & proven no-spend** (ADR-028/029/030):
  `agent-demo.sh` → `AGENT RUN PASS`; `--preflight` → `PREFLIGHT PASS`
  (real `--claude` env); `--check-onboarding` → `ONBOARDING CHECK PASS`
  (no wizard/key prompt, subscription auto-detected). Generalized broker
  `{ctx-access, ctx-verify}`, egress 1a, `--clearenv`+synthesized
  `~/.claude.json`, headless + PTY-isolated interactive.
- **MVP VALIDATED end-to-end** (ADR-031/032/033): summary tree
  regenerated via `ctx-summarize` (18 leaves + 6 rollups) and
  **committed** (`meal-planning bfc7280`); a re-run caged billed agent
  built `profile edit` (77 insertions) with the chain served,
  `ctx-verify mealplan` = `{"status":"pass"}`, audit 0 divergences.
  Harness fixes landed (ADR-027/031). The agent's deliverable remains
  uncommitted in meal-planning (keep or discard).
- **Cage promoted to Rust** (ADR-034): new `crates/ctx-cage` (lib + 2
  bins) supersedes the Bash sandbox. Parameterized target (no default),
  auto-discovered crate dirs, embedded cage-rules + nsswitch, real
  framed UNIX-socket protocol, spend gate enforced. Delivered in 7
  turns; each `ctx-verify` `{"status":"pass"}`. Parity smoke against
  meal-planning: `ctx-cage --self-test stub` → `SELF-TEST-STUB-OK`.
  All `sandbox/*.sh` + `pty-relay.py` retired (the README is now a
  pointer). Dedicated-PTY isolation for `--interactive` deferred
  (turn 6b backlog; current mode inherits parent tty, sound under
  `legacy_tiocsti=0`).

## MVP VALIDATED end-to-end — chain present, committed, used (ADR-031/032/033)

**The full thesis now holds, on a committed reproducible baseline.**
History: a first billed run proved the **access/cage** layer
(cage/broker/deny-gate/write-requires-prior-read/lifecycle/blinding;
ADR-031) but — caught by inspection (ADR-032) — ran with **no summary
tree** (wiped on git-init, never committed), so the agent built from
raw source and CTX's *core thesis* (the summarized chain is useful
context) was unproven. Corrective executed (ADR-033): regenerated
`.context` via `ctx-summarize` (billed; 18 leaves + 6 rollups),
**committed** it (`meal-planning bfc7280` — closes the
stated-but-unfollowed "`.ctx` ARE committed" doctrine), and re-ran the
caged billed agent. Result: clean `profile edit` (77 insertions),
`ctx-verify mealplan` → `{"status":"pass"}` (re-verified host-side),
audit **0 divergences**. The chain reached the agent (committed
non-absent tree + brokered sole source path, reproduced host-side;
implementation honors summary-stated invariants). Honest caveat:
headless `claude -p` shows narration not raw tool stdout, so proof is
environmental+corroborating, not a verbatim capture (a stream-json run
could capture bytes if ever wanted — judged unnecessary, ADR-033).

Harness fixes also landed: `meal-planning/CLAUDE.md`,
`--dangerously-skip-permissions`, and the ADR-027 scoped-revert +
refuse-on-dirty guard (a blanket `git checkout crates` had destroyed a
prior deliverable).

**Open:** the agent's `profile edit` deliverable is uncommitted in
meal-planning (validation output — keep or discard). Deferred:
production `ctx`-uid/cache-owning broker, Layer 3 (`UNIMPLEMENTED.md`).

The cage substrate (kept as the record):

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

**Result (ADR-031).** `CTX_CAGE_ALLOW_SPEND=1 sandbox/agent-demo.sh`
ran a real billed `claude` in the cage; it used `ctx-access` for all
source, hit and recovered from write-requires-prior-read, refactored to
the length tiers (not `// rationale:`), and reached `ctx-verify
mealplan` = `{"status":"pass"}`. The agent's deliverable is in
`../meal-planning` (uncommitted): `cli/mod.rs`+`handlers.rs`,
`ProfileEditArgs`/`ProfileCmd::Edit`/`apply_profile_edit`.

**Re-run (clean conditions):** `CTX_CAGE_ALLOW_SPEND=1
sandbox/agent-demo.sh` (add `--interactive` to watch). No API key
(subscription auto-bound); house rules auto-loaded from
`/work/CLAUDE.md`; no manual permission accept. Spend boundaries: the
agent loop (that env switch) and `end-task` (audit→summarize) — still
explicit-go.

**Next (post-MVP):** commit the validated artifact if desired; write
`../meal-planning` findings; the deferred production broker
(`ctx`-uid/cache-owning, SANDBOX.md) and Layer-3 work remain in
`UNIMPLEMENTED.md`.

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
