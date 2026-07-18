# Status & Active Plan

The live "where we are, what's next, and why." This file is the moving
part; it deliberately does **not** describe current architecture — that
is derived, and lives in the regenerated root rollup. **For current
architecture, read the root rollup: `ctx-context .`** (tool names, hook
wiring, freshness model, cage posture all live there). `SPEC.md` is the
sealed design, `UNIMPLEMENTED.md` the backlog, `DECISIONS.md` the
rationale log. Keep this file to what a rollup can't express: open work,
dates, what's blocked on what, and deferred items.

Last updated: 2026-07-18.

## Done and verified (the lead-by-hooks pivot — ADR-035..043)

Status only; see the rollups for how each works.

- `ctx-context`: read-only chain CLI + fail-open `--hook`; write
  path/task lifecycle deleted. Hook verified firing in real sessions
  (dedup state files are the evidence, not model self-reports — ADR-037).
- `ctx-scan`: CACT hash tree + sidecars, `.ctxignore` scope,
  `--check`/`--update`/`--stop-hook`, orphan-leaf cleanup. `--check` also
  reports missing (deleted/never-generated) artifacts. Tests cover the
  edit-one-file ⇒ one-leaf+ancestor-rollups delta.
- Serve-time freshness: `ctx-context` tags each served summary against its
  `hashes.json` sidecar — `[STALE]` (source changed) vs `[NEVER GENERATED]`
  (source exists, no summary) — so a weak model reading injected context
  can tell current from outdated. Shared hashing lives in `ctx-core`.
- Prompts: leaf + rollup rewritten to prose (leaves KEPT, ADR-039).
- `ctx-cage` rework: broker/protocol/client deleted; safety-only cage +
  `ctx-run` launcher (ADR-040..043, 046, 048, 050).
- Interactive caged `claude` reached the TUI on subscription auth.
- Docs: SPEC re-specced to current truth (ADR-051); retired designs moved
  to `docs/retired/`.
- Whole-workspace `ctx-verify` = `pass` at every step.

Pre-pivot MVP validation (access/cage thesis, billed runs, reference
project) stands as history: ADR-016..034 and git.

## Open / next

1. **First billed headless `ctx-run`** (one small task): proves relay →
   proxy → TLS passthrough with OAuth end-to-end. Free tests cover
   everything up to the first real API request.
2. **Seed this repo's summary tree** with the current prompts:
   `ctx-scan . --update --approve` (~110 leaves + 34 rollups, billed).
   Until then `.context/` below the root is STALE (pre-pivot content,
   mostly no sidecars): chains still serve, but describe the old
   architecture and are absent for the new crates. Serve-time markers now
   flag this where sidecars exist (`[STALE]`/`[NEVER GENERATED]`), but a
   full reseed is still the fix. This remains a live risk to any agent
   trusting injected context in this repo below the root.
3. **Phase 5 — e2e smoke fixture**: throwaway workspace, buggy file +
   failing test, generated tree, `--stub` (deterministic fake agent, no
   billing) and billed modes; asserts hook injection, native Edit,
   `ctx-verify` pass, post-session rollup regen, no egress beyond the
   proxy, no writes outside the workspace.

## Deferred / residuals (recorded in ADRs)

- seccomp filter for the cage (namespaces + `no_new_privs` + offline
  stand today) — ADR-040.
- OAuth refresh mid-session would miss the single-host proxy — ADR-041.
- Orphaned `.context` *directory* subtrees are not auto-pruned — ADR-038.
- Dedicated-PTY isolation for `--interactive` (ADR-034 backlog).
- Production multi-agent broker + Layer 3: `UNIMPLEMENTED.md`, unchanged.

## How to verify anything

`target/debug/ctx-verify` (optionally a crate name to scope) — `pass` =
done. Containment: `ctx-cage <dir> --self-test stub`. Chain:
`ctx-context <path>`. Freshness: `ctx-scan <dir> --check`.
