# Sandbox Design (deployment layer)

Status: design, not implemented at MVP. Referenced by `docs/SPEC.md`
Layer 2. Without the sandbox deployed, Layer 2 is **advisory**, not
enforced: an agent can read source directly and bypass `ctx-access`. This
document is the deployment-side design that makes Layer 2 load-bearing.

## Threat model

The "agent" is an autonomous process with shell access. It is not
adversarial in the malware sense, but it is *capable* and *lazy*: if it can
`cat src/foo.rs` it will, skipping the context chain the system exists to
enforce. The control must make direct source reads **impossible**, not
merely discouraged, while leaving the `ctx-access` path open.

Non-threats at MVP: kernel exploits, container escape, side channels. The
agent is trusted not to attack the host; it is not trusted to follow a
protocol voluntarily.

## Why the CLI cannot do this itself

`ctx-access` is a binary the agent invokes. Anything the agent can invoke,
the agent can also *not* invoke. If source is readable from the agent's
filesystem, no in-process check in `ctx-access` matters — the agent reads
around it. Enforcement must live somewhere the agent cannot reach.

## Rejected approaches

- **Restricted shell + scrubbed `PATH`** (rbash, remove
  `cat`/`less`/`head`, no interpreters): defeated by any interpreter
  (`python -c`, `perl`, even `mapfile`/`read` builtins). Brittle.
- **Syscall interception** (seccomp-bpf or `LD_PRELOAD` on
  `openat`/`open`): bypassed by static binaries and raw syscalls; complex
  to get right. Not a primary control.

Both may be used as defense-in-depth but neither is sufficient alone.

## Chosen architecture: broker daemon + isolated agent

Two processes, two trust domains:

```
┌─ agent domain ───────────────┐        ┌─ broker domain ──────────────┐
│  agent shell                 │        │  ctx-broker (uid: ctx)       │
│  └─ ctx-access (thin client) │ ─UDS─▶ │  └─ enforcement + source FS  │
│  source roots: NOT PRESENT   │        │  owns .context/.cache/*      │
└──────────────────────────────┘        └──────────────────────────────┘
```

1. **Source roots are absent from the agent's filesystem view.** Two
   equivalent mechanisms:
   - *Container*: run the agent in a container; do not bind-mount the
     source tree into it. The broker runs on the host (or a sidecar) with
     the tree mounted.
   - *Mount namespace* (`bwrap`/`unshare -m`): launch the agent shell with
     the source roots replaced by an empty `tmpfs`, or simply never
     bind-mounted. The broker runs outside that namespace.
2. **`ctx-broker` runs as a dedicated `ctx` user/identity** that owns the
   source tree (mode `0700`) and the `.context/.cache/` and
   `.context/.reports/` directories. It is the only process that can read
   source or mutate the per-task cache.
3. **`ctx-access` becomes a thin client.** It parses argv, opens the
   unix-domain socket to `ctx-broker`, forwards the request, streams the
   response. It contains *no* enforcement logic and *no* filesystem access
   to source.
4. **All enforcement lives in the broker**: chain computation, served-node
   tracking, write-requires-prior-read, stale banners, task lifecycle.
   A compromised or bypassed client gains nothing — there is no source on
   the agent's filesystem and the cache is not writable by the agent uid.

## Implication for the `ctx-access` implementation (Phase 1)

Build the MVP `ctx-access` as a single binary, but with a hard internal
seam:

- `cli` module: argv parsing, stdout/stderr, exit codes. Knows nothing
  about the protocol.
- `enforcement` module: pure logic over an injected filesystem + clock
  abstraction. Knows nothing about argv, sockets, or process boundaries.
- `transport` module (MVP: direct in-process call; future: UDS to broker).

Splitting into client + `ctx-broker` later is then a transport swap, not a
rewrite. This seam is mandatory, not optional, because the sandbox is the
only thing that makes Layer 2 real.

## Concurrency

The broker is the natural home for the concurrency foundations in
`docs/SPEC.md`: it owns the cache files, so cache-file locking and
task-affinity scheduling become broker-internal concerns. MVP remains
branch/worktree-per-agent (no shared-filesystem concurrency), so no locking
is required yet; the broker model just leaves room for it without redesign.

## MVP posture

At MVP the sandbox is **not deployed**. The spec states plainly that Layer 2
is advisory until this design is in place. The Phase 1 internal seam is the
only part of this document that is built at MVP; the broker, the namespace
plumbing, and the `ctx` identity are deployment work tracked in
`docs/UNIMPLEMENTED.md`.
