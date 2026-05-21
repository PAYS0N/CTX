//! Configuration types + cage-internal path constants.

use std::ffi::OsString;
use std::path::PathBuf;

/// Cage-internal mount point of the target project (RO).
pub const WORK_DIR: &str = "/work";

/// Cage-internal bin directory the broker tools are bound into.
pub const CAGE_BIN: &str = "/cage/bin";

/// Cage-internal RO bind of the always-injected rules markdown.
pub const CAGE_RULES_PATH: &str = "/opt/cage/rules.md";

/// Cage-internal RO bind of the host's socket directory.
pub const CAGE_SOCK_DIR: &str = "/run/ctx";

/// Cage-internal RW bind path of the synthesized `~/.claude.json`.
pub const CAGE_CLAUDE_CONFIG: &str = "/tmp/.claude.json";

/// Cage-internal RO bind path of `~/.claude/.credentials.json`.
pub const CAGE_CLAUDE_CRED: &str = "/tmp/.claude/.credentials.json";

/// Output of `discover_crate_dirs`: repo-relative `src` and `tests`
/// directories for each Cargo crate under `<target_root>/crates`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveredCrates {
    /// Repo-relative paths like `crates/<name>/src`.
    pub srcs: Vec<PathBuf>,
    /// Repo-relative paths like `crates/<name>/tests` (only when the
    /// directory exists).
    pub tests: Vec<PathBuf>,
}

/// Resolved configuration for one cage launch.
#[derive(Debug, Clone)]
pub struct BwrapConfig {
    /// Absolute host path of the target project root.
    pub target_root: PathBuf,
    /// Crate src/tests dirs to overlay with empty tmpfs (the jail).
    pub crates: DiscoveredCrates,
    /// Absolute host path of the dir containing the broker socket.
    pub sockdir: PathBuf,
    /// Basename of the socket file inside `sockdir`.
    pub sockname: String,
    /// Task identifier exported as `TASK=<id>` to the cage.
    pub task_id: String,
    /// Absolute host path of the `ctx-cage-client` binary; bound into
    /// the cage as both `ctx-access` and `ctx-verify`.
    pub client_binary: PathBuf,
    /// Absolute host path of a file containing the cage-rules markdown.
    pub cage_rules_path: PathBuf,
    /// `TERM=` value set inside the cage.
    pub term: String,
    /// `Some` ⇒ `--claude` mode (binds `claude` + DNS/TLS + credential).
    pub claude: Option<ClaudeBinds>,
    /// Don't `--unshare-net`. Implied by `--claude`.
    pub allow_net: bool,
    /// Pass `--new-session` to bwrap. `true` is the safe default for
    /// non-interactive runs (blocks TIOCSTI-style injection); set to
    /// `false` for interactive use so the cage inherits the parent's
    /// controlling tty and Claude Code can render its TUI directly.
    pub new_session: bool,
    /// argv (after `--`) to exec inside the cage.
    pub cage_cmd: Vec<OsString>,
}

/// Host paths that `--claude` mode binds in.
#[derive(Debug, Clone)]
pub struct ClaudeBinds {
    /// Resolved (`readlink -f`) host path of the `claude` binary.
    pub claude_binary: PathBuf,
    /// Host path of `~/.claude/.credentials.json`.
    pub credentials: PathBuf,
    /// Resolved host path of `/etc/resolv.conf`.
    pub resolv_conf: PathBuf,
    /// Host path of the deterministic minimal `nsswitch.conf`.
    pub nsswitch_conf: PathBuf,
    /// Host path of the synthesized `~/.claude.json` (rw, ephemeral).
    pub claude_config_json: PathBuf,
}
