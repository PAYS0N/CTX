//! Hermetic and filesystem-backed integration tests for ctx-scan.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use ctx_scan::error::ScanError;
use ctx_scan::fs::ScanFs;
use ctx_scan::runner::summarize;
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
