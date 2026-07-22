intent_version: 3
# Project Intent — CTX

An opinionated agentic coding system. Its job is to make bad code
**uncompilable or unrepresentable, not merely discouraged**, and to force
agents to read project context **top-down before acting on any file**.

## What it is for

- **Layer 1 — enforcement:** make whole classes of defect
  *unrepresentable*, not merely discouraged, by building every crate —
  CTX's own included (dogfood) — under a compiler-and-lint regime with no
  suppression path.
- **Layer 2 — context before source:** force an agent to read a
  directory's accumulated context, top-down, before it acts on any file
  under that directory; and keep that context regenerated from the code it
  describes, so it cannot silently drift from the source.

## Non-goals

- No lint suppression/appeal mechanism.

## Architectural invariants

- Every tool is `cli` over a pure core over injected boundaries
  (`Env`/`Fs`/`Agent`/`Runner`/clock); the core has no argv, process, or
  network dependency. This seam keeps the transport swappable without
  touching the core.
- Prompts are decoupled files; agents speak a
  `{system,user}`→stdout contract.
- Serving context fails open; a gate over code fails closed.
