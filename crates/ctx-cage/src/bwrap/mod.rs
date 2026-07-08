//! `bwrap` argv builder for the safety cage.
//!
//! Pure over a resolved [`BwrapConfig`]: no I/O, no `bwrap` exec, no
//! filesystem access. The lifecycle module fills the config from CLI
//! flags + the user's environment; this module turns it into a
//! deterministic `Vec<OsString>` ready for
//! `Command::new("bwrap").args(...)`.
//!
//! Mount layout in the cage:
//! - the target project — **RW** bind of `target_root` at its own real
//!   host path (no fixed alias, ADR-046: a build compiled inside the
//!   cage must see the identical path outside it); the agent edits the
//!   real tree, and recovery is plain git from a clean committed start.
//! - `<target_root>/<secret>` — empty-file RO masks (`.env`,
//!   `.git/config`).
//! - toolchain dirs — RO binds at their identical host paths.
//! - `/cage/bin` — tmpfs with the host CTX tools (and `claude`) RO-bound.
//! - `/opt/cage/rules.md` — RO bind of the cage-rules file.
//! - `/run/ctx` — RO bind of the run dir holding the API proxy socket
//!   (the sole egress; the cage itself has no network).

mod build;
mod config;

pub use build::build_bwrap_args;
pub use config::{
    BwrapConfig, API_SOCK_NAME, CAGE_BIN, CAGE_CLAUDE_CONFIG, CAGE_CLAUDE_CRED, CAGE_LOCAL_CLAUDE,
    CAGE_RULES_PATH, CAGE_RUN_DIR,
};
