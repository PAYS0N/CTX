//! Tests for [`super::super::proxy_diagnostic`]: the capped diagnostic
//! built from the proxy's log file plus the accept thread's join
//! outcome, surfaced to the user after a billed run.

use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::error::CageError;

use super::super::{proxy_diagnostic, ProxyJoin};

/// A unique proxy-log path; the test owns cleanup.
fn fresh_log_path(label: &str) -> std::path::PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ctx-cage-run-diag-{label}-{}-{n}.log",
        std::process::id()
    ))
}

/// No log content and a clean join must report nothing — the common
/// case must see zero behavior change.
#[test]
fn no_diagnostic_on_absent_log_and_clean_join() {
    let log = fresh_log_path("absent");
    let join: ProxyJoin = Ok(Ok(()));
    assert!(proxy_diagnostic(&log, join, false).is_none());
}

/// Default (non-verbose) mode reports a count and only the last logged
/// line, not the full history.
#[test]
fn summary_mode_reports_count_and_last_line_only() {
    let log = fresh_log_path("summary");
    fs::write(
        &log,
        "connection failed: first\nconnection failed: second\n",
    )
    .expect("write log");
    let join: ProxyJoin = Ok(Ok(()));
    let out = proxy_diagnostic(&log, join, false).expect("diagnostic");
    assert!(out.contains("2 issue"), "got: {out}");
    assert!(out.contains("connection failed: second"), "got: {out}");
    assert!(
        !out.contains("connection failed: first"),
        "summary must not include earlier lines: {out}"
    );
    let _ = fs::remove_file(&log);
}

/// `--verbose-proxy-log` prints the full log, capped at the last 50
/// lines, dropping the earliest entries once over that cap.
#[test]
fn verbose_mode_caps_at_the_last_fifty_lines() {
    use std::fmt::Write as _;

    let log = fresh_log_path("verbose");
    let mut body = String::new();
    for i in 0..60 {
        writeln!(body, "connection failed: {i}").expect("write to String never fails");
    }
    fs::write(&log, &body).expect("write log");
    let join: ProxyJoin = Ok(Ok(()));
    let out = proxy_diagnostic(&log, join, true).expect("diagnostic");
    let mut out_lines = out.lines();
    let header = out_lines.next().expect("header line");
    assert!(header.contains("60 lines"), "got: {header}");
    let tail: Vec<&str> = out_lines.collect();
    assert_eq!(tail.len(), 50, "should cap at 50 lines, got: {tail:?}");
    assert_eq!(tail.first(), Some(&"connection failed: 10"));
    assert_eq!(tail.last(), Some(&"connection failed: 59"));
    let _ = fs::remove_file(&log);
}

/// A returned `Err` from the proxy accept loop folds into the same
/// diagnostic as the per-connection log lines, not a separate message.
#[test]
fn join_error_folds_into_the_same_diagnostic() {
    let log = fresh_log_path("join-err");
    fs::write(&log, "connection failed: dial refused\n").expect("write log");
    let join: ProxyJoin = Ok(Err(CageError::Protocol("accept loop exploded".to_owned())));
    let out = proxy_diagnostic(&log, join, false).expect("diagnostic");
    assert!(out.contains("2 issue"), "got: {out}");
    assert!(out.contains("accept loop exploded"), "got: {out}");
    let _ = fs::remove_file(&log);
}

/// A panicked accept-loop thread is folded in too, even with no logged
/// per-connection failures. Constructs the join outcome directly rather
/// than actually panicking a thread (`clippy::panic` is denied crate-wide,
/// tests included).
#[test]
fn join_panic_produces_a_diagnostic() {
    let log = fresh_log_path("join-panic");
    let join: ProxyJoin = Err(Box::new("boom"));
    let out = proxy_diagnostic(&log, join, false).expect("diagnostic");
    assert!(out.contains("panicked"), "got: {out}");
}
