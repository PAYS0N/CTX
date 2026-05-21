//! Shared test helpers. Lives under `tests/common/` so cargo treats
//! it as a module (not a separate integration-test target) — each
//! `tests/*.rs` that wants it does `mod common;`. Include it only
//! when something here is actually used: an unused public item would
//! trip `dead_code = "deny"` (workspace policy), and `#[allow]` is
//! banned (no-allow check).
//!
//! Free-fn helpers cannot use `unwrap`/`expect`/`panic` (only `#[test]`
//! bodies are exempt). Helpers therefore return `bool`/`Result` and
//! the test body itself does the `assert!`.

use std::ffi::OsString;
use std::path::PathBuf;

use ctx_cage::bwrap::{BwrapConfig, ClaudeBinds, DiscoveredCrates};

/// A `BwrapConfig` good enough for argv-shape tests. Paths need not
/// exist on disk — the builder is pure.
pub fn sample_config() -> BwrapConfig {
    BwrapConfig {
        target_root: PathBuf::from("/abs/proj"),
        crates: DiscoveredCrates {
            srcs: vec![PathBuf::from("crates/a/src"), PathBuf::from("crates/b/src")],
            tests: vec![PathBuf::from("crates/a/tests")],
        },
        sockdir: PathBuf::from("/tmp/ctxcage-sock"),
        sockname: "ctx.sock".to_owned(),
        task_id: "t1".to_owned(),
        client_binary: PathBuf::from("/abs/target/debug/ctx-cage-client"),
        cage_rules_path: PathBuf::from("/abs/cage-rules.md"),
        term: "xterm-256color".to_owned(),
        claude: None,
        allow_net: false,
        new_session: true,
        cage_cmd: vec![OsString::from("/cage/bin/ctx-verify")],
    }
}

/// A `ClaudeBinds` good enough for `--claude` argv tests.
pub fn sample_claude_binds() -> ClaudeBinds {
    ClaudeBinds {
        claude_binary: PathBuf::from("/home/u/.local/share/claude/versions/9.9.9"),
        credentials: PathBuf::from("/home/u/.claude/.credentials.json"),
        resolv_conf: PathBuf::from("/etc/resolv.conf"),
        nsswitch_conf: PathBuf::from("/host/cage-nsswitch.conf"),
        claude_config_json: PathBuf::from("/host/tmp/claude.json"),
    }
}

/// Does `argv` contain the three-token sequence `<flag> <a> <b>`?
pub fn has_triplet(argv: &[OsString], flag: &str, a: &str, b: &str) -> bool {
    argv.windows(3).any(|w| {
        w.first().map(OsString::as_os_str) == Some(flag.as_ref())
            && w.get(1).map(OsString::as_os_str) == Some(a.as_ref())
            && w.get(2).map(OsString::as_os_str) == Some(b.as_ref())
    })
}

/// Does `argv` contain the two-token sequence `<flag> <a>`?
pub fn has_pair(argv: &[OsString], flag: &str, a: &str) -> bool {
    argv.windows(2).any(|w| {
        w.first().map(OsString::as_os_str) == Some(flag.as_ref())
            && w.get(1).map(OsString::as_os_str) == Some(a.as_ref())
    })
}

/// Convenience: `--ro-bind host cage` present?
pub fn has_ro_bind(argv: &[OsString], host: &str, cage: &str) -> bool {
    has_triplet(argv, "--ro-bind", host, cage)
}

/// Convenience: `--setenv key val` present?
pub fn has_setenv(argv: &[OsString], key: &str, val: &str) -> bool {
    has_triplet(argv, "--setenv", key, val)
}

/// Convenience: `--tmpfs cage` present?
pub fn has_tmpfs(argv: &[OsString], cage: &str) -> bool {
    has_pair(argv, "--tmpfs", cage)
}
