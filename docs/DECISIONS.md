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
