---
intent_version: 1
---
# Project Intent — CTX

An opinionated agentic coding system. Its job is to make bad code
**uncompilable or unrepresentable, not merely discouraged**, and to force
agents to read project context **top-down before acting on any file**.

## What it is for

- **Layer 1 — enforcement:** a Rust workspace lint regime (compiler +
  clippy all/pedantic/nursery + a restriction subset, length/complexity
  tiers, no `#[allow]`) under which all member crates and the reference
  project are built. CTX builds under its own regime (dogfood).
- **Layer 2 — context tree:** `ctx-access` is the sanctioned path to
  source (chain read root→target, write-needs-read, task lifecycle);
  `ctx-summarize` regenerates the `.ctx`/`rollup.ctx` tree leaf-up via a
  decoupled agent; `ctx-verify` is the token-frugal verification broker.
- **Layer 3 — architecture audit:** deferred; hooks (intent files,
  divergence reports) exist.

## Who consumes the public surface

Autonomous agents, via the three `crates/` binaries and the
file-contract agents in `agents/`. Human ergonomics is an explicit
non-goal.

## Non-goals

- Not a linter plugin or formatter; it composes existing tools.
- No lint suppression/appeal mechanism at MVP.
- Layer 2 is **advisory until the sandbox is deployed** — without it an
  agent can bypass `ctx-access`. This is stated, not hidden.

## Architectural invariants (must hold)

- No `unsafe`; no `#[allow]` anywhere; typed errors only (no
  `Box<dyn Error>` in libraries); output through injected writers.
- Every tool is `cli` over a pure core over injected boundaries
  (`Env`/`Fs`/`Agent`/`Runner`/clock); the core has no argv, process, or
  network dependency. This seam is what makes the broker/sandbox split a
  transport swap.
- Prompts are decoupled files, never embedded in code; agents speak a
  `{system,user}`→stdout contract.
- Secrets (`.env`) are never read by a model nor committed.

## Rationale & decisions

This document states *what* and *must*. For *why* a choice was made and
what was rejected, see `docs/DECISIONS.md` (ADR log). Current state and
the active plan are in `docs/STATUS.md`. The frozen design is
`docs/SPEC.md`. Do not restate ADR content here; point to it.
