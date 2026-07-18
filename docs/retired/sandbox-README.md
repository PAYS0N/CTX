# The cage (now a Rust crate)

The sandbox lives in **`crates/ctx-cage`** as of the Rust port
(ADR-034). This directory is historical; the prototype Bash transport
(`agent-demo.sh`, `cage-run.sh`, `broker.sh`, `broker-handler.sh`,
`tool-client.sh`, `pty-relay.py`, `cage-demo.sh`, plus the in-cage
probes `stub-agent.sh`, `cage-adversary.sh`, `claude-preflight.sh`,
`stub-claude.sh`) has been retired. The Rust binary does what the
Bash did, under CTX's own lint regime, with a real framed protocol
replacing the base64+sentinel transport.

## Quick run

    # No-spend self-test (proves the orchestration end-to-end):
    target/debug/ctx-cage <target-project-root> --self-test stub

    # Billed headless task (subscription credential auto-bound):
    CTX_CAGE_ALLOW_SPEND=1 target/debug/ctx-cage <target> --task "<brief>"
    CTX_CAGE_ALLOW_SPEND=1 target/debug/ctx-cage <target> --task-file <path>

    # Billed interactive session (default, no task):
    CTX_CAGE_ALLOW_SPEND=1 target/debug/ctx-cage <target>

The target is required and has **no default**. Crates are auto-
discovered under `<target>/crates/*/{src,tests}` — tmpfs overlays
follow whatever the project's Cargo layout actually is.

## Where everything lives now

- Asset (always-injected caged-agent doctrine): `crates/ctx-cage/assets/cage-rules.md` (`include_str!`'d into the binary).
- Asset (deterministic minimal nsswitch for `--claude`): `crates/ctx-cage/assets/cage-nsswitch.conf`.
- Wire protocol: `crates/ctx-cage/src/protocol.rs`.
- Broker (`UnixListener` + `Spawner`): `crates/ctx-cage/src/{broker,spawn}.rs`.
- In-cage forwarder binary: `crates/ctx-cage/src/bin/ctx_cage_client.rs` (bound at `/cage/bin/{ctx-access,ctx-verify}`).
- `bwrap` argv builder + auto-discovery: `crates/ctx-cage/src/bwrap/`.
- CLI + spend gate: `crates/ctx-cage/src/cli.rs`.
- Lifecycle (prepare → serve → teardown): `crates/ctx-cage/src/lifecycle/`.
- `--claude` host runtime + synthesized config: `crates/ctx-cage/src/runtime.rs`.
- Auto-summarize hooks (pre/post under spend gate): `crates/ctx-cage/src/summarize.rs`.
- Integration tests: `crates/ctx-cage/tests/`.

## Deferred

- Dedicated-PTY isolation for `--interactive` (currently inherits the
  parent's tty and drops `--new-session`; sound on kernels with
  `dev.tty.legacy_tiocsti=0`, fine on this host). A `portable-pty`
  hardening pass is tracked in STATUS.
- Production `ctx`-uid / cache-owning broker (SANDBOX.md).
