//! ctx-cage — generic sandboxed-agent harness.
//!
//! A staged refactor (ADR-034, in flight): replaces the meal-planning-
//! shaped `sandbox/*.sh` scripts with a project-parameterized Rust tool
//! subject to the same lint regime and `ctx-verify` gate as every other
//! crate in this workspace. This commit lands the foundation — the
//! [`protocol`] module the broker, in-cage forwarder, and lifecycle
//! orchestrator will all speak — plus the embedded caged-agent rules.
//! Subsequent commits land the broker, `bwrap` launcher, lifecycle,
//! summarize-on-start/-stop, and the PTY relay.
//!
//! Cage rules — the project-agnostic caged-agent doctrine, injected
//! via `--append-system-prompt-file` on every real `claude` launch —
//! are embedded with `include_str!` so the binary cannot be shipped
//! without them and cannot drift from a file on disk.

pub mod broker;
pub mod bwrap;
pub mod cli;
pub mod error;
pub mod lifecycle;
pub mod protocol;
pub mod runtime;
pub mod spawn;
pub mod summarize;

/// The project-agnostic caged-agent rules, injected as the system
/// prompt's append-context on every real `claude` launch.
pub const CAGE_RULES_MD: &str = include_str!("../assets/cage-rules.md");
