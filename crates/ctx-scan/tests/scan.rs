//! Hermetic and filesystem-backed integration tests for ctx-scan.

// rationale: integration scenario file; many small hermetic tests naturally accrete past the 250-line soft tier.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use ctx_scan::error::ScanError;
use ctx_scan::fs::ScanFs;
use ctx_scan::hash;
use ctx_scan::runner::{check_run, summarize, update_run};
use ctx_scan::walker::walk_dir;
use ctx_summarize::agent::Agent;
use ctx_summarize::error::SummError;
use ctx_summarize::fs::Fs;
use ctx_summarize::runner;

// ── in-memory fakes ──────────────────────────────────────────────────────────

/// In-memory filesystem for hermetic tests.
struct FakeFs {
    /// path → contents.
    map: RefCell<BTreeMap<String, String>>,
}

impl FakeFs {
    /// Seed with both embedded prompts and one source file at `src/lib.rs`.
    fn with_source() -> Self {
        let mut m = BTreeMap::new();
        m.insert("prompts/summarizer-leaf.md".to_owned(), "LEAF".to_owned());
        m.insert(
            "prompts/summarizer-rollup.md".to_owned(),
            "ROLLUP".to_owned(),
        );
        m.insert("src/lib.rs".to_owned(), "fn foo() {}".to_owned());
        Self {
            map: RefCell::new(m),
        }
    }

    /// Seed with prompts only (no source files).
    fn prompts_only() -> Self {
        let mut m = BTreeMap::new();
        m.insert("prompts/summarizer-leaf.md".to_owned(), "LEAF".to_owned());
        m.insert(
            "prompts/summarizer-rollup.md".to_owned(),
            "ROLLUP".to_owned(),
        );
        Self {
            map: RefCell::new(m),
        }
    }
}

impl Fs for FakeFs {
    fn read(&self, rel: &str) -> Result<String, SummError> {
        self.map
            .borrow()
            .get(rel)
            .cloned()
            .ok_or_else(|| SummError::Io {
                path: rel.to_owned(),
                detail: "missing".to_owned(),
            })
    }

    fn write(&self, rel: &str, contents: &str) -> Result<(), SummError> {
        self.map
            .borrow_mut()
            .insert(rel.to_owned(), contents.to_owned());
        Ok(())
    }

    fn exists(&self, rel: &str) -> bool {
        self.map.borrow().contains_key(rel)
    }

    fn list_dir(&self, rel: &str) -> Result<Vec<String>, SummError> {
        let prefix = format!("{rel}/");
        let mut names: Vec<String> = Vec::new();
        for key in self.map.borrow().keys() {
            if let Some(rest) = key.strip_prefix(&prefix) {
                if let Some(first) = rest.split('/').next() {
                    let name = first.to_owned();
                    if !name.is_empty() && !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    fn is_ignored(&self, _rel: &str) -> Result<bool, SummError> {
        Ok(false)
    }

    fn remove(&self, rel: &str) -> Result<(), SummError> {
        self.map.borrow_mut().remove(rel);
        Ok(())
    }
}

/// Records every `(system, user)` completion call; always succeeds.
struct RecordingAgent {
    /// Recorded `(system, user)` pairs in call order.
    calls: RefCell<Vec<(String, String)>>,
}

impl Agent for RecordingAgent {
    fn complete(&self, system: &str, user: &str) -> Result<String, SummError> {
        self.calls
            .borrow_mut()
            .push((system.to_owned(), user.to_owned()));
        Ok(format!("SUMMARY[{system}]"))
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a fresh `RecordingAgent`.
const fn recording() -> RecordingAgent {
    RecordingAgent {
        calls: RefCell::new(Vec::new()),
    }
}

/// Unique tmpdir path for a given test label.
fn test_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ctx-scan-test-{label}"))
}

// ── hermetic tests ────────────────────────────────────────────────────────────

#[test]
fn summarize_writes_leaves_rollups_and_readme() {
    let fs = FakeFs::with_source();
    let agent = recording();
    let summary = summarize(&fs, &agent, &["src/lib.rs".to_owned()], false).expect("summarize");
    assert!(summary.readme_written);
    assert_eq!(summary.leaves_written, vec![".context/src/lib.rs.ctx"]);
    assert!(fs.map.borrow().contains_key(".context/README.md"));
}

#[test]
fn scope_gate_blocks_unapproved_large_runs() {
    let fs = FakeFs::prompts_only();
    let agent = recording();
    let targets: Vec<String> = (0..=runner::MAX_TARGETS)
        .map(|i| format!("{i}.rs"))
        .collect();
    let err = summarize(&fs, &agent, &targets, false).expect_err("should be refused");
    assert!(matches!(
        err,
        ScanError::Summarize(SummError::ScopeTooLarge { .. })
    ));
}

#[test]
fn scope_gate_allows_approved_large_runs() {
    let mut m = BTreeMap::new();
    m.insert("prompts/summarizer-leaf.md".to_owned(), "LEAF".to_owned());
    m.insert(
        "prompts/summarizer-rollup.md".to_owned(),
        "ROLLUP".to_owned(),
    );
    let targets: Vec<String> = (0..=runner::MAX_TARGETS)
        .map(|i| format!("{i}.rs"))
        .collect();
    for t in &targets {
        m.insert(t.clone(), "content".to_owned());
    }
    let fs = FakeFs {
        map: RefCell::new(m),
    };
    let agent = recording();
    assert!(summarize(&fs, &agent, &targets, true).is_ok());
}

#[test]
fn embedded_prompts_are_served_by_scan_fs() {
    let base = test_dir("prompts");
    std::fs::create_dir_all(&base).expect("mkdir");
    let fs = ScanFs::new(base.clone());
    let leaf = fs.read("prompts/summarizer-leaf.md").expect("read leaf");
    let rollup = fs
        .read("prompts/summarizer-rollup.md")
        .expect("read rollup");
    assert!(!leaf.is_empty());
    assert!(!rollup.is_empty());
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn is_ignored_returns_false_outside_git() {
    let base = test_dir("notgit");
    std::fs::create_dir_all(&base).expect("mkdir");
    let fs = ScanFs::new(base.clone());
    let result = fs.is_ignored("any.rs").expect("is_ignored");
    assert!(!result);
    drop(std::fs::remove_dir_all(&base));
}

// ── walker integration test ───────────────────────────────────────────────────

/// Create the walk-test fixture directory tree under `base`.
fn create_walk_fixture(base: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(base.join("src"))?;
    std::fs::create_dir_all(base.join(".context"))?;
    std::fs::write(base.join("src/main.rs"), "fn main() {}")?;
    std::fs::write(base.join("logo.png"), b"\x89PNG")?;
    std::fs::write(base.join(".context/rollup.ctx"), "old")?;
    Ok(())
}

#[test]
fn walk_collects_files_and_excludes_context_and_binaries() {
    let base = test_dir("walk");
    drop(std::fs::remove_dir_all(&base));
    create_walk_fixture(&base).expect("fixture setup");

    let files = walk_dir(&base).expect("walk");

    assert!(
        files.contains(&"src/main.rs".to_owned()),
        "rs file included"
    );
    assert!(
        !files.iter().any(|f| {
            std::path::Path::new(f)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        }),
        "png excluded"
    );
    assert!(
        !files.iter().any(|f| f.starts_with(".context")),
        ".context excluded"
    );
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn walk_scope_honors_ctxignore_and_builtin_target_default() {
    let base = test_dir("scope");
    drop(std::fs::remove_dir_all(&base));
    std::fs::create_dir_all(base.join("src")).expect("mkdir src");
    std::fs::create_dir_all(base.join("target/debug")).expect("mkdir target");
    std::fs::create_dir_all(base.join("gen")).expect("mkdir gen");
    std::fs::write(base.join("src/main.rs"), "fn main() {}").expect("write src");
    std::fs::write(base.join("target/debug/junk.rs"), "junk").expect("write target");
    std::fs::write(base.join("gen/out.rs"), "generated").expect("write gen");
    std::fs::write(base.join(".ctxignore"), "gen/\n").expect("write ctxignore");

    let files = walk_dir(&base).expect("walk");

    assert!(files.contains(&"src/main.rs".to_owned()), "src included");
    assert!(
        !files.iter().any(|f| f.starts_with("target/")),
        "target/ excluded by built-in default"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("gen/")),
        "gen/ excluded by .ctxignore"
    );
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

#[test]
fn check_reports_everything_stale_without_sidecars_then_fresh_after_store() {
    let base = test_dir("hash-cycle");
    drop(std::fs::remove_dir_all(&base));
    create_hash_fixture(&base).expect("fixture");

    let first = check_run(&base).expect("check");
    assert!(!first.is_fresh(), "no sidecars -> stale");
    assert!(first.changed_files.contains(&"src/main.rs".to_owned()));
    assert!(first.stale_dirs.contains(&"src".to_owned()));
    assert!(first.stale_dirs.contains(&String::new()), "root stale");

    // Store the current state; the tree is then fresh.
    let files = walk_dir(&base).expect("walk");
    let current = hash::compute(&base, &files).expect("compute");
    hash::store(&ScanFs::new(base.clone()), &current).expect("store");
    let second = check_run(&base).expect("recheck");
    assert!(second.is_fresh(), "stored -> fresh");

    // Touch one file: exactly its leaf and its ancestor dirs go stale.
    std::fs::write(base.join("src/main.rs"), "fn main() { edited(); }").expect("edit");
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
    let first = update_run(&base, &agent, false).expect("first update");
    assert!(!first.is_fresh());
    assert_seeded(&base);
    let calls_after_first = agent.calls.borrow().len();

    // Second update with no source change: fresh, zero agent calls.
    let second = update_run(&base, &agent, false).expect("second update");
    assert!(second.is_fresh());
    assert_eq!(agent.calls.borrow().len(), calls_after_first);

    // Edit one file: exactly one leaf + two rollups regenerate.
    std::fs::write(base.join("src/main.rs"), "fn main() { edited(); }").expect("edit");
    let third = update_run(&base, &agent, false).expect("third update");
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
    update_run(&base, &agent, false).expect("seed update");
    assert!(base.join(".context/top.rs.ctx").is_file());

    std::fs::remove_file(base.join("top.rs")).expect("delete source");
    let report = update_run(&base, &agent, false).expect("update after delete");
    assert_eq!(report.orphan_leaves, vec![".context/top.rs.ctx".to_owned()]);
    assert!(!base.join(".context/top.rs.ctx").exists());
    assert!(check_run(&base).expect("final check").is_fresh());
    drop(std::fs::remove_dir_all(&base));
}
