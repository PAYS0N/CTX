//! Tests for the Claude Code Stop-hook mode: report-only — it must
//! never regenerate (that is `ctx-run`'s post-session job) and must
//! stay silent on a fresh tree.

use std::cell::RefCell;
use std::path::PathBuf;

use ctx_scan::cli::stop_hook;
use ctx_scan::runner::{check_run, update_run};
use ctx_summarize::agent::Agent;
use ctx_summarize::error::SummError;

/// Minimal always-succeeding agent, used only to seed a fresh tree.
struct StubAgent {
    /// Number of completions served.
    calls: RefCell<usize>,
}

impl Agent for StubAgent {
    fn complete(&self, _system: &str, _user: &str) -> Result<String, SummError> {
        *self.calls.borrow_mut() += 1;
        Ok("SUMMARY".to_owned())
    }
}

/// Unique tmpdir path for a given test label.
fn test_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ctx-scan-stophook-{label}"))
}

/// Absolute path to the workspace prompt files, independent of cwd.
fn prompts_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../prompts")
        .to_string_lossy()
        .into_owned()
}

/// One source file fixture.
fn fixture(base: &std::path::Path) -> Result<(), std::io::Error> {
    drop(std::fs::remove_dir_all(base));
    std::fs::create_dir_all(base.join("src"))?;
    std::fs::write(base.join("src/lib.rs"), "fn f() {}")
}

#[test]
fn stale_tree_reports_and_never_regenerates() {
    let base = test_dir("report");
    fixture(&base).expect("fixture");
    let mut out = Vec::new();
    stop_hook(&base, &mut out).expect("hook");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("systemMessage"), "got: {text}");
    assert!(text.contains("stale"), "got: {text}");
    assert!(text.contains("--update"), "got: {text}");
    // Report-only: the tree must still be stale afterwards.
    assert!(!check_run(&base).expect("check").is_fresh());
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn fresh_tree_emits_nothing() {
    let base = test_dir("fresh");
    fixture(&base).expect("fixture");
    let agent = StubAgent {
        calls: RefCell::new(0),
    };
    update_run(&base, &prompts_path(), &agent, false).expect("seed");
    let mut out = Vec::new();
    stop_hook(&base, &mut out).expect("hook");
    assert!(out.is_empty(), "fresh tree must stay silent");
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn oversized_backlog_hint_includes_approve() {
    let base = test_dir("approve");
    drop(std::fs::remove_dir_all(&base));
    std::fs::create_dir_all(base.join("src")).expect("mkdir");
    for i in 0..=ctx_summarize::runner::MAX_TARGETS {
        std::fs::write(base.join(format!("src/f{i}.rs")), "fn f() {}").expect("write");
    }
    let mut out = Vec::new();
    stop_hook(&base, &mut out).expect("hook");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("--update --approve"), "got: {text}");
    drop(std::fs::remove_dir_all(&base));
}
