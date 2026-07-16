//! Configuration types + cage-internal path constants.

use std::ffi::OsString;
use std::path::PathBuf;

/// Cage-internal bin directory the host tools are bound into.
pub const CAGE_BIN: &str = "/cage/bin";

/// Cage-internal RO bind of the always-injected rules markdown.
pub const CAGE_RULES_PATH: &str = "/opt/cage/rules.md";

/// Cage-internal RO bind of the stub `resolv.conf`.
pub const CAGE_RESOLV_PATH: &str = "/etc/resolv.conf";

/// Cage-internal RO bind of the host's run directory (holds the API
/// proxy socket).
pub const CAGE_RUN_DIR: &str = "/run/ctx";

/// Basename of the API proxy socket inside [`CAGE_RUN_DIR`].
pub const API_SOCK_NAME: &str = "api.sock";

/// Cage-internal RW bind path of the synthesized `~/.claude.json`.
pub const CAGE_CLAUDE_CONFIG: &str = "/tmp/.claude.json";

/// Cage-internal RO bind path of `~/.claude/.credentials.json` (the
/// cage `HOME` is `/tmp`, so claude finds it here).
pub const CAGE_CLAUDE_CRED: &str = "/tmp/.claude/.credentials.json";

/// Cage-internal secondary bind of the `claude` binary at the path the
/// installer health-check expects under the cage `HOME` (silences the
/// spurious "claude command missing or broken" startup warning).
pub const CAGE_LOCAL_CLAUDE: &str = "/tmp/.local/bin/claude";

/// Resolved configuration for one cage launch.
#[derive(Debug, Clone, Default)]
pub struct BwrapConfig {
    /// Absolute host path of the target project root, bound RW at this
    /// same path inside the cage (no fixed alias — a compiled build
    /// artifact must see the identical path in and out of the cage).
    pub target_root: PathBuf,
    /// Repo-relative paths masked with an RO bind of [`Self::mask_file`]
    /// so their contents are hidden inside the cage (e.g. `.env`,
    /// `.git/config`).
    pub secret_masks: Vec<String>,
    /// Host path of an **empty regular file** bound over each secret
    /// path. Not `/dev/null`: bwrap bind mounts carry `nodev`, so a
    /// masked device node fails every open with EACCES and breaks its
    /// readers (git died on the masked `.git/config`).
    pub mask_file: PathBuf,
    /// Host directories bound read-only at their identical host paths
    /// (the Rust toolchain: `~/.cargo`, `~/.rustup`) so builds work
    /// offline without making anything else of `$HOME` visible.
    pub toolchain: Vec<PathBuf>,
    /// `(host binary, cage path)` pairs bound read-only (the CTX tools
    /// under [`CAGE_BIN`]; `claude` in billed modes).
    pub tool_binds: Vec<(PathBuf, String)>,
    /// `(host file, cage path)` pairs bound read-write (only the
    /// synthesized `~/.claude.json`, which claude rewrites at runtime).
    pub rw_binds: Vec<(PathBuf, String)>,
    /// Absolute host path of the run directory (bound RO at
    /// [`CAGE_RUN_DIR`]; contains the API proxy socket).
    pub rundir: PathBuf,
    /// Absolute host path of a file containing the cage-rules markdown.
    pub cage_rules_path: PathBuf,
    /// Absolute host path of a file containing the stub `resolv.conf`,
    /// bound RO at [`CAGE_RESOLV_PATH`]. The cage is offline, so this
    /// resolves nothing; it exists because DNS must fail *slowly*.
    /// Absent (the cage mounts almost no `/etc`) or empty, the resolver
    /// defaults to `127.0.0.1:53`, which the cage's netns refuses
    /// instantly — and the retry loop burns a full core for the whole
    /// session (ADR-049).
    pub resolv_conf: PathBuf,
    /// `--setenv` pairs — with `--clearenv`, this is the entire cage
    /// environment.
    pub env: Vec<(String, String)>,
    /// Pass `--new-session` to bwrap. `true` is the safe default for
    /// non-interactive runs (blocks TIOCSTI-style injection); `false`
    /// for interactive use so Claude Code can render its TUI directly.
    pub new_session: bool,
    /// argv (after `--`) to exec inside the cage.
    pub cage_cmd: Vec<OsString>,
}
