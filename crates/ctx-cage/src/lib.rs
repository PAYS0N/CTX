//! ctx-cage — a *safety* boundary for autonomous agent runs.
//!
//! Post-pivot role (supersedes the ADR-034 enforcement cage): the cage
//! no longer masks source or brokers tools — the context chain is led
//! by hooks (`ctx-context`), and the agent uses its native Read/Edit
//! inside the cage. What the cage contains is *blast radius*:
//!
//! - **Filesystem:** only the target workspace is writable, bound
//!   read-write at its own real host path (no fixed alias — ADR-046);
//!   the toolchain is bound read-only; secrets are masked even inside
//!   the workspace (`.env`, `.git/config`); nothing from `$HOME` is
//!   mounted. Recovery is plain git — sessions start from a clean
//!   committed tree.
//! - **Host:** fresh user/pid/ipc/uts namespaces, `--die-with-parent`,
//!   cleared environment; bwrap sets `no_new_privs` unconditionally.
//!   (A seccomp filter is a documented residual, not yet wired.)
//! - **Network:** fully offline (`--unshare-net`). The sole egress is
//!   the host-side [`proxy`] that injects the real API key and
//!   originates TLS to the API, reached over a bind-mounted UNIX
//!   socket; the raw key never enters the cage.
//!
//! Cage rules — the agent doctrine injected via
//! `--append-system-prompt-file` on every real `claude` launch — are
//! embedded with `include_str!` so the binary cannot ship without them.

pub mod bwrap;
pub mod cli;
pub mod error;
pub mod lifecycle;
pub mod proxy;
pub mod runtime;

/// The project-agnostic caged-agent rules, injected as the system
/// prompt's append-context on every real `claude` launch.
pub const CAGE_RULES_MD: &str = include_str!("../assets/cage-rules.md");

/// The stub `resolv.conf` bound into every cage. The cage is offline,
/// so this never resolves anything — it only keeps DNS failing
/// *slowly*; see the asset's own comment and ADR-049.
pub const CAGE_RESOLV_CONF: &str = include_str!("../assets/cage-resolv.conf");
