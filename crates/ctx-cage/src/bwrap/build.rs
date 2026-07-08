//! Pure bwrap argv builder. Composes a deterministic `Vec<OsString>`
//! from a [`BwrapConfig`] by delegating to small `add_*` and `push_*`
//! helpers; each helper keeps its block well under the length tier.

use std::ffi::OsString;
use std::path::Path;

use super::config::{BwrapConfig, CAGE_BIN, CAGE_RULES_PATH, CAGE_RUN_DIR};

/// Build the bwrap argv. Output starts with `"bwrap"` so the caller
/// can do `Command::new(&argv[0]).args(&argv[1..])`.
///
/// The cage is always offline (`--unshare-net` is unconditional): the
/// API proxy socket under [`CAGE_RUN_DIR`] is the sole egress.
#[must_use]
pub fn build_bwrap_args(c: &BwrapConfig) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec!["bwrap".into()];
    add_base_isolation(&mut a, c);
    add_workspace(&mut a, c);
    add_toolchain(&mut a, c);
    add_tools_and_rules(&mut a, c);
    add_envs(&mut a, c);
    a.push("--".into());
    a.extend(c.cage_cmd.iter().cloned());
    a
}

/// Standard unshares + base RO binds + `--clearenv` (no host env leak).
/// Network is never shared.
fn add_base_isolation(a: &mut Vec<OsString>, c: &BwrapConfig) {
    for flag in [
        "--unshare-user",
        "--unshare-pid",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-net",
        "--die-with-parent",
        "--clearenv",
    ] {
        a.push(flag.into());
    }
    if c.new_session {
        a.push("--new-session".into());
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

/// Target project: READ-WRITE bind at its own real host path (no fixed
/// alias — see ADR-046), then empty-file masks over each secret path
/// so its contents are hidden (but still readable-as-empty — readers
/// like git must not break) even though the workspace is writable.
fn add_workspace(a: &mut Vec<OsString>, c: &BwrapConfig) {
    let root = c.target_root.clone().into_os_string();
    a.push("--bind".into());
    a.push(root.clone());
    a.push(root.clone());
    for rel in &c.secret_masks {
        let mask_dest = c.target_root.join(rel);
        push_ro_bind(a, &c.mask_file, &mask_dest.to_string_lossy());
    }
    a.push("--chdir".into());
    a.push(root);
}

/// Toolchain directories bound read-only at their host paths, so
/// cargo/rustup resolve exactly as configured while the rest of
/// `$HOME` stays invisible.
fn add_toolchain(a: &mut Vec<OsString>, c: &BwrapConfig) {
    for dir in &c.toolchain {
        let s = dir.to_string_lossy().into_owned();
        push_ro_bind(a, dir, &s);
    }
}

/// `/cage/bin` tmpfs + host tool binds, rules bind, run-dir bind, and
/// any RW binds (the synthesized claude config).
fn add_tools_and_rules(a: &mut Vec<OsString>, c: &BwrapConfig) {
    a.push("--tmpfs".into());
    a.push(CAGE_BIN.into());
    for (host, cage) in &c.tool_binds {
        push_ro_bind(a, host, cage);
    }
    push_ro_bind(a, &c.cage_rules_path, CAGE_RULES_PATH);
    push_ro_bind(a, &c.rundir, CAGE_RUN_DIR);
    for (host, cage) in &c.rw_binds {
        a.push("--bind".into());
        a.push(host.clone().into_os_string());
        a.push(cage.into());
    }
}

/// Explicit env (matches `--clearenv`: only what is set here exists).
fn add_envs(a: &mut Vec<OsString>, c: &BwrapConfig) {
    for (key, val) in &c.env {
        a.push("--setenv".into());
        a.push(key.into());
        a.push(val.into());
    }
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
