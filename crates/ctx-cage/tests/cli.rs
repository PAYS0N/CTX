//! Tests for [`ctx_cage::cli::resolve_mode`]. Pure; no clap, no
//! filesystem (except for `--task-file` which uses a unique tempdir).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ctx_cage::cli::{resolve_mode, Cli, Mode, ResolveError, SelfTestKind, SpendFlags, TaskFlags};

/// Counter for unique tempfile names across tests.
static SEQ: AtomicU32 = AtomicU32::new(0);

/// A unique tempfile path; the test owns cleanup.
fn fresh_tempfile(label: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ctx-cage-cli-{}-{}-{}.txt",
        label,
        std::process::id(),
        n
    ))
}

/// Builder: a minimal `Cli` with every flag at its default. Free-fn
/// helper, so no `unwrap`/`expect` (workspace rule).
fn base(target: &str) -> Cli {
    Cli {
        target: PathBuf::from(target),
        task_id: None,
        self_test: None,
        task_flags: TaskFlags {
            task: None,
            task_file: None,
            interactive: false,
        },
        spend_flags: SpendFlags {
            allow_spend: false,
            allow_dirty: false,
        },
        verbose_proxy_log: false,
    }
}

#[test]
fn self_test_stub_resolves_without_spend() {
    let mut c = base("/work");
    c.self_test = Some(SelfTestKind::Stub);
    assert_eq!(
        resolve_mode(&c, false).expect("resolve"),
        Mode::SelfTestStub
    );
}

#[test]
fn default_is_interactive_when_spend_is_allowed() {
    assert_eq!(
        resolve_mode(&base("/work"), true).expect("resolve"),
        Mode::Interactive
    );
}

#[test]
fn default_interactive_without_spend_is_blocked() {
    let err = resolve_mode(&base("/work"), false).expect_err("must block");
    assert!(matches!(err, ResolveError::NotAllowed(_)), "got: {err}");
}

#[test]
fn inline_task_becomes_task_mode() {
    let mut c = base("/work");
    c.task_flags.task = Some("do thing".to_owned());
    let mode = resolve_mode(&c, true).expect("resolve");
    assert_eq!(mode, Mode::Task("do thing".to_owned()));
}

#[test]
fn task_file_is_read_from_disk() {
    let path = fresh_tempfile("brief");
    fs::write(&path, "BRIEF FROM FILE\n").expect("write tmp");
    let mut c = base("/work");
    c.task_flags.task_file = Some(path.clone());
    let mode = resolve_mode(&c, true).expect("resolve");
    assert_eq!(mode, Mode::Task("BRIEF FROM FILE\n".to_owned()));
    let _ = fs::remove_file(&path);
}

#[test]
fn billed_task_without_spend_is_blocked() {
    let mut c = base("/work");
    c.task_flags.task = Some("brief".to_owned());
    let err = resolve_mode(&c, false).expect_err("must block");
    assert!(matches!(err, ResolveError::NotAllowed(_)), "got: {err}");
}

#[test]
fn task_and_task_file_together_are_a_conflict() {
    let mut c = base("/work");
    c.task_flags.task = Some("inline".to_owned());
    c.task_flags.task_file = Some(PathBuf::from("/tmp/nonexistent"));
    let err = resolve_mode(&c, true).expect_err("must conflict");
    assert!(matches!(err, ResolveError::Conflict(_)), "got: {err}");
}

#[test]
fn self_test_with_task_is_a_conflict() {
    let mut c = base("/work");
    c.self_test = Some(SelfTestKind::Stub);
    c.task_flags.task = Some("brief".to_owned());
    let err = resolve_mode(&c, true).expect_err("must conflict");
    assert!(matches!(err, ResolveError::Conflict(_)), "got: {err}");
}

#[test]
fn task_and_interactive_together_are_a_conflict() {
    let mut c = base("/work");
    c.task_flags.task = Some("brief".to_owned());
    c.task_flags.interactive = true;
    let err = resolve_mode(&c, true).expect_err("must conflict");
    assert!(matches!(err, ResolveError::Conflict(_)), "got: {err}");
}
