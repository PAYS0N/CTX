# The cage (MVP sandbox + agent run)

Makes Layer 2 **load-bearing**: the agent's filesystem has *no source*
and *no toolchain*, so the only path to source **and** verification is
the brokered tools. MVP realization of the client/broker transport seam
in `../docs/SANDBOX.md` (ADR-026/028).

## Topology

```
┌─ cage (bwrap mount[+net] ns) ────────┐      ┌─ host ────────────────────┐
│ /work = meal-planning, RO            │      │ broker.sh (socat LISTEN)  │
│   crates/mealplan/src   → tmpfs ∅    │      │  → broker-handler.sh      │
│   crates/mealplan/tests → tmpfs ∅    │      │    allowlist:             │
│   target/               → tmpfs ∅    │      │      ctx-access  (source) │
│ /cage/bin/ctx-access ─┐              │      │      ctx-verify  (verify) │
│ /cage/bin/ctx-verify ─┴─ forwarder ──┼─UDS─▶│    in the real tree       │
│ agent: stub | claude                 │      │ (model: 1a, direct egress)│
└──────────────────────────────────────┘      └───────────────────────────┘
```

Every capability that crosses the boundary is a host-side broker call:
read source, write source, **verify** (`ctx-verify` compiles/tests host
-side against the tree the agent's writes landed in — the cage needs no
cargo/rustc/source). Model calls use **egress 1a**: the real `claude`
in the cage talks to the API directly (`--net`, key in cage env);
accepted residual under the *capable-but-lazy, not adversarial* threat
model (`../docs/SANDBOX.md`).

## Two harnesses

| harness | what it proves | spend |
|---|---|---|
| `cage-demo.sh` | Cage C/D: source unreachable except via the tool; enforcement intact through the transport; tree unmutated | none |
| `agent-demo.sh` | the full agent loop: init-task → caged read/write/verify → shutdown → host acceptance | none by default |

`agent-demo.sh [--interactive]`:
- **default** — agent = `stub-claude.sh`, a no-spend stand-in walking
  verify→read→write→verify; no net, no key; probe write reverted, tree
  asserted clean.
- **`CTX_CAGE_ALLOW_SPEND=1`** (+ `ANTHROPIC_API_KEY`) — agent = real
  `claude` doing the task; `--net`+key; changes kept; `end-task`
  (audit→summarize) runs. Two spend boundaries, both gated.
- **`--interactive`** — the cage runs on its own pty, relayed to your
  terminal by `pty-relay.py`. The cage sees only that pty, so a
  TIOCSTI-style injection lands in the host-owned relay, never your real
  terminal (`--new-session` kept; sound even where
  `dev.tty.legacy_tiocsti` ≠ 0). Interactive is for *observing*; the
  validity-bearing run is **headless, unassisted** `claude -p`.

## Files

| file | side | role |
|---|---|---|
| `agent-demo.sh` | host | the agent run (stub default; `claude` on go) |
| `cage-demo.sh` | host | Cage C/D proof (reachability + adversary + integrity) |
| `cage-run.sh` | host | bwrap launcher; `[--interactive] [--net] [--pass-key]` |
| `broker.sh` / `broker-handler.sh` | host | UNIX-socket transport, allowlist `{ctx-access, ctx-verify}` |
| `pty-relay.py` | host | dedicated-pty pump for `--interactive` |
| `tool-client.sh` | cage | forwarder, bound as both `ctx-access` and `ctx-verify` |
| `stub-claude.sh` | cage | no-spend agent stand-in (loop wiring) |
| `stub-agent.sh` / `cage-adversary.sh` | cage | Cage D reachability / enforcement probes |

## Run

    ./cage-demo.sh        # the cage holds  → CAGE D PASS
    ./agent-demo.sh       # the loop wires  → AGENT RUN PASS (no spend)
    ./agent-demo.sh --interactive
    CTX_CAGE_ALLOW_SPEND=1 ANTHROPIC_API_KEY=… ./agent-demo.sh   # real, billed

## Prerequisites / residuals

- The reference project must be a git repo (`ctx-access` gate/manifest +
  the static checks single-source on git: ADR-023/025). meal-planning's
  `scripts/` are synced from `../template/scripts/` so the brokered
  `ctx-verify` is deterministic there (ADR-025).
- Same uid both sides; the production `ctx`-uid / cache-owning broker
  (`../docs/SANDBOX.md`, `../docs/UNIMPLEMENTED.md`) stays deferred.
