# Decision Log (ADRs)

Why things are the way they are — including roads not taken. Lightweight
ADR format. Outcomes also live in `SPEC.md`/code; this file is the
*rationale and rejected alternatives*, which are otherwise undiscoverable.
`intent.md` files should point here rather than restate this. Append a
new ADR for every non-obvious choice; never rewrite history (supersede
with a new ADR).

---

## ADR-001 — The sealed spec may be re-opened by the owner
**Status:** accepted. `SPEC.md` says "sealed; changes require explicit
re-spec." Revisions 2 and 3 were owner-authorized in conversation. Rule:
spec changes are allowed but must be recorded as a dated revision note in
SPEC.md *and* an ADR here. The "sealed" discipline = no silent drift, not
no change.

## ADR-002 — Reference project: meal planner, not a config validator
**Decision:** the reference project is a CLI meal planner.
**Rejected:** the rev-1 config-validator — a single linear
parse→validate→print pipeline that exercises little module surface or
error taxonomy. **Rationale:** the meal planner has real domain modules,
persistence, an external-call boundary, and a genuine numeric domain, so
it stresses the context-tree depth and lint regime far harder. The
"no networking/async" rev-1 constraint was dropped on purpose — the
dependency/seam stress is part of the test.

## ADR-003 — Nutrition math is integer/fixed-point
**Decision:** kcal as `i64`, nutrients as integer mg, ratios as basis
points, one `div_round` (i128 internally, half-away-from-zero).
**Rejected:** (a) build the `SafeFloat` newtype from `DYLINT_RULES.md`
rule 6 now — too much scope, couples two experiments; (b) relax
`float_arithmetic` for the reference project — defeats the experiment's
whole point. **Rationale:** purest test of whether the float ban is
tolerable for a real numeric domain. **Outcome:** it is — total cost ≈
one helper; every generated day scored 0 (exact). Headline finding.

## ADR-004 — Toolchain pinned to 1.95.0 (was 1.83.0)
**Decision:** bump the pin. **Rationale:** 1.83.0 was a stale, never-
smoke-tested pin; only current stable was installed; SPEC itself mandates
re-smoke-testing the lint list on a toolchain change. The full clippy
restriction list is valid on 1.95.0 with zero renames.

## ADR-005 — rustfmt: only stable-channel options
**Decision:** strip the 7 nightly-only options from `rustfmt.toml`.
**Rationale:** stable rustfmt *silently ignores* unstable options — a
rule that "looks enforced but isn't," the exact failure class this
project exists to prevent. `imports_granularity`/`group_imports` are the
real losses; documented as deferred. Revisit only via a deliberate
pinned-nightly decision.

## ADR-006 — Test unwrap/expect via clippy.toml, not `#[allow]`
**Decision:** `allow-unwrap-in-tests`/`allow-expect-in-tests = true`.
**Rationale:** the spec exempts unwrap/expect in tests, but `#[allow]` is
banned project-wide and CI-grep-enforced, so clippy.toml is the ONLY
mechanism that can express the exemption. Caveat: it covers only
`#[test]`/`#[cfg(test)]` bodies — NOT free helper fns in `tests/`; build
fixtures via public struct literals there.

## ADR-007 — `ctx-access read` is one bundled call, not N step-calls
**Decision:** one `read` returns every unserved chain node + source.
**Rejected:** rev-1's "one tool call per chain node." **Rationale:** the
enforcement is "the chain passed through the agent's context before it
touched source," which a single bundled response achieves; N round-trips
added latency/token tax and enforced nothing extra. `served_nodes` is the
prefix cache; `--shallow` stops before source.

## ADR-008 — Task lifecycle + `served_nodes` cache schema
**Decision:** explicit `init-task`/`end-task`; cache holds
`served_nodes` (a set) + `paths_written`. **Rejected:** a `chains_read`
step counter. **Rationale:** a served-node set fully expresses chain
progress without per-step state; `end-task` is the only mutator of
generated context and the sole cache deleter.

## ADR-009 — `write` evicts the path's leaf+source so STALE is reachable
**Decision:** `write` removes the written path's leaf+source from
`served_nodes`. **Rationale:** without eviction the STALE banner is dead
code — any path you may write has, by write-needs-read, already had its
whole chain served and cached, so it would never be re-served to carry
the banner. Verified end-to-end.

## ADR-010 — Missing chain node: soft marker for scaffolding, hard error for source
**Decision:** a missing `rollup.ctx`/`intent.md`/leaf is served as a
one-line `(absent: no <kind> at this level)`; only missing **source** is
a hard `MissingNode`. **Rationale:** running the real command revealed
`ctx-access read` hard-failed on absent `intent.md` — but intent.md is
owner-authored and sparse, not one-per-directory. Context scaffolding is
derived/optional; source is the only mandatory node. Every level is still
surfaced.

## ADR-011 — `ctx-verify` is the verification path
**Status:** scope superseded by ADR-022 (originally `ctx-check`,
read-only, no tests; now renamed and also formats + tests).
**Decision:** a token-frugal broker that wraps the checks and emits one
capped JSON report; passing checks serialize to `{"status":"pass"}` only.
**Rationale:** raw `cargo`/`ci.sh` output cost far more tokens than the
pass/fail + structured diagnostics needed. Built before the reference
project so every iteration is cheap to verify. Use it, not raw cargo.

## ADR-012 — Agents speak a decoupled subprocess contract
**Decision:** `{"system","user"}` JSON on stdin → completion on stdout;
prompts loaded from files at runtime, never embedded; runner adds no
instructions. **Rejected:** embedding prompts / an SDK dependency in the
core. **Rationale:** keeps the core model-agnostic and the prompts
portable and iterable; the SDK/network stress is quarantined behind the
seam (and lands in the reference project, not the tooling).

## ADR-013 — Reference summarizer adapter uses the Anthropic API, not the `claude` CLI
**Decision:** `agents/summarizer-claude.py` (Anthropic Messages API,
stdlib, prompt-cached system block). **Rejected:** shelling the `claude`
CLI. **Rationale:** the CLI injects its own system prompt/initialization,
which contaminates the decoupled-prompt experiment — the adapter must
send exactly `{system,user}` and nothing else. Sonnet default,
overridable. Key via gitignored `.env`.

## ADR-014 — Adopt the richer (`files(9)/`) prompt phrasing
**Decision:** the verbose summarizer/auditor prompts replace the terse
ones. **Rationale:** the extra length is entirely *instruction* (system
prompt), which the adapter sends as a cached block (≈free amortized); the
produced `.ctx`/report schemas and ≤budget were byte-identical. Verbose
*instructions* that tighten output are good; guard *output* budgets, not
prompt length.

## ADR-015 — Reference project relocated outside the repo
**Decision:** `reference-project/` → `../meal-planning/` (standalone).
**Rationale:** a real consumer of the system is its own repo, and a
sibling dir is a clean sandbox boundary (tmpfs over its `src`, bind its
`.context` + `ctx-access`). Tradeoff: leaves CTX git; acceptable and
correct for the test.

## ADR-016 — Sandbox = bwrap namespace; validity is environmental
**Decision:** the cage is a `bwrap` mount-namespace (tmpfs over source,
read-only binds of `.context`/manifest/`ctx-access`, no net), not the
full broker daemon (that's the multi-agent future per `SANDBOX.md`).
**Principle:** a valid test's constraint is *environmental*, not the
agent's discipline. An agent with normal Read/Bash is never a valid
subject (that's the advisory mode the spec disclaims); the builder
process cannot be the caged subject. The agent is a headless `claude -p`
inside the cage; `ctx-access` is its only path to source.

## ADR-017 — Manifest is deny-by-default; enforced in-tool
**Decision:** the discoverable set = `git ls-files ∩ summarizable` minus
a secret/binary denylist; `ctx-access`/`ctx-summarize` hard-refuse
gitignored/secret/binary targets **even if explicitly passed**; plus a
>N-file scope-approval gate. **Rationale:** `ctx-summarize` would today
read and exfiltrate `.env` if asked (only `..`/absolute are blocked) —
the exact "make the bad thing unrepresentable" failure, in our own tool.
Deny-by-default closes it; the scope gate prevents accidental huge/costly
runs.

## ADR-018 — Doctrine is discoverable + pointered, never injected wholesale
**Decision:** binding per-subtree *intent* is injected (it's a chain
node). Rationale/decision logs (`DECISIONS.md`, `DYLINT_RULES.md`) are
manifest-discoverable and read on demand; `intent.md` injects only a
*pointer* to them. They are **never summarized into the tree** (authored
prose, not derived — you want exact rationale on demand). Authored vs.
derived is the split.

## ADR-019 — No non-Rust summarizer prompt (for now)
**Decision:** don't build a prose/markdown summarizer prompt. **Rationale
(follows ADR-018):** doctrine isn't summarized at all; non-Rust *code*
(`agents/*.py`, `scripts/*.sh`) is small/stable/few → manifest-
discoverable and read verbatim via `ctx-access`. Revisit only if non-Rust
code grows materially.

## ADR-020 — `ctx-core` extraction is deferred debt
**Status:** partially resolved by ADR-023 — the *access gate* is now
extracted (it was security-sensitive); the path-safety + `.context`
mapping duplication remains deferred as below.
**Decision:** the repo-relative path-safety + `.context` mapping is
duplicated in `ctx-access` (`repo_path`/`chain`) and `ctx-summarize`
(`cpath`), kept small and identical in spirit. **Rationale:** a shared
`ctx-core` crate is the right end state but coupling the runner to the
access-broker crate now is worse; recorded so the architecture-audit
layer flags it rather than it being silent drift.

## ADR-021 — Length-tier guidance: refactor first, rationale is the backup
**Decision:** `rationale_check.py` messages (passed through verbatim by
`ctx-verify`) and `CLAUDE.md` direct the agent to **fix length-tier hits by
extraction/splitting**, presenting `// rationale:` only as the
last-resort escape when genuinely irreducible — not as the primary
remedy. **Rationale:** the meal-planning findings showed every 30+ line
non-test fn was reducible and the soft tiers correctly drove structure;
leading with "add `// rationale:`" trains agents to paper over instead of
improve. The escape hatch must read like one. Applied to root + template
copies; `../meal-planning/`'s instantiated copy is stale until re-synced.

## ADR-022 — `ctx-verify`: rename + it is the whole agent checkpoint
**Supersedes the scope of ADR-011.** **Decision:** renamed `ctx-check`
→ `ctx-verify` (it mutates — applies `cargo fmt` — and runs tests, so
"check" wrongly implied read-only); it now **formats (apply, first),
builds, lints (clippy/doc/rationale/workspace_lints/no_allow), and
tests** by default; an optional crate-name arg scopes the cargo checks
via `-p`; `--checks`/`--max-diagnostics` remain tight-loop overrides; an
all-pass run serializes to just `{"status":"pass"}` (per-check map
omitted), same token logic as the per-check trim.
**Rejected:** (a) keep it read-only and have agents assemble
`cargo fmt`/`build`/`test` around it — the recurring compound command was
the symptom that prompted this; (b) keep the `ci.sh`-parity "no tests"
scope from ADR-011 — `ctx-verify`'s audience is *agents at a checkpoint*,
not CI parity, and "am I done?" includes tests. **Rationale:** the tool's
entire purpose is one standardized call that answers "is this sound?";
formatting is mechanical/authoritative so the tool applies it (not an
agent judgement), while clippy/rationale stay report-only (they need
judgement). Module-level scoping is intentionally not offered (not a
cargo concept). Renaming touched all crates/docs/memory; the immediate
payoff: it caught a broken doc link the earlier tight runs skipped.

## ADR-023 — Extract `ctx-core` for the access gate (security single-source)
**Decision:** create a tiny, dependency-free `ctx-core` crate holding the
*only* copy of the access gate (`is_secret`/`is_binary`/`deny_reason`/
`accessible_set`); `ctx-access` and `ctx-summarize` both depend on it and
each maps the neutral deny reason into its own typed error.
**Rejected:** (a) duplicate the gate into `ctx-summarize` (the ADR-020
"tolerate small duplication" stance) — rejected because this predicate is
a *security boundary*: a divergent secret denylist between the two crates
is a silent exfil bug, a different risk class than the path-safety
duplication ADR-020 deferred; (b) make `ctx-summarize` depend on
`ctx-access` — ADR-020 already rejected coupling the runner to the
access-broker crate. **Rationale:** smallest change that removes the
dangerous duplication; `ctx-core` is a leaf crate (no deps, no coupling),
so the objection that blocked the full extraction does not apply here.
The deeper `RepoPath`/`cpath` extraction stays deferred (ADR-020).

## ADR-024 — `ctx-verify`: `errored` ≠ `fail`; never a silent result
**Decision:** a check that cannot be *executed* (spawn/infra `Err`, not
tool-missing) is `Status::Errored` carrying the underlying message — not
`fail`; a check that ran, failed, and produced no parseable diagnostics
gets a synthesized stderr-tail hint. `errored` outranks `fail` in the
overall status ("I could not verify" is more urgent than "your code
failed"). **Context:** a transient spawn failure once made three script
checks report `{status:fail,count:0}` — indistinguishable from a real
bare failure and non-deterministic across runs. **Rationale:** a
verification tool must be deterministic in interpretation and must never
report a failing check with zero information; conflating
infrastructure failure with code failure trains agents to distrust or
misread the gate. Execution is strictly sequential (blocking
`Command`s); the defect was error *classification*, not timing.
**Amended by ADR-025:** the triggering incident was *not* a spawn
failure — the scripts ran and aborted mid-filesystem-walk. ADR-024's
mechanism stands as defense-in-depth (any inconclusive check must still
be legible); ADR-025 removes the actual cause.

## ADR-025 — Static checks enumerate via `git ls-files`, not a FS walk
**Decision:** the three static checks (`rationale_check.py`,
`workspace_lints_check.sh`, `no_allow_check.sh`) enumerate their inputs
with `git ls-files -z --cached --others --exclude-standard` (tracked +
untracked-but-not-ignored), `cd`'d into the root, after a
`git rev-parse` guard; no `rglob`/`find`/`grep -r` filesystem walk.
**Context:** the incident behind ADR-024 was a `{status:fail,count:0}`
triple that flipped to pass on the next run with no source change
(observed sequence: verify → standalone checks → verify). Root cause,
found by reading the scripts: their `target/` exclusion *filtered* but
did not *prune* — `find … -not -path '*/target/*'` is a test not
`-prune`, and Python `rglob("*.rs")` descends into `target/` before the
skip. So each "static" check's input set was a function of the live
build tree, not of source. When that walk raced a concurrent `target/`
writer (cargo's own post-step churn, or rust-analyzer in the open IDE —
an independent writer ctx-verify's sequential execution does not gate),
an entry vanished between readdir and stat; `find`/`rglob` aborted
non-zero with no `FAIL:` line → the count:0 triple. Quiescent `target/`
on the next run → pass. **Rationale:** a verification result must be a
pure function of committed (or at least repo-known) source, independent
of build state, the IDE, or any concurrent process. git prunes ignored
paths itself (`target/` is gitignored — verified), so the inspected set
is deterministic; it also matches the project's existing
manifest/deny single-source-of-truth (ADR-023), which already keys on
`git ls-files`/`check-ignore`. `--others --exclude-standard` keeps
in-progress (uncommitted, non-ignored) source covered, so the gate is
not weakened during active development. **Rejected:** `find … -prune`
(a weaker stopgap that still trusts the working tree and diverges from
the git-tracked model); leaving ADR-024 as the only mitigation (it
makes the phantom *legible* but does not stop it recurring — masking,
not fixing). **Scope:** `cycle_check.sh` (uses `cargo modules`, no
walk) and `ci.sh` (only delegates) are unaffected; both `scripts/` and
`template/scripts/` updated in lockstep.

## ADR-026 — Cage MVP: bwrap ns + UNIX-socket forwarder to host ctx-access
**Decision:** the MVP cage (`sandbox/`) is a `bwrap` mount+net namespace
where `../meal-planning` is mounted **read-only** with
`crates/mealplan/{src,tests}` and `target/` replaced by empty `tmpfs`
(source genuinely absent from the agent FS), `--unshare-net` (no
network), and the in-cage `ctx-access` is a ~20-line forwarding client
that base64-ships argv over a bound UNIX socket to a host-side broker
(`socat …,fork EXEC:broker-handler.sh`) that runs the **real**
`ctx-access` in the real tree. **Context:** STATUS framed Cage C as
"bind the `ctx-access` binary in"; but a single uid/namespace cannot
both hide source from the shell and let an in-cage `ctx-access` read it
— that is precisely SANDBOX.md's "why the CLI cannot do this itself".
**Rationale:** this realizes SANDBOX.md's client/broker **transport
seam** (the only thing SANDBOX.md said to build at MVP) at the minimum
that *proves the property*: enforcement (deny gate, repo-boundary,
write-requires-prior-read, lifecycle) stays host-side in `ctx-access`,
so the cage cannot weaken it — verified by `cage-adversary.sh` (secret,
`../../../etc/passwd`, absolute path, blind write, bogus task all denied
through the forwarder). **Supersedes** STATUS's literal "bind the
binary" wording. **Rejected:** binding source at an obscure in-ns path
(a `find`/`/proc/mounts` away — fails even the *lazy* bar); building the
production broker now (ADR's `ctx` uid + cache ownership + locking
remains deferred per SANDBOX.md/UNIMPLEMENTED — explicitly NOT MVP).
**Residual (accepted):** same uid both sides; the threat model is
*capable & lazy, not adversarial* (SANDBOX.md) — the `cat src/foo.rs`
shortcut is closed hard; uid separation is the deferred production
broker. **Prerequisite recorded:** the reference project must be a git
repo — `ctx-access`'s gate/manifest single-source on
`git ls-files`/`check-ignore` (ADR-023) and the static checks on
`git ls-files` (ADR-025); meal-planning lost its `.git` on relocation
(ADR-015) and was re-init'd.

## ADR-027 — A dry-run that mutates the reference tree is itself a defect
**Decision:** Cage D's no-spend proof must be strictly non-mutating; the
adversary runs under its **own fresh task with zero served reads**, the
blind-write probe targets a non-existent scratch path, and
`cage-demo.sh` asserts host-tree integrity (`git status` on
`crates`/manifests, no stray probe) after every run — auto-restoring and
failing loudly on any mutation. **Context:** the first adversary draft
reused the reachability task-id; the stub's *legitimate* read of
`profile.rs` satisfied write-requires-prior-read, so the "blind" write
**succeeded and overwrote real source** (recovered via
`git checkout` — only possible because the project is now a git repo).
The invariant held; the *test* was unsound and destructive.
**Rationale:** a verification harness that can corrupt the subject it
verifies is a worse failure than the bug it hunts; isolation of test
state and a post-condition integrity gate are mandatory, not optional.
Related: [[ADR-024]] (never a silent/misattributed result) — same
principle applied to the test harness itself.

## ADR-028 — Agent run: brokered {ctx-access, ctx-verify}, egress 1a, dual mode
**Decision:** the real run extends the cage ([[ADR-026]]) so the broker
allowlist is **`{ctx-access, ctx-verify}`** (not ctx-access alone): the
caged `ctx-verify` is a forwarder too, the real one runs host-side
against the tree the agent's `ctx-access write`s land in — so the cage
needs no cargo/rustc/source. One generalized forwarder (`tool-client.sh`,
tool = `argv0`) is bound as both names. **Model egress = 1a** (owner
decision): the real caged `claude` reaches the API directly (`--net` +
`ANTHROPIC_API_KEY` in cage env); accepted residual under the
*capable-but-lazy, not adversarial* threat model — the key is auth, not
prompt content, and never committed/`.env`-sourced into a model.
**Auth mechanism amended by [[ADR-029]]:** this host's `claude` is
subscription/OAuth-authed (no API key); the credential is the bound
`~/.claude/.credentials.json`, not an env key.
**Two modes:** headless `claude -p` is the **validity-bearing** run
(environmental *and* behavioral validity, ADR-016); `--interactive`
relays a **dedicated** cage pty via `pty-relay.py` (the cage never gets
the real terminal, so TIOCSTI escapes are contained even with
`--new-session`) and is for observation only — human steering forfeits
*behavioral* validity though *environmental* validity still holds.
**Spend:** two gated boundaries — the agent loop (`CTX_CAGE_ALLOW_SPEND=1`,
never set by the dry-run) and `end-task`'s audit. A no-spend
`stub-claude.sh` proves the whole loop (init→verify→read→write→verify→
shutdown→host-acceptance) wires end-to-end; the only delta to the billed
run is swapping the stub for `claude`. **Rejected:** brokering the model
call too (option 1b — owner chose 1a; recorded as the hardening path,
not MVP); host-side-only acceptance verify (the agent's *own* checkpoint
is doctrine — it must be able to call `ctx-verify`, hence brokered).
**Found & fixed building this:** (1) `socat`'s default 0.5s half-close
timeout reaped slow brokered `ctx-verify` (silent multi-second compile)
after the one-line request EOF'd → empty result; both ends now `-t
86400`. A fast call (`manifest`/`read`) masked it — only a slow brokered
tool exposed it. (2) meal-planning carried the **pre-[[ADR-025]]**
walk-based scripts; the brokered `ctx-verify` would race the agent's own
`target/` rebuild — synced from `../template/scripts/` (prereq:
meal-planning is a git repo, [[ADR-023]]). (3) `pty-relay.py` must not
kill the child on local stdin EOF (non-interactive driver) — it stops
forwarding input but relays output until the child exits.

## ADR-029 — Agent auth = bound subscription credential; no-spend preflight
**Decision:** the caged `claude` authenticates via the host user's
existing **subscription/OAuth** credential — ONLY
`~/.claude/.credentials.json`, bind-mounted **read-only** into the
cage's `$HOME/.claude/` (nothing else from `~/.claude`, to preserve
blinding). No `ANTHROPIC_API_KEY`, no `--bare` (mutually exclusive with
OAuth: under `--bare`, claude reads strictly the env key). `cage-run.sh
--claude` provisions the real runtime: the 237 MB `claude` ELF bound on
PATH, DNS/TLS plumbing (`resolv.conf`, `/etc/hosts`, `/etc/ssl`, and a
**deterministic minimal `nsswitch.conf`** — the host's pulls
systemd-only NSS plugins whose sockets are absent in the cage), and the
credential. Headless spend uses `claude -p --permission-mode
bypassPermissions` (autonomous tool use; the flag's own "no-internet
only" guidance is knowingly traded against the accepted 1a residual).
**Context:** the first real launch died `execvp claude: No such file`
— the binary was never bound (only the in-cage stub had run), and the
`--net` branch additionally lacked DNS/TLS. `--pass-key` had assumed an
API key the owner doesn't have. **Rationale:** the run needs *a*
credential because the agent really calls the paid API (the dry-run
stub never did — that is the only spend difference); the owner chose
the subscription over a dedicated key, so the build follows it. A new
**`claude-preflight.sh`** (`agent-demo.sh --preflight`) proves the
entire real environment with **zero spend**: `claude --version` runs,
the credential is present, DNS + a TLS *handshake* to
api.anthropic.com:443 succeed (a handshake is not a billed request — no
HTTP is sent), the source jail still holds under `--net --claude`, and
the broker is reachable. **Residual:** a live OAuth token is visible to
an autonomous networked agent while it runs — same residual class as
1a, now concretely the owner's subscription, accepted by explicit owner
choice. **Rejected:** dedicated API key + `--bare` (cleaner blinding,
metered — offered, owner chose subscription); binding all of
`~/.claude` (leaks projects/history/settings — would break blinding,
ADR-016). Supersedes [[ADR-028]]'s auth clause; everything else in
ADR-028 stands.

## ADR-030 — Cage env is cleared; onboarding pre-satisfied from a synthesized config
**Decision:** the cage launches with `bwrap --clearenv` and an
explicit minimal env (`PATH HOME USER LANG TERM CTX_SOCK TASK`; in
`--claude` also the synthesized config). For `--claude`, `cage-run.sh`
synthesizes a **minimal `~/.claude.json`** — `hasCompletedOnboarding:
true`, pre-trusted `/work`, and ONLY the host's `oauthAccount` object —
bound rw but ephemeral (under the harness temp dir), alongside the RO
`.credentials.json`. **Context:** the first real interactive launch hit
Claude Code's first-run wizard (theme/login/trust) **and** a "use the
detected `ANTHROPIC_API_KEY`?" prompt. Two root causes: (1) `bwrap`
inherits the parent environment by default, so the host's
`ANTHROPIC_API_KEY` (from CTX's `.env`) leaked into the cage — both the
source of that prompt **and** a blinding leak ([[ADR-016]]); (2) the
cage's fresh `HOME` had no `~/.claude.json` (separate from
`~/.claude/`), so claude treated it as a first run. **Rationale:**
`--clearenv` removes the leak (no key visible → no prompt; the bound
OAuth credential is then used silently) and hardens blinding for
*every* mode, not just `--claude`. The synthesized config skips
onboarding deterministically and carries the account object so the
credential auto-detects (no login prompt) — verified empirically
against the host config's schema (`hasCompletedOnboarding`,
per-project trust map). **Proven no-spend** by `agent-demo.sh
--check-onboarding`: interactive claude, immediate `/exit`, capture
ANSI-normalized (the TUI separates words with cursor-forward, not
spaces) and asserted to show the authenticated returning-user UI with
no wizard / no key prompt — reaching the TUI without submitting a
message is not a billed call; the probe then times out and is killed,
which is expected, not a failure. **Residual:** `oauthAccount`
(account email/org metadata, not the token) enters the synthesized
config so auth auto-detects — same accepted residual class as
[[ADR-029]] (the owner's subscription identity is used by explicit
choice); still nothing else from `~/.claude` (no projects/history), so
blinding is otherwise intact. **Rejected:** `--bare` (would skip
onboarding but forces API-key auth — incompatible with the chosen
subscription path, [[ADR-029]]); binding the real `~/.claude.json`
(leaks projects/history — breaks blinding). Extends [[ADR-029]];
ADR-028/029 otherwise stand.

## ADR-031 — MVP environmental-validity: PROVEN (billed run); two harness fixes
**Result:** the MVP claim holds. A **blinded Sonnet agent** (different
model, no CTX context), caged (no source on disk; only brokered
`ctx-access`/`ctx-verify`; egress 1a), completed the real chosen task —
a `profile edit` subcommand for `mealplan` — and `ctx-verify mealplan`
returned `{"status":"pass"}`, **independently re-verified host-side**
(77 insertions across `cli/mod.rs`+`handlers.rs`: `ProfileEditArgs`,
`ProfileCmd::Edit`, `apply_profile_edit`). The cage, generalized broker,
blinding, lifecycle, and spend gates all held under a real autonomous
agent. **This closes the loop ([[ADR-016]]): validity is
environmental, and the environment held.** **Observed friction (the
point of the run):** the agent burned turns on `cat`/`wc` (returned
empty — the jail working) before settling on re-`read`; assumed
`ctx-verify --task-id …` (arg-shape divergence from `ctx-access`) and
self-corrected to `ctx-verify <pkg>`; correctly hit and recovered from
write-requires-prior-read (`write denied: source not read in task` →
re-`read` → `write`); and **refactored** to satisfy the length tiers
rather than reaching for `// rationale:` ([[ADR-021]] behaving as
intended on an agent that never saw CLAUDE.md). **Fix 1 — provision the
project's own house rules:** added `meal-planning/CLAUDE.md` (the
caged-agent operating rules). `/work` is a read-only bind (only
`src`/`tests`/`target` are tmpfs), and Claude Code auto-discovers
`/work/CLAUDE.md` (we do **not** use `--bare`, [[ADR-029]]). This is
legitimate context-provisioning, **not** a blinding breach: it is the
documented onboarding a real teammate gets; the source jail is
untouched; and the agent demonstrably *succeeded without it*, which is
stronger evidence, not weaker. The brief shrinks to just the task.
**Fix 2 — `--dangerously-skip-permissions`** replaces
`--permission-mode bypassPermissions`: the latter triggers a one-time
interactive "accept bypass mode" gate that blocked the autonomous run
(the operator had to approve by hand). The cage is precisely the
sandbox that flag asks for; its "no internet" guidance is the
knowingly-accepted 1a residual ([[ADR-028]]/[[ADR-029]]). **Rejected:**
setting an undocumented `bypassPermissionsModeAccepted` config key
(version-coupled, fragile) — the explicit flag is self-documenting.
**Verification:** both fixes proven no-spend — `agent-demo.sh
--preflight` now also asserts `/work/CLAUDE.md` is present;
`--check-onboarding` already proved the auth/env path. The fixes make
the run reproducible and clean; they do not affect the (already
achieved) validity result. Extends [[ADR-026]]/[[ADR-028]]/[[ADR-029]]/
[[ADR-030]].

## ADR-032 — Validation scope correction: the context chain was never served
**Decision/Correction:** [[ADR-031]]'s "MVP validated — loop closed"
**overclaimed** and is amended here. Investigation (prompted by the
operator noticing "nothing in context"): meal-planning's `.context`
summary tree **does not exist** — zero `*.ctx`, no `intent.md`, never
committed (`.ctx tracked: 0`), unrecoverable. It was produced by an old
billed `ctx-summarize`, never committed, and wiped during the project's
git-init/clean. **The tools are exonerated:** planted dummy
`rollup.ctx`/`intent.md`/`*.ctx` are served correctly and **survive**
`init-task --force` and a full no-spend run; `(absent: …)` is the
designed soft-marker ([[ADR-010]]) for a missing node, not a bug. **So
what the billed run actually proved:** the cage, broker, deny-gate,
write-requires-prior-read, lifecycle, blinding, and that a blinded
agent can complete a real task through the constrained interface — all
real, all stand. **What it did NOT prove:** CTX's central thesis, that
a *summarized context chain* is sufficient/useful context. At run time
the chain served only `intent.md` (+101 lines) and **no rollups, no
leaf summaries**; the agent built from **raw source**. The
chain-value claim is **unvalidated**. **Systemic cause:**
meal-planning's `.gitignore` explicitly states "Generated context files
(rollup.ctx, *.ctx, intent.md) ARE committed" — yet they never were;
doctrine stated, not followed, so a routine clean erased the entire
value layer silently and the gap was invisible until inspected.
**Corrective (gated):** (1) regenerate the summary tree via
`ctx-summarize` — a **billed** model operation, explicit-go only; (2)
**commit** the regenerated tree (close the stated-but-unfollowed
doctrine; a tracked tree cannot be silently cleaned and the deny-gate
serves tracked files); (3) re-run the caged agent so it actually works
from the chain — only then is the thesis end-to-end validated.
**Process lesson:** "it passed" is not "it was exercised"; a green
result whose key input was absent is a hollow pass. Validation harnesses
must assert their preconditions (the summary tree present/non-empty)
before claiming the thing they exist to prove — same family as
[[ADR-024]]/[[ADR-027]]. Amends [[ADR-031]] (scope only; the
access/cage results there stand).

## ADR-033 — Thesis validated end-to-end: chain present, committed, used
**Result:** with the corrective in [[ADR-032]] applied, the full MVP
thesis is now validated. Sequence: (1) regenerated meal-planning's
`.context` via `ctx-summarize` (billed; 18 leaf + 6 rollup nodes via
the documented `.env`/`summarizer-claude.py` adapter); (2) **committed**
the tree (`meal-planning bfc7280`) — closing the stated-but-unfollowed
"`.ctx` ARE committed" doctrine so a clean can never silently erase the
value layer again, and a tracked tree is what the deny-gate serves; (3)
re-ran the caged billed agent. It produced a clean `profile edit`
(`EditProfileArgs`, `ProfileCmd::Edit`, `apply_edit`, `run_profile`
dispatch; 77 insertions), `ctx-verify mealplan` → `{"status":"pass"}`
(independently re-verified host-side), end-task audit **0
divergences**. **Evidence the chain reached the agent (vs. the
raw-source-only first run):** the committed tree makes `ctx-access read`
serve the full non-absent rollup→leaf prefix for every edited file
(reproduced host-side verbatim: root rollup "integer-only; no
floating-point… all public APIs return `Result<_, MealError>`", crate
and dir rollups, leaf); the caged agent's *only* source path is that
same brokered tool against that same committed tree, so it
*necessarily* received the chain this run, where the first served
`(absent)` everywhere; corroborated by the implementation honoring
invariants stated in the **summaries** (re-validate via `Profile::new`,
integer-only, `Result`/`MealError`) and the auditor's zero divergences.
**Honest caveat:** headless `claude -p` emits its narration, not raw
tool stdout, so this is environmental + corroborating proof, not a
verbatim capture of the agent printing a rollup. A stricter artifact
(an `--interactive`/`--output-format stream-json` run that records the
served bytes the agent consumed) is available if ever wanted; it was
judged unnecessary given the structural certainty (brokered sole path +
committed non-absent tree) and the contrast with [[ADR-032]].
**Status:** the access/cage claims ([[ADR-031]]) and the thesis claim
(this ADR) now both hold, on a committed, reproducible baseline. The
agent's `profile edit` deliverable remains uncommitted in meal-planning
(validation output; the operator may keep or discard it). Deferred work
(production `ctx`-uid broker, Layer 3) unchanged in `UNIMPLEMENTED.md`.
