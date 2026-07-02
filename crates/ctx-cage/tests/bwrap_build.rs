//! Tests for the pure bwrap argv builder: safety-cage semantics —
//! writable workspace, masked secrets, unconditional network unshare,
//! explicit env, RO toolchain and tool binds.

use std::ffi::OsString;
use std::path::PathBuf;

use ctx_cage::bwrap::{build_bwrap_args, BwrapConfig, CAGE_BIN, CAGE_RULES_PATH, CAGE_RUN_DIR};

/// A representative config exercising every bind family.
fn sample_config() -> BwrapConfig {
    BwrapConfig {
        target_root: PathBuf::from("/home/u/proj"),
        secret_masks: vec![".env".to_owned(), ".git/config".to_owned()],
        mask_file: PathBuf::from("/tmp/run/empty-mask"),
        toolchain: vec![
            PathBuf::from("/home/u/.cargo"),
            PathBuf::from("/home/u/.rustup"),
        ],
        tool_binds: vec![(
            PathBuf::from("/host/bin/ctx-verify"),
            format!("{CAGE_BIN}/ctx-verify"),
        )],
        rw_binds: vec![(
            PathBuf::from("/tmp/run/claude.json"),
            "/tmp/.claude.json".to_owned(),
        )],
        rundir: PathBuf::from("/tmp/run"),
        cage_rules_path: PathBuf::from("/tmp/run/cage-rules.md"),
        env: vec![("PATH".to_owned(), "/cage/bin:/usr/bin:/bin".to_owned())],
        new_session: true,
        cage_cmd: vec!["sh".into(), "-c".into(), "true".into()],
    }
}

/// Position of `needle` in `argv`, if present.
fn find(argv: &[OsString], needle: &str) -> Option<usize> {
    argv.iter().position(|a| a == needle)
}

/// Assert the triple `flag host cage` appears contiguously in `argv`.
fn assert_bind(argv: &[OsString], flag: &str, host: &str, cage: &str) {
    let found = argv.windows(3).any(|w| {
        w.first().is_some_and(|a| a == flag)
            && w.get(1).is_some_and(|a| a == host)
            && w.get(2).is_some_and(|a| a == cage)
    });
    assert!(found, "expected `{flag} {host} {cage}` in argv");
}

#[test]
fn workspace_is_bound_read_write_with_regular_file_secret_masks() {
    let argv = build_bwrap_args(&sample_config());
    assert_bind(&argv, "--bind", "/home/u/proj", "/work");
    // Masks must be a regular file, never /dev/null: bind mounts carry
    // nodev, so a masked device node breaks every reader (git).
    assert_bind(&argv, "--ro-bind", "/tmp/run/empty-mask", "/work/.env");
    assert_bind(
        &argv,
        "--ro-bind",
        "/tmp/run/empty-mask",
        "/work/.git/config",
    );
    assert!(!argv.iter().any(|a| a == "/dev/null"));
}

#[test]
fn network_is_always_unshared_and_env_cleared() {
    let argv = build_bwrap_args(&sample_config());
    assert!(find(&argv, "--unshare-net").is_some(), "must be offline");
    assert!(find(&argv, "--clearenv").is_some(), "no host env leak");
    assert!(find(&argv, "--die-with-parent").is_some());
    assert!(find(&argv, "--new-session").is_some());
}

#[test]
fn toolchain_and_tools_are_read_only() {
    let argv = build_bwrap_args(&sample_config());
    assert_bind(&argv, "--ro-bind", "/home/u/.cargo", "/home/u/.cargo");
    assert_bind(&argv, "--ro-bind", "/home/u/.rustup", "/home/u/.rustup");
    let cage_verify = format!("{CAGE_BIN}/ctx-verify");
    assert_bind(&argv, "--ro-bind", "/host/bin/ctx-verify", &cage_verify);
    assert_bind(
        &argv,
        "--ro-bind",
        "/tmp/run/cage-rules.md",
        CAGE_RULES_PATH,
    );
    assert_bind(&argv, "--ro-bind", "/tmp/run", CAGE_RUN_DIR);
}

#[test]
fn rw_binds_and_env_and_command_are_emitted() {
    let argv = build_bwrap_args(&sample_config());
    assert_bind(&argv, "--bind", "/tmp/run/claude.json", "/tmp/.claude.json");
    assert_bind(&argv, "--setenv", "PATH", "/cage/bin:/usr/bin:/bin");
    let sep = find(&argv, "--").expect("separator");
    assert_eq!(argv.get(sep + 1), Some(&OsString::from("sh")));
}

#[test]
fn interactive_mode_drops_new_session() {
    let mut cfg = sample_config();
    cfg.new_session = false;
    let argv = build_bwrap_args(&cfg);
    assert!(find(&argv, "--new-session").is_none());
    assert!(find(&argv, "--unshare-net").is_some(), "still offline");
}
