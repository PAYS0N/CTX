//! Pure bwrap argv builder. Composes a deterministic `Vec<OsString>`
//! from a [`BwrapConfig`] by delegating to small `add_*` and `push_*`
//! helpers; each helper keeps its block well under the length tier.

use std::ffi::OsString;
use std::path::Path;

use crate::error::CageError;

use super::config::{
    BwrapConfig, ClaudeBinds, CAGE_BIN, CAGE_CLAUDE_CONFIG, CAGE_CLAUDE_CRED, CAGE_RULES_PATH,
    CAGE_SOCK_DIR, WORK_DIR,
};

/// Build the bwrap argv. Output starts with `"bwrap"` so the caller
/// can do `Command::new(&argv[0]).args(&argv[1..])`.
///
/// # Errors
///
/// [`CageError::Protocol`] if [`BwrapConfig::claude`] is `Some` but
/// [`BwrapConfig::allow_net`] is `false` (the cage cannot reach the
/// API without network).
pub fn build_bwrap_args(c: &BwrapConfig) -> Result<Vec<OsString>, CageError> {
    if c.claude.is_some() && !c.allow_net {
        return Err(CageError::Protocol(
            "--claude requires --net (no --unshare-net)".to_owned(),
        ));
    }
    let mut a: Vec<OsString> = vec!["bwrap".into()];
    add_base_isolation(&mut a, c);
    add_target_binds(&mut a, c);
    add_cage_bin_and_rules(&mut a, c);
    add_envs(&mut a, c);
    if let Some(cb) = &c.claude {
        add_claude_binds(&mut a, cb);
    }
    a.push("--".into());
    a.extend(c.cage_cmd.iter().cloned());
    Ok(a)
}

/// Standard unshares + base RO binds + `--clearenv` (no host env leak).
fn add_base_isolation(a: &mut Vec<OsString>, c: &BwrapConfig) {
    for flag in [
        "--unshare-user",
        "--unshare-pid",
        "--unshare-ipc",
        "--unshare-uts",
        "--die-with-parent",
        "--clearenv",
    ] {
        a.push(flag.into());
    }
    if c.new_session {
        a.push("--new-session".into());
    }
    if !c.allow_net {
        a.push("--unshare-net".into());
    }
    for host in ["/usr", "/bin", "/lib", "/lib64", "/etc/alternatives"] {
        push_ro_bind_str(a, host, host);
    }
    a.push("--proc".into());
    a.push("/proc".into());
    a.push("--dev".into());
    a.push("/dev".into());
    a.push("--tmpfs".into());
    a.push("/tmp".into());
}

/// Target project: RO bind at `/work`, then tmpfs overlays for each
/// discovered crate src/tests dir and the `target/` build tree.
fn add_target_binds(a: &mut Vec<OsString>, c: &BwrapConfig) {
    push_ro_bind(a, &c.target_root, WORK_DIR);
    for rel in c.crates.srcs.iter().chain(c.crates.tests.iter()) {
        let cage_path = format!("{WORK_DIR}/{}", rel.display());
        a.push("--tmpfs".into());
        a.push(cage_path.into());
    }
    a.push("--tmpfs".into());
    a.push(format!("{WORK_DIR}/target").into());
}

/// `/cage/bin` tmpfs, broker-tool binds, rules + sockdir bind, chdir.
fn add_cage_bin_and_rules(a: &mut Vec<OsString>, c: &BwrapConfig) {
    a.push("--tmpfs".into());
    a.push(CAGE_BIN.into());
    let ca = format!("{CAGE_BIN}/ctx-access");
    let cv = format!("{CAGE_BIN}/ctx-verify");
    push_ro_bind(a, &c.client_binary, &ca);
    push_ro_bind(a, &c.client_binary, &cv);
    push_ro_bind(a, &c.cage_rules_path, CAGE_RULES_PATH);
    push_ro_bind(a, &c.sockdir, CAGE_SOCK_DIR);
    a.push("--chdir".into());
    a.push(WORK_DIR.into());
}

/// Explicit env (matches `--clearenv`: only what is set here exists).
fn add_envs(a: &mut Vec<OsString>, c: &BwrapConfig) {
    let sock = format!("{CAGE_SOCK_DIR}/{}", c.sockname);
    let path = format!("{CAGE_BIN}:/usr/bin:/bin");
    push_setenv(a, "PATH", &path);
    push_setenv(a, "HOME", "/tmp");
    push_setenv(a, "USER", "cage");
    push_setenv(a, "LANG", "C.UTF-8");
    push_setenv(a, "TERM", &c.term);
    push_setenv(a, "CTX_SOCK", &sock);
    push_setenv(a, "TASK", &c.task_id);
}

/// `--claude` mode: bind the binary, DNS/TLS, the credential (RO) and
/// the synthesized `~/.claude.json` (rw, ephemeral).
fn add_claude_binds(a: &mut Vec<OsString>, cb: &ClaudeBinds) {
    push_ro_bind(a, &cb.claude_binary, &format!("{CAGE_BIN}/claude"));
    push_ro_bind(a, &cb.resolv_conf, "/etc/resolv.conf");
    push_ro_bind_str(a, "/etc/hosts", "/etc/hosts");
    push_ro_bind_str(a, "/etc/ssl", "/etc/ssl");
    push_ro_bind(a, &cb.nsswitch_conf, "/etc/nsswitch.conf");
    push_ro_bind(a, &cb.credentials, CAGE_CLAUDE_CRED);
    a.push("--bind".into());
    a.push(cb.claude_config_json.clone().into_os_string());
    a.push(CAGE_CLAUDE_CONFIG.into());
}

/// `--ro-bind <host> <cage>`.
fn push_ro_bind(a: &mut Vec<OsString>, host: &Path, cage: &str) {
    a.push("--ro-bind".into());
    a.push(host.into());
    a.push(cage.into());
}

/// `--ro-bind <host-as-str> <cage>` — convenience for fixed paths.
fn push_ro_bind_str(a: &mut Vec<OsString>, host: &str, cage: &str) {
    a.push("--ro-bind".into());
    a.push(host.into());
    a.push(cage.into());
}

/// `--setenv <key> <val>`.
fn push_setenv(a: &mut Vec<OsString>, key: &str, val: &str) {
    a.push("--setenv".into());
    a.push(key.into());
    a.push(val.into());
}
