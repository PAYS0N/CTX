//! Hermetic and filesystem-backed integration tests for ctx-scan's
//! summarize/hash/check/update pipeline. Walker-only tests live in
//! `walker.rs`; shared fixtures live in `common/`.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::{prompts_path, recording, seed_prompts, FakeFs};
use ctx_scan::error::ScanError;
use ctx_scan::hash;
use ctx_scan::runner::{check_run, summarize, update_run};
use ctx_scan::walker::walk_dir;
use ctx_summarize::error::SummError;
use ctx_summarize::fs::{Fs, StdFs};
use ctx_summarize::progress::NoProgress;
use ctx_summarize::runner::{self, Models};

impl FakeFs {
    /// Seed with both embedded prompts and one source file at `src/lib.rs`.
    fn with_source() -> Self {
        let mut m = BTreeMap::new();
        seed_prompts(&mut m);
        m.insert("src/lib.rs".to_owned(), "fn foo() {}".to_owned());
        Self {
            map: std::cell::RefCell::new(m),
        }
    }

    /// Seed with prompts only (no source files).
    fn prompts_only() -> Self {
        let mut m = BTreeMap::new();
        seed_prompts(&mut m);
        Self {
            map: std::cell::RefCell::new(m),
        }
    }
}

/// Unique tmpdir path for a given test label.
fn test_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ctx-scan-test-{label}"))
}

/// Model passed to every test call site; content is irrelevant here.
const MODEL: &str = "claude-sonnet-5";

/// Leaf/rollup model bundle every test call site shares.
const MODELS: Models<'static> = Models {
    leaf: MODEL,
    rollup: MODEL,
};

// ── hermetic tests ────────────────────────────────────────────────────────────

#[test]
fn summarize_writes_leaves_rollups_and_readme() {
    let fs = FakeFs::with_source();
    let agent = recording();
    let prompts = runner::load_prompts(&fs, "prompts").expect("prompts");
    let target = "src/lib.rs".to_owned();
    let summary = summarize(
        &fs,
        &agent,
        &prompts,
        &[target],
        &MODELS,
        false,
        &NoProgress,
    )
    .expect("summarize");
    assert!(summary.readme_written);
    assert_eq!(summary.leaves_written, vec![".context/src/lib.rs.ctx"]);
    assert!(fs.map.borrow().contains_key(".context/README.md"));
}

#[test]
fn scope_gate_blocks_unapproved_large_runs() {
    let fs = FakeFs::prompts_only();
    let agent = recording();
    let prompts = runner::load_prompts(&fs, "prompts").expect("prompts");
    let targets: Vec<String> = (0..=runner::MAX_TARGETS)
        .map(|i| format!("{i}.rs"))
        .collect();
    let err = summarize(&fs, &agent, &prompts, &targets, &MODELS, false, &NoProgress)
        .expect_err("should be refused");
    assert!(matches!(
        err,
        ScanError::Summarize(SummError::ScopeTooLarge { .. })
    ));
}

#[test]
fn scope_gate_allows_approved_large_runs() {
    let mut m = BTreeMap::new();
    seed_prompts(&mut m);
    let targets: Vec<String> = (0..=runner::MAX_TARGETS)
        .map(|i| format!("{i}.rs"))
        .collect();
    for t in &targets {
        m.insert(t.clone(), "content".to_owned());
    }
    let fs = FakeFs {
        map: std::cell::RefCell::new(m),
    };
    let agent = recording();
    let prompts = runner::load_prompts(&fs, "prompts").expect("prompts");
    assert!(summarize(&fs, &agent, &prompts, &targets, &MODELS, true, &NoProgress).is_ok());
}

#[test]
fn is_ignored_follows_ctxignore_and_needs_no_git() {
    let base = test_dir("scopefs");
    drop(std::fs::remove_dir_all(&base));
    std::fs::create_dir_all(&base).expect("mkdir");
    std::fs::write(base.join(".ctxignore"), "gen/\n").expect("write ctxignore");
    let fs = StdFs::new(base.clone());
    assert!(!fs.is_ignored("any.rs").expect("in scope"));
    assert!(fs.is_ignored("gen/out.rs").expect("scoped out"));
    assert!(fs.is_ignored("target/debug/x.rs").expect("builtin default"));
    drop(std::fs::remove_dir_all(&base));
}

// ── hash / check / update integration tests ─────────────────────────────────

/// Create a two-level source fixture under `base`.
fn create_hash_fixture(base: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(base.join("src"))?;
    std::fs::write(base.join("src/main.rs"), "fn main() {}")?;
    std::fs::write(base.join("top.rs"), "fn top() {}")?;
    Ok(())
}

/// Compute and persist the hash sidecars for `base` (hashes only, no
/// summaries generated). A test helper, so it returns `Result` rather
/// than unwrapping.
fn store_hashes(base: &std::path::Path) -> Result<(), ScanError> {
    let files = walk_dir(base)?;
    let current = hash::compute(base, &files)?;
    hash::store(&StdFs::new(base.to_path_buf()), &current)
}

#[test]
fn check_cycle_stale_then_missing_artifacts_then_edit_propagates() {
    let base = test_dir("hash-cycle");
    drop(std::fs::remove_dir_all(&base));
    create_hash_fixture(&base).expect("fixture");

    let first = check_run(&base).expect("check");
    assert!(!first.is_fresh(), "no sidecars -> stale");
    assert!(first.stale_dirs.contains(&String::new()), "root stale");

    // Hashes stored but no summaries generated: the hash diff is clean,
    // yet the integrity check flags the missing artifacts (finding #11).
    store_hashes(&base).expect("store");
    let second = check_run(&base).expect("recheck");
    assert!(second.stale_dirs.is_empty() && second.changed_files.is_empty());
    let path = ".context/rollup.ctx".to_owned();
    assert!(second.missing_artifacts.contains(&path));

    // Edit one file: exactly its leaf and ancestor dirs go stale.
    std::fs::write(base.join("src/main.rs"), "fn main() { e(); }").expect("edit");
    let third = check_run(&base).expect("check after edit");
    assert_eq!(third.changed_files, vec!["src/main.rs".to_owned()]);
    assert_eq!(third.stale_dirs, vec!["src".to_owned(), String::new()]);
    drop(std::fs::remove_dir_all(&base));
}

/// Assert the fixture's summary tree artifacts exist after a first update.
fn assert_seeded(base: &std::path::Path) {
    assert!(base.join(".context/src/main.rs.ctx").is_file());
    assert!(base.join(".context/src/rollup.ctx").is_file());
    assert!(base.join(".context/hashes.json").is_file());
}

#[test]
fn update_regenerates_only_stale_summaries_and_refreshes_hashes() {
    let base = test_dir("update");
    drop(std::fs::remove_dir_all(&base));
    create_hash_fixture(&base).expect("fixture");
    let agent = recording();

    // First update: everything is stale, all leaves + rollups produced.
    let first = update_run(&base, &prompts_path(), &agent, &MODELS, false).expect("first update");
    assert!(!first.is_fresh());
    assert_seeded(&base);
    let calls_after_first = agent.calls.borrow().len();

    // Second update with no source change: fresh, zero agent calls.
    let second = update_run(&base, &prompts_path(), &agent, &MODELS, false).expect("second update");
    assert!(second.is_fresh());
    assert_eq!(agent.calls.borrow().len(), calls_after_first);

    // Edit one file: exactly one leaf + two rollups regenerate.
    std::fs::write(base.join("src/main.rs"), "fn main() { edited(); }").expect("edit");
    let third = update_run(&base, &prompts_path(), &agent, &MODELS, false).expect("third update");
    assert_eq!(third.changed_files, vec!["src/main.rs".to_owned()]);
    assert_eq!(
        agent.calls.borrow().len(),
        calls_after_first + 3,
        "one leaf + src rollup + root rollup"
    );
    assert!(check_run(&base).expect("final check").is_fresh());
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn update_removes_orphan_leaf_when_source_is_deleted() {
    let base = test_dir("orphan");
    drop(std::fs::remove_dir_all(&base));
    create_hash_fixture(&base).expect("fixture");
    let agent = recording();
    update_run(&base, &prompts_path(), &agent, &MODELS, false).expect("seed update");
    assert!(base.join(".context/top.rs.ctx").is_file());

    std::fs::remove_file(base.join("top.rs")).expect("delete source");
    let report = update_run(&base, &prompts_path(), &agent, &MODELS, false).expect("after delete");
    assert_eq!(report.orphan_leaves, vec![".context/top.rs.ctx".to_owned()]);
    assert!(!base.join(".context/top.rs.ctx").exists());
    assert!(check_run(&base).expect("final check").is_fresh());
    drop(std::fs::remove_dir_all(&base));
}
