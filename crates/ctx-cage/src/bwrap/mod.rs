//! `bwrap` argv builder + crate auto-discovery.
//!
//! Pure over a resolved [`BwrapConfig`]: no I/O, no `bwrap` exec, no
//! filesystem access (except [`discover_crate_dirs`]). The lifecycle
//! module fills the config from CLI flags + discovery + the user's
//! environment; this module turns it into a deterministic
//! `Vec<OsString>` ready for `Command::new("bwrap").args(...)`.
//!
//! Mount layout in the cage:
//! - `/work` — RO bind of `target_root`.
//! - `/work/<crate>/{src,tests}` — empty `tmpfs` per discovered crate.
//! - `/work/target` — empty `tmpfs` (no stray build artefacts).
//! - `/opt/cage/rules.md` — RO bind of the cage-rules file.
//! - `/cage/bin` — `tmpfs` with broker tools RO-bound twice (as
//!   `ctx-access` and `ctx-verify`).
//! - `/run/ctx` — RO bind of the host's socket directory.

mod build;
mod config;
mod discover;

pub use build::build_bwrap_args;
pub use config::{
    BwrapConfig, ClaudeBinds, DiscoveredCrates, CAGE_BIN, CAGE_CLAUDE_CONFIG, CAGE_CLAUDE_CRED,
    CAGE_RULES_PATH, CAGE_SOCK_DIR, WORK_DIR,
};
pub use discover::discover_crate_dirs;
