---
intent_version: 1
---
# Project Intent — CTX

An opinionated agentic coding system. Its job is to make bad code
**uncompilable or unrepresentable, not merely discouraged**, and to force
agents to read project context **top-down before acting on any file**.

## What it is for

Three standing goals, stated so they survive a change of mechanism. *How*
each is currently realized — the tools, their flags, the hook wiring — is
described in the regenerated root rollup (`ctx-context .`), specified in
`docs/SPEC.md`, and tracked in `docs/STATUS.md`; it is deliberately not
restated here, because a goal that names its implementation drifts the
moment the implementation changes.

- **Layer 1 — enforcement:** make whole classes of defect
  *unrepresentable*, not merely discouraged, by building every crate —
  CTX's own included (dogfood) — under a compiler-and-lint regime with no
  suppression path.
- **Layer 2 — context before source:** force an agent to read a
  directory's accumulated context, top-down, before it acts on any file
  under that directory; and keep that context regenerated from the code it
  describes, so it cannot silently drift from the source.
- **Layer 3 — architecture audit:** detect divergence between this stated
  intent and the actual structure of the tree. Deferred; the hooks it
  needs (intent files, divergence reports) exist.

## Who consumes the public surface

Autonomous agents. Human ergonomics is an explicit non-goal.

## Non-goals

- Not a linter plugin or formatter; it composes existing tools.
- No lint suppression/appeal mechanism at MVP.
- Not a substitute for review: the audit layer reports divergence, it
  does not adjudicate it.

## Architectural invariants (must hold)

- No `unsafe`; no `#[allow]` anywhere; typed errors only (no
  `Box<dyn Error>` in libraries); output through injected writers.
- Every tool is `cli` over a pure core over injected boundaries
  (`Env`/`Fs`/`Agent`/`Runner`/clock); the core has no argv, process, or
  network dependency. This seam keeps the transport swappable without
  touching the core.
- Prompts are decoupled files, never embedded in code; agents speak a
  `{system,user}`→stdout contract.
- Secrets (`.env`) are never read by a model nor committed.
- Serving context fails open; a gate over code fails closed.

## Rationale & decisions

This document states *what* and *must*. For *why* a choice was made and
what was rejected, see `docs/DECISIONS.md` (ADR log). Current state and
the active plan are in `docs/STATUS.md`; current mechanism is in
`docs/SPEC.md` and the regenerated rollups. Do not restate ADR content or
current mechanism here; point to them.
