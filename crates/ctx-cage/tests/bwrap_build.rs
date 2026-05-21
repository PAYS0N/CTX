//! Argv-shape tests for [`build_bwrap_args`]. Pure; never invokes
//! `bwrap`. Helpers live in `tests/common/mod.rs`.

mod common;

use std::ffi::OsString;

use ctx_cage::bwrap::{
    build_bwrap_args, CAGE_BIN, CAGE_CLAUDE_CONFIG, CAGE_CLAUDE_CRED, CAGE_RULES_PATH,
    CAGE_SOCK_DIR, WORK_DIR,
};
use ctx_cage::error::CageError;

use common::{has_pair, has_ro_bind, has_setenv, has_tmpfs, sample_claude_binds, sample_config};

#[test]
fn starts_with_bwrap_and_injects_isolation_flags() -> Result<(), CageError> {
    let argv = build_bwrap_args(&sample_config())?;
    assert_eq!(argv.first(), Some(&OsString::from("bwrap")));
    for flag in [
        "--unshare-user",
        "--unshare-pid",
        "--unshare-ipc",
        "--unshare-uts",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--unshare-net",
    ] {
        assert!(
            argv.contains(&OsString::from(flag)),
            "missing flag {flag} in {argv:?}"
        );
    }
    Ok(())
}

#[test]
fn overlays_tmpfs_on_every_discovered_crate_dir() -> Result<(), CageError> {
    let argv = build_bwrap_args(&sample_config())?;
    for cage_path in [
        "/work/crates/a/src",
        "/work/crates/b/src",
        "/work/crates/a/tests",
        "/work/target",
    ] {
        assert!(has_tmpfs(&argv, cage_path), "missing tmpfs of {cage_path}");
    }
    Ok(())
}

#[test]
fn binds_target_rules_sock_and_both_client_names() -> Result<(), CageError> {
    let argv = build_bwrap_args(&sample_config())?;
    let client_at_access = format!("{CAGE_BIN}/ctx-access");
    let client_at_verify = format!("{CAGE_BIN}/ctx-verify");
    let pairs: [(&str, &str); 5] = [
        ("/abs/proj", WORK_DIR),
        ("/abs/cage-rules.md", CAGE_RULES_PATH),
        ("/tmp/ctxcage-sock", CAGE_SOCK_DIR),
        ("/abs/target/debug/ctx-cage-client", &client_at_access),
        ("/abs/target/debug/ctx-cage-client", &client_at_verify),
    ];
    for (host, cage) in pairs {
        assert!(has_ro_bind(&argv, host, cage), "missing {host} -> {cage}");
    }
    Ok(())
}

#[test]
fn sets_the_post_clearenv_env_explicitly() -> Result<(), CageError> {
    let argv = build_bwrap_args(&sample_config())?;
    let sock = format!("{CAGE_SOCK_DIR}/ctx.sock");
    let path = format!("{CAGE_BIN}:/usr/bin:/bin");
    let envs: [(&str, &str); 7] = [
        ("HOME", "/tmp"),
        ("USER", "cage"),
        ("LANG", "C.UTF-8"),
        ("TERM", "xterm-256color"),
        ("TASK", "t1"),
        ("CTX_SOCK", &sock),
        ("PATH", &path),
    ];
    for (k, v) in envs {
        assert!(has_setenv(&argv, k, v), "missing setenv {k}={v}");
    }
    Ok(())
}

#[test]
fn allow_net_drops_unshare_net() -> Result<(), CageError> {
    let mut c = sample_config();
    c.allow_net = true;
    let argv = build_bwrap_args(&c)?;
    assert!(!argv.contains(&OsString::from("--unshare-net")));
    Ok(())
}

#[test]
fn claude_mode_binds_runtime_dns_tls_creds_and_config() -> Result<(), CageError> {
    let mut c = sample_config();
    c.allow_net = true;
    c.claude = Some(sample_claude_binds());
    let argv = build_bwrap_args(&c)?;
    let claude_at = format!("{CAGE_BIN}/claude");
    let ro_pairs: [(&str, &str); 5] = [
        ("/home/u/.local/share/claude/versions/9.9.9", &claude_at),
        ("/etc/resolv.conf", "/etc/resolv.conf"),
        ("/etc/ssl", "/etc/ssl"),
        ("/host/cage-nsswitch.conf", "/etc/nsswitch.conf"),
        ("/home/u/.claude/.credentials.json", CAGE_CLAUDE_CRED),
    ];
    for (host, cage) in ro_pairs {
        assert!(has_ro_bind(&argv, host, cage), "missing {host} -> {cage}");
    }
    assert!(has_pair(&argv, "--bind", "/host/tmp/claude.json"));
    assert!(argv.contains(&OsString::from(CAGE_CLAUDE_CONFIG)));
    Ok(())
}

#[test]
fn claude_without_net_is_a_protocol_error() {
    let mut c = sample_config();
    c.claude = Some(sample_claude_binds());
    c.allow_net = false;
    let err = build_bwrap_args(&c).expect_err("must reject");
    assert!(matches!(err, CageError::Protocol(_)));
}

#[test]
fn cage_cmd_appears_after_a_double_dash_terminator() -> Result<(), CageError> {
    let mut c = sample_config();
    c.cage_cmd = vec![
        OsString::from("/cage/bin/ctx-access"),
        OsString::from("manifest"),
    ];
    let argv = build_bwrap_args(&c)?;
    let dash_idx = argv
        .iter()
        .position(|s| s == &OsString::from("--"))
        .expect("must contain a -- terminator");
    let after: Vec<&OsString> = argv.iter().skip(dash_idx + 1).collect();
    assert_eq!(after.len(), 2);
    assert_eq!(
        after.first().map(|s| s.as_os_str()),
        Some("/cage/bin/ctx-access".as_ref())
    );
    assert_eq!(
        after.get(1).map(|s| s.as_os_str()),
        Some("manifest".as_ref())
    );
    Ok(())
}
