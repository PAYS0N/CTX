# Status & Active Plan

The live "where we are, what's next, and why." `SPEC.md` is the frozen
design (revision 4 = the lead-by-hooks re-spec); `UNIMPLEMENTED.md` is
the backlog; `DECISIONS.md` is the rationale log; this file is the
moving part. Update it whenever the active focus changes.

Last updated: 2026-07-02.

## Current shape (post-pivot)

Agents use native Read/Edit. `ctx-context <path>` serves the
rollup+intent chain (a directory or `.` serves directory summaries);
the committed `.claude/settings.json` injects it on every Read/Grep/
Glob via the fail-open `PostToolUse` hook, deduplicated per session.
Freshness is content-hashed (`.context/<dir>/hashes.json`); the Stop
hook reports staleness only; regeneration is post-session
(`ctx-scan <dir> --update`, `--approve` past 50 targets) — one owner,
ADR-043. The cage is safety-only: RW workspace, masked secrets
(empty-file masks, ADR-042), offline with the host passthrough proxy
as sole egress, subscription auth (ADR-041). `ctx-run <dir> "<task>"`
is the billed launcher. `ctx-verify` is unchanged and remains THE
checkpoint.

## Done and verified (this pivot — ADR-035..043)

- **`ctx-context`** (was `ctx-access`): read-only chain CLI + `--hook`
  mode (event-`cwd` rooted, session-deduped, fail-open with loud
  markers). Write path/task lifecycle deleted. Hermetic tests; hook
  verified firing in real GUI/CLI sessions (dedup state files are the
  evidence — model self-reports are not, ADR-037).
- **`ctx-scan`**: CACT hash tree + sidecars, `.ctxignore` scope,
  `--check` (free) / `--update` (selective, gated) / `--stop-hook`
  (report-only), orphan-leaf cleanup. Tests cover the
  edit-one-file ⇒ one-leaf+ancestor-rollups delta.
- **Prompts**: leaf + rollup rewritten cdoc-style (behavior-first,
  `edit_notes`, invariants demoted; budgets unchanged). Leaves KEPT
  (ADR-039).
- **`ctx-cage` rework**: broker/protocol/client deleted; RW workspace,
  toolchain RO binds, always-offline + passthrough proxy (pure header
  rewrite, socat-TLS upstream seam, socketpair-tested); `ctx-run`
  launcher (dirty-tree refusal, optional 0600 `~/.config/ctx/env` for
  the summarizer only, post-run refresh that warns but never fails the
  run). `--self-test stub` probes: workspace writable, masks readable-
  as-empty, in-cage `git status` works, no egress — green.
- **Interactive caged claude reached the TUI** on subscription auth
  with no onboarding/key prompt (installer warning silenced by the
  `/tmp/.local/bin/claude` bind).
- **Docs**: SPEC revision 4, ADR-035..043, CLAUDE.md rewritten
  cdoc-lean, cage-rules precedence clause, SANDBOX.md retired-banner.
- Whole workspace `ctx-verify` = `{"status":"pass"}` at every step.

Pre-pivot MVP validation (access/cage thesis, billed runs, reference
project) stands as history: ADR-016..034 and git.

## Open / next

1. **First billed headless `ctx-run`** (one small task): proves relay →
   proxy → TLS passthrough with OAuth end-to-end. Free tests cover
   everything up to the first real API request.
2. **Seed this repo's summary tree** with the new prompts:
   `ctx-scan . --update --approve` (~110 leaves + 34 rollups, billed).
   Until then `.context/` is STALE (pre-pivot content, no sidecars):
   chains still serve, but describe the old architecture below the
   root level and are absent for the new crates.
3. **Phase 5 — e2e smoke fixture**: throwaway workspace, buggy file +
   failing test, generated tree, `--stub` (deterministic fake agent,
   no billing) and billed modes; asserts hook injection, native Edit,
   `ctx-verify` pass, post-session rollup regen, no egress beyond the
   proxy, no writes outside the workspace.

## Deferred / residuals (recorded in ADRs)

- seccomp filter for the cage (namespaces + `no_new_privs` + offline
  stand today) — ADR-040.
- OAuth refresh mid-session would miss the single-host proxy —
  ADR-041.
- Orphaned `.context` *directory* subtrees are not auto-pruned —
  ADR-038.
- Dedicated-PTY isolation for `--interactive` (ADR-034 backlog).
- Production multi-agent broker + Layer 3: `UNIMPLEMENTED.md`,
  unchanged.

## How to verify anything

`target/debug/ctx-verify` (optionally a crate name to scope). It
formats+builds+lints+**tests** in one call; `{"status":"pass"}` = done.
Containment: `target/debug/ctx-cage <dir> --self-test stub`. Chain:
`target/debug/ctx-context <path>`. Freshness: `ctx-scan <dir> --check`.
