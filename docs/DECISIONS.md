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

## ADR-034 — Cage is a Rust crate (`ctx-cage`); Bash sandbox retired
**Decision:** the cage moves from `sandbox/*.sh`+`pty-relay.py` to
**`crates/ctx-cage`** — a new workspace crate under the same lint
regime and `ctx-verify` gate as every other crate. Two binaries:
`ctx-cage` (host orchestrator) and `ctx-cage-client` (busybox-style
in-cage forwarder, bound twice as `ctx-access` and `ctx-verify`).
Modules: `protocol` (length-prefixed JSON request + tagged
output/exit frames), `broker` (`UnixListener` + `Spawner` seam),
`bwrap` (pure argv builder + crate auto-discovery), `cli` (clap + a
spend gate that *enforces* `--allow-spend`), `lifecycle` (prepare →
serve → teardown), `runtime` (host-side `--claude` resolution +
synthesized `~/.claude.json`), `summarize` (stale-detect +
`ctx-summarize` invocation, billed-gated), `spawn`, `error`. Embedded
assets: `cage-rules.md` (`include_str!`) + `cage-nsswitch.conf`.
**Context:** the Bash transport was the only substantial piece of
CTX *outside* its own quality regime — and it had real bugs the new
crate eliminates by construction: the `socat -t 0.5s` half-close
reaper ([[ADR-028]]) is gone (a real binary protocol with explicit
EOF), the base64+`__CTXRC__N` argv encoding is gone (length-prefixed
JSON), the destructive `git checkout crates` revert ([[ADR-027]]) is
gone (the lifecycle's stub probe writes to a known target and the
spend branch keeps deliverables — no blanket revert anywhere). The
target project is parameterized with **no default** (per the user's
"broader tool" direction); crate src/tests dirs are auto-discovered
under `<target>/crates/*`. **Delivered in seven turns**, each landing
`ctx-verify` `{"status":"pass"}`; the parity smoke
(`ctx-cage <meal-planning> --self-test stub`) prints
`SELF-TEST-STUB-OK`. **Retired:** `sandbox/{agent-demo,cage-run,broker,
broker-handler,tool-client,cage-demo,stub-agent,cage-adversary,
claude-preflight,stub-claude}.sh`, `sandbox/pty-relay.py`,
`sandbox/cage-nsswitch.conf` (the canonical copy now lives at
`crates/ctx-cage/assets/cage-nsswitch.conf`). **Deferred (turn 6b
backlog):** dedicated-PTY isolation for `--interactive` via
`portable-pty`. Today `--interactive` inherits the parent's
controlling tty and drops `--new-session`; sound under kernel
`dev.tty.legacy_tiocsti=0` (the host's posture). **Rejected:**
a hybrid "ship Bash now / port later" path (the user chose
"single commitment"). **Supersedes** the Bash transport described by
[[ADR-026]] (the cage's architecture and threat model from ADR-026
stand; only the implementation language changed).

## ADR-035 — Owner-authorized re-spec: lead-by-hooks replaces enforce-by-cage
**Decision:** CTX stops *enforcing* top-down context reading via
source-masking and a custom write tool, and starts *leading* it: agents
use native Read/Edit; a Claude Code `PostToolUse` hook injects the
context chain on every read; the cage survives purely as a safety
boundary for autonomous runs. The context model (path-keyed `.context`
mirror tree), `ctx-verify`, `ctx-core`, and the Layer-1 lint regime are
unchanged. Authorized per [[ADR-001]]; executed 2026-07-01/02 as SPEC
revision 4. **Context (the three frictions):** heavy init ceremony
(task ids, cage-only tools), whole-file echo on every `write` (the
"one file per turn" output-cap workaround), and leaf summaries that
over-weighted invariants and didn't compose into task-level
understanding. **Rejected:** keeping the enforcement cage — its
environmental guarantee ([[ADR-016]]) was real but priced every edit;
the pivot trades "impossible to skip context" for "free to receive
context", betting the chain's *value* (validated in [[ADR-033]]) makes
voluntary consumption sufficient. **Execution lesson:** stale doctrine
actively misleads — a caged agent followed the old CLAUDE.md into the
deleted `ctx-access` flow (a stale binary in `target/debug` completed
the illusion). CLAUDE.md was rewritten (cdoc-lean, owner-trimmed),
`cage-rules.md` gained an explicit "these rules win over stale project
docs" precedence clause, and orphaned binaries were removed.

## ADR-036 — `ctx-access` shrinks to `ctx-context`: read-only chain, no lifecycle
**Decision:** the crate is renamed `ctx-context`; one command:
`ctx-context <path>` prints the rollup+intent chain root→target (a
directory target — or `.` — serves its own level, so directory
summaries are on demand; a file target ends with its leaf `.ctx`). No
source bytes, no `--task-id`. Deleted: the `write` path,
`init-task`/`end-task`, the `served_nodes` cache, manifest and report
modules. Soft absent-markers stay ([[ADR-010]]). Per-session
deduplication — the useful remnant of `served_nodes` — relocates to
the hook edge as `.context/.cache/hook-<session-id>.json`, keyed by the
event's `session_id`, so repeated reads inject only the chain delta.
**Rejected:** keeping write-needs-read enforcement in an advisory tool
(without the cage's masking it enforces nothing and taxes everything).
**Supersedes** the mechanisms of [[ADR-007]]/[[ADR-008]]/[[ADR-009]];
ADR-007's principle (the chain passes through context before source is
touched) survives, now delivered by injection rather than by gating.

## ADR-037 — Hooks are the forcing function; the read hook is fail-open
**Decision:** committed `.claude/settings.json` wires `PostToolUse`
(`Read|Grep|Glob`) → `ctx-context --hook`, which reads the event JSON,
resolves the repo root from the event's own `cwd` (harnesses differ in
process cwd), and emits `hookSpecificOutput.additionalContext`.
Failure posture (owner decision): **fail-open, loudly** — chain errors
inject an explicit `(chain unavailable)` marker; unparseable input,
out-of-repo targets, and `.context`/`.git` reads stay silent; nothing
ever blocks the read. **Rejected:** fail-closed — a `PostToolUse` hook
cannot block (content is already served), so true fail-closed means a
`PreToolUse` deny that turns every gap in the summary tree (fresh
dirs, docs, scratch) into a hard stop; ADR-032's lesson is honored by
making absence *visible*, not fatal. **Operational notes:** hooks in
project settings need one-time review via `/hooks`; verification is
the transcript or the dedup state file — asking the model whether
context "was injected" is unreliable (observed with a haiku agent
denying an injection the state file proved).

## ADR-038 — Freshness = content-hash tree (CACT) + `.ctxignore`, not git
**Decision:** each mirrored directory carries
`.context/<dir>/hashes.json`: leaf entries are the SHA-256 of source
bytes, the directory hash is the SHA-256 of its sorted child entries,
so any change propagates to the root. `ctx-scan <dir> --check` diffs
stored vs recomputed with no model call; `--update` regenerates
exactly the stale leaves and rollups (leaf-up), deletes orphaned leaf
`.ctx` files, and rewrites sidecars — still behind the `MAX_TARGETS`
`--approve` cost gate. Summarization scope is `.ctxignore` (gitignore
syntax; falls back to `.gitignore`; built-in `target/`), evaluated by
the `ignore` crate — git-independent, so the walker's
`git check-ignore` subprocess is gone; the `ctx-core` secret/binary
deny still applies on top and cannot be un-ignored. **Rejected:** git
diff as the change signal — it couples summary freshness to commit
state and misses gitignore-invisible edits. **Residual:** a deleted
*directory's* `.context` subtree is not auto-pruned (delete `.context/`
and rescan is the documented remedy). New deps: `sha2`, `ignore`
(licenses allowlist-clean; no duplicate versions).

## ADR-039 — Leaf summaries KEPT; both prompts rewritten cdoc-style
**Decision:** leaf `.ctx` files stay in the pipeline (they remain the
rollup assembler's input and the file target's chain tail), but both
summarizer prompts are rewritten as *context documents* in the owner's
Cdoc spec sense: lead with behavior in domain terms, then a new
`edit_notes` section ("what you must know before changing this"),
functions, then invariants demoted to load-bearing-only;
self-contained facts, no history (the hash tree owns freshness),
mandatory deduplication pass. Budgets unchanged (leaf 10/40, rollup
15/40). **Context:** the pivot draft dropped leaves; review surfaced
that rollups would then be synthesized from raw source (a real token
and quality change), and the owner chose keep-but-restyle instead.
**Supersedes** [[ADR-014]]'s prompt content, not its verbose-
instructions-cheap principle.

## ADR-040 — The cage is a safety boundary, not an enforcement mechanism
**Decision:** `/work` is bound **read-write** (the agent edits the
real tree with native tools); deleted: the broker, framed socket
protocol, `ctx-cage-client` forwarder, tmpfs source-masking, and crate
discovery. Containment: workspace-only writes (`/tmp` aside), RO
toolchain binds (`~/.cargo`, `~/.rustup` at identical paths,
`CARGO_NET_OFFLINE=true`), secrets masked inside the workspace
([[ADR-042]]), nothing else from `$HOME`, fresh
user/pid/ipc/uts/net namespaces, `--clearenv`, `--die-with-parent`,
bwrap's unconditional `no_new_privs`. Recovery is plain git: billed
runs refuse a dirty tree (`--allow-dirty` overrides) — no snapshot
layer. `ctx-run <dir> "<task>"` is the one-command billed launcher
(typing it *is* the explicit spend go; `ctx-cage` keeps the
`--allow-spend` gate and the free `--self-test stub` containment
probe). Host CTX tools are bound at `/cage/bin` as real binaries — no
forwarders, since there is nothing left to broker. **Supersedes** the
transport of [[ADR-026]] and the broker/protocol modules of
[[ADR-034]] (its cage-is-a-lint-clean-Rust-crate decision stands).
**Residuals (recorded, not wired):** no seccomp filter yet; dedicated
PTY isolation for `--interactive` still deferred (ADR-034 backlog).

## ADR-041 — Egress: offline cage + host passthrough proxy; auth = subscription
**Decision:** the cage is always `--unshare-net`; the sole egress is a
host-side proxy on a UNIX socket bind-mounted at `/run/ctx/api.sock`,
reached from inside via a `socat` relay on `127.0.0.1:8080`
(`ANTHROPIC_BASE_URL` points there). The proxy rewrites each request
head (upstream `Host`, `Connection: close`) and dials
`api.anthropic.com:443` through a verified-TLS `socat` child behind an
injected `Upstream` seam — no TLS stack enters the crate; the pure
header-rewrite is unit-tested over socketpairs. **Auth:** the agent
uses the operator's Claude Code **subscription** ([[ADR-029]]
reaffirmed): `~/.claude/.credentials.json` bound RO at the cage HOME,
synthesized `~/.claude.json` carries only `oauthAccount` + onboarding
pre-completion; the proxy passes `Authorization` through. A
key-injection mode (`ProxyConfig.api_key: Some`) exists and is tested
but unwired — the pivot brief specified it, the owner then corrected
that no metered agent key exists; the env-file key is summarizer-only.
**Rejected:** full network with direct API egress (the accepted 1a
residual of [[ADR-028]] — strictly worse than a single-host relay).
**Residuals:** an OAuth token refresh mid-session would target a host
the single-endpoint proxy doesn't serve (credential is bound fresh at
launch; observed lifetimes exceed sessions); the claude binary is
additionally bound at `/tmp/.local/bin/claude` because the installer
health-check probes `$HOME/.local/bin` and warned on every launch.

## ADR-042 — Secret masks are an empty regular file, never `/dev/null`
**Decision:** `.env` and `.git/config` (when present) are masked with
an RO bind of an empty regular file minted in the run dir; masked
paths must read as empty, not fail to open. **Context:** the first
real caged run died in git — bwrap bind mounts carry `nodev`, so a
`/dev/null` mask makes the path an unopenable device node (EACCES),
and git parses config on nearly every command. **Rationale:** a mask
that breaks its readers is a usability bug wearing a security costume;
hiding content and preserving readability are both requirements. The
`--self-test stub` probe now asserts mask readability and in-cage
`git status` usability — same family as [[ADR-024]]/[[ADR-027]]:
harnesses must assert the property they exist to provide, not a
proxy for it.

## ADR-043 — Summary regeneration has one owner: post-session
**Decision (owner):** the Stop hook is **report-only** — it recomputes
staleness (free) and emits a `systemMessage` naming what is stale and
the exact refresh command (pre-hinting `--approve` when the backlog
exceeds `MAX_TARGETS`, so following the suggestion never dead-ends on
the cost gate). Regeneration happens in exactly one place: after the
session — `ctx-run`'s post-run `ctx-scan --update`, or the same
command run manually. The refresh never fails the run (the session's
deliverable is not hostage to maintenance; failures warn with the
manual command) and never passes `--approve` itself — the gate is
crossed only by a human typing it. **Context:** the first wiring
regenerated from the Stop hook when spend env vars were set, while
`ctx-run` also refreshed post-run — two owners; and the Stop event
fires at the end of *every turn*, so mid-session regeneration both
bills repeatedly and races the agent's half-finished edits. The owner
framed the fix: regenerate between sessions, with finalized states at
both ends of the user's input→output loop.

## ADR-044 — The per-target deny gate keys on `.ctxignore` too, not git
**Decision:** `ctx-summarize`'s `StdFs::is_ignored` — the `ignored`
input to `ctx_core::access::deny_reason` — no longer shells
`git check-ignore`; it evaluates the same scope matcher as the walker
(`.ctxignore`, else `.gitignore`, built-in `target/`, parent-dir
semantics via `matched_path_or_any_parents`). The matcher builder moved
into `ctx_summarize::fs::scope_matcher` and `ctx-scan`'s walker now
reuses it, so scope has exactly one implementation. The neutral deny
reason renames `"gitignored"` → `"ignored"`. **Context:** [[ADR-038]]
made the walker git-independent but left the per-target gate on git —
two scope authorities that could disagree (a path excluded by
`.ctxignore` but not by git would pass the gate when targeted
directly), and the gate failed outside git repositories. **Rejected:**
keeping the git subprocess as "defense in depth" (a second, weaker
definition of scope is drift, not depth — the ADR-023 single-source
argument applies); putting the matcher in `ctx-core` (it would break
that crate's deliberately dependency-free posture; the secret/binary
denylist single-source is untouched). Completes [[ADR-038]].

## ADR-045 — `.ctxignore` is seeded once from `.gitignore`, then sole authority
**Decision:** the scope matcher reads **only** `.ctxignore` (plus the
built-in `target/`); the `.gitignore` *fallback* is removed. Instead,
`ctx-scan` seeds a missing `.ctxignore` on first contact — `.gitignore`
copied verbatim under an explanatory header (bare header when there is
no `.gitignore`) — and after that one-time hand-off `.gitignore` is
never consulted. **Context:** the owner observed `--check` results
flipping as `.gitignore` was edited: with no `.ctxignore` on disk the
fallback made git state a live, invisible scope input — precisely the
coupling [[ADR-038]] set out to remove. **Rationale:** seed-once makes
the scope an explicit, versionable artifact that answers "what is in
scope?" by opening one file; determinism beats convenience-fallback.
**Rejected:** keeping the live fallback (hidden coupling); refusing to
run without a `.ctxignore` (hostile first contact); consulting both
files (two authorities again — [[ADR-044]]). **Tradeoff (accepted):**
even read-shaped modes (`--check`, `--dry-run`) write the seed file on
first contact — a visible one-time artifact beats results that depend
on which file happens to exist. Amends [[ADR-038]]/[[ADR-044]].

## ADR-046 — The workspace is bound at its own real host path, not a fixed alias
**Decision:** `add_workspace()` binds the target project RW at its own
`target_root` (source path == destination path — the same pattern
`add_toolchain()` already used for `~/.cargo`/`~/.rustup`), and
`--chdir`/secret-mask destinations move with it. The fixed cage-internal
alias `WORK_DIR = "/work"` is deleted; the synthesized `~/.claude.json`
trust-map key and the `--self-test stub` probe follow suit (the latter
made cwd-relative instead, so it never needs to know the path at all).
**Context:** under [[ADR-040]] the agent runs `cargo`/`rustc` natively
inside the cage against the RW-mounted tree, and `target/` persists
across sessions as part of that same tree (not tmpfs). Rust's
`env!("CARGO_MANIFEST_DIR")` bakes the cage-internal path into compiled
test binaries at build time; when a binary built under the `/work`
alias was later run outside the cage (or vice versa), a runtime path
lookup relative to that baked-in value silently resolved wrong —
observed concretely on an external project where a fixture-count
assertion passed inside the cage and failed outside it against the
identical, correctly-staged file. The `/work` alias itself predates
this hazard: it was designed under [[ADR-026]]/[[ADR-028]], where
source was tmpfs-masked and all building/verification ran host-side
through a broker, so nothing ever compiled against the cage-internal
path. [[ADR-040]] deleted that broker and moved compilation inside the
cage without revisiting whether the fixed alias was still safe to
carry forward. **Rationale:** a compiled artifact must see the same
path in and out of the cage, or its build-time assumptions are
silently invalid outside the sandbox that produced it — a correctness
fix, not a hardening measure; the cage's actual containment (offline,
RO toolchain, secret masks, cleared env, unshared namespaces) is
unaffected by what string names the mount point. **Tradeoff
(accepted):** the agent's cwd (and anything it prints or bakes into
build output) now reveals the real host path — e.g. the OS username or
directory layout — instead of a blinded `/work` label. This was never
a deliberate, documented blinding guarantee ([[ADR-029]]/[[ADR-030]]
scope blinding to `~/.claude`'s other projects/history/settings, not
to the target project's own path); it was an incidental byproduct of
the pre-[[ADR-040]] broker design. **Rejected:** keeping the alias and
isolating `target/` under a per-session tmpfs instead — smaller diff,
but loses incremental build caching across cage sessions and leaves
the cosmetic path-leakage (and the same class of hazard for any other
env-baked or cwd-baked absolute path) unfixed.

## ADR-047 — `ctx-verify` folds in `cargo-machete`; parallel CI retired; `cargo-deny`/module-cycle stay out
**Decision:** add `cargo-machete` (unused-dependency detection) to
`ctx-verify` as a script-wrapped `Spec`
(`scripts/machete_check.sh`, `bash` + `script_parser`, after `no_allow`),
mirrored byte-identically in `template/scripts/` per [[ADR-025]]. Retire
the pre-`ctx-verify` CI stack entirely: delete `scripts/ci.sh` (root +
template) and `template/.github/` (the `ci.yml` workflow + dir). This
makes `ctx-verify` the single agent checkpoint in `template/` too,
matching how the root repo already verifies itself — root shipped no
`.github/` and its `ci.sh` was already orphaned. Extends [[ADR-011]]/
[[ADR-022]] ("`ctx-verify` is the whole checkpoint; use it, not raw
`cargo`/`ci.sh`") to close the last place where `template/` still
diverged. `cargo-machete` reads only manifests + source (no
`cargo metadata`, no network), so it runs in the same offline static-check
context as `no_allow`/`workspace_lints`; a missing binary is a loud
`FAIL`, not a skip (stated policy, same posture as `cycle_check.sh`).
Landing it shook out one real finding — a genuinely-unused `serde`
dependency in `ctx-cage` (only `serde_json` is used), removed here.
**Rejected — folding in `cargo-deny` and `cargo-modules --acyclic`**
(the original intent of this change), for two concrete blockers found on
this toolchain/environment:
(1) **`cargo-deny` cannot run offline.** It drives `cargo metadata`,
which tries to download host-foreign (Windows-only) crates
(`winapi-util`, `anstyle-wincon`) absent from the vendored Linux cache,
and the advisory-db fetch needs network — both impossible in the offline
cage ([[ADR-040]]/[[ADR-041]]). Folded in, it would make `ctx-verify`
un-passable here. It stays documented policy (SPEC "Dependency policy"),
enforceable on a networked CI, not by `ctx-verify`.
(2) **`cargo-modules 0.26.0 --acyclic` false-positives.** It flags every
type↔associated-function pair as a "circular dependency" (e.g.
`CommandOutcome` ↔ `CommandOutcome::ok`) on 5 of 6 workspace crates; the
acyclicity check runs on the *unfiltered* graph, so no
`--no-fns`/`--no-types`/`--no-owns` flag suppresses it. Folded in, it
would fail `ctx-verify` on known-clean code — the "looks-enforced-but-
misfires" class this project exists to prevent. Module-cycle detection
therefore stays deferred to dylint rule 4 (`DYLINT_RULES.md`);
`cycle_check.sh` remains the runs-cleanly stub and SPEC's "aspirational"
wording on cycle detection stands. **Residual:** `cargo-deny`'s
license/advisory/duplicate-version policy and real module-cycle detection
are unautomated at the repo root today (they were already orphaned there);
this change makes `template/` match that reality rather than adding
coverage — recorded so a future networked-CI or dylint pass reintroduces
them deliberately, not by reverting this ADR.

## ADR-048 — Interactive PTY isolation for the cage (resolves ADR-034 backlog)
**Decision:** `--interactive` cage runs now get a **dedicated host-side
PTY relay** instead of inheriting the host's controlling terminal
directly. The host allocates a `openpty` pair, drops the real terminal
into raw mode (restored via an RAII guard on every exit/panic path),
hands the slave to `bwrap` as stdin/stdout/stderr, and pumps bytes both
directions between the master and the real terminal (reusing
`proxy::pump`); `SIGWINCH` is caught via a `signalfd` and the size
copied onto the master so resizes reach the cage live. Inside the cage
the interactive command is wrapped in **`setsid --ctty --wait`** so
claude gets a new session whose controlling terminal is its stdin (the
private PTY slave). `--unshare-pid` and the rest of the isolation are
**unchanged** — full PID-namespace isolation is retained.
**Context:** `--unshare-pid` is always on, but the caged process's
controlling terminal used to live in the *host's* PID namespace. Job-
control ioctls (`tcgetpgrp`/`tcsetpgrp`, `SIGTTIN`/`SIGTTOU`) can't
resolve across that boundary and return `ENOTTY`/`EIO`; Node's readline
(the claude TUI) busy-retries instead of blocking, pinning one core at
100%. Reproduced from inside the cage (`tcgetpgrp` → "Inappropriate
ioctl for device"). Deferred since [[ADR-034]]/[[ADR-040]]; this closes
it (option 2 of the backlog: a real PTY relay, not the isolation-
weakening "drop `--unshare-pid`" shortcut).
**Rationale:** a private PTY as the child's controlling terminal keeps
`tcgetpgrp`/`tcsetpgrp` inside the cage's own namespace, so flow control
works and the spin disappears. Dropping `--unshare-pid` (the quick
mitigation) was rejected: it would expose the host process list via
`/proc` (argv leakage) and widen signal blast radius to same-uid host
processes — regressions the cage's threat model ([[ADR-026]], "capable
& lazy, not adversarial") tolerates only because it need not.
**Implementation:** new `nix` (safe `openpty`/`termios`/`signalfd`) and
`rustix` (safe `tcgetwinsize`/`tcsetwinsize`, which `nix` lacks) deps —
both already in the offline registry cache; `portable-pty` is **not**
cached and would need network, so it was rejected. `TIOCSCTTY` is
delegated to `setsid --ctty` rather than a `pre_exec` ioctl, keeping the
workspace `unsafe_code = "forbid"` intact (bwrap's own `--new-session`
only `setsid`s — no `TIOCSCTTY` — so it is neither used nor sufficient
here). The relay engages only when host stdin `is_terminal()`; piped /
CI / test invocations fall back to plain stdio inheritance unchanged.

## ADR-049 — Stub `resolv.conf`: the cage's DNS must fail slowly
**Decision:** bind a **stub `/etc/resolv.conf`** (new asset
`assets/cage-resolv.conf`, `include_str!`-embedded like
`cage-rules.md`, materialized per-run into the rundir) into every cage.
It names one unroutable server — `192.0.2.1`, TEST-NET-1 (RFC 5737).
It resolves nothing and is never reached; its only job is to keep DNS
failing *slowly*.
**Context:** caged `claude` burned **~101% of a core permanently, at an
empty prompt, with no input** — not during streaming, and it never
self-quiesced. The cage mounts almost none of `/etc` (only
`/etc/alternatives`), so there is no `resolv.conf`; the resolver falls
back to its `127.0.0.1:53` default. `--unshare-net` gives the cage a
netns with loopback **up** and nothing listening on `:53`, so every
query is refused *instantly* (ICMP port-unreachable → ECONNREFUSED) and
retried immediately — a tight loop. Measured host-vs-cage under
identical conditions: idle 2.8% host / 101.6% cage; with the stub,
2.8% / 2.7%, and under a keystroke load 77.0% / 77.7% (parity).
**Root cause:** [[ADR-029]] provisioned exactly this plumbing
(`resolv.conf`, `/etc/hosts`, `/etc/ssl`, a minimal `nsswitch.conf`)
for the Bash `cage-run.sh --claude`. The [[ADR-034]] Rust port carried
the *assets* across but **silently dropped the binds** — `nsswitch` is
still an orphan asset today (nothing in `src/` references it, and its
own header claims it is "Bound as /etc/nsswitch.conf"). Nobody noticed
because [[ADR-035]]+ made the cage unconditionally offline, so DNS is
*expected* to fail. The bug is that it did not fail *quietly*.
**Rationale:** any non-loopback address works — the fix is not "make
DNS work" (it must not) but "deny the resolver an instant refusal".
This is pinned by a test asserting the asset names a nameserver and
that none are loopback, since both regressions are silent at build time
and cost a core at runtime.
**Rejected:** binding the host's real `/etc/resolv.conf` (works, but
leaks host DNS config into a blinded offline cage and makes cage
behaviour depend on host network state); an **empty** file (measured:
still 101.6% — an empty resolv.conf *is* the `127.0.0.1` default, not
an inert one); `nameserver 127.0.0.1` (measured: 101.7%, the bug
itself); binding all of `/etc` (measured fix at 2.5%, but leaks host
config wholesale and contradicts [[ADR-026]] blinding). Reviving the
orphaned `cage-nsswitch.conf` bind was **not** bundled here: it targets
glibc NSS, whereas the spinning resolver is Bun's own (c-ares reads
`resolv.conf` directly and ignores `nsswitch.conf`), and the asset's
stated purpose — "resolving api.anthropic.com over the shared (1a)
network" — describes a network mode that no longer exists. It is dead
weight with a false header comment; disposition is a separate call.
**Method note:** the prior investigation (`cage-cpu-findings.md`, now
deleted) concluded the opposite — "claude reaches genuine idle (0%)",
residual CPU is "Claude Code's own TUI reconciler", "not fixable in
run.rs", "extend ADR-048 with a note and stop" — and was one step from
writing that into this log. It measured only *inside* the cage, where
the PID namespace structurally forbids the host comparison that settles
it, and reported a vivid mechanism (timerfd fd7, 0.78 ms one-shot,
~14 ms render callback, ~70 Hz) for an experiment with no control arm.
Its own numbers were self-contradictory (87% lifetime average vs 0%
idle). The decisive datum was free all along and in the first file
read: **`utime`/`stime` split** — 46.8% user vs **54.7% system**. Over
half the burn was kernel time, which no userspace render loop can
explain. Sampling `/proc/<tid>/syscall` said "99.9% running" and misled
both passes, because it can only catch a syscall that *blocks*; an
instantly-failing `sendto` is invisible to it. Prefer unbiased counters
over sampled ones, and measure the null (idle) case before any load.
