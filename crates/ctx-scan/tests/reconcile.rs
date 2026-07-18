//! Reconciliation tests: derived `.context/` artifacts whose source was
//! deleted, moved, or scoped out are flagged by `--check` (read-only)
//! and removed by pruning — while `intent.md` and the runtime dirs
//! `.cache`/`.reports` always survive, and pruning never needs a model.

// rationale: integration scenario file; many small hermetic tests naturally accrete past the 250-line soft tier.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use ctx_scan::reconcile::find_orphan_artifacts;
use ctx_scan::runner::{check_run, prune_run, update_run};
use ctx_summarize::agent::Agent;
use ctx_summarize::error::SummError;
use ctx_summarize::fs::Fs;

// ── in-memory fake ───────────────────────────────────────────────────────────

/// In-memory filesystem; reconciliation only lists, so `list_dir` is
/// the load-bearing member.
struct FakeFs {
    /// path → contents.
    map: RefCell<BTreeMap<String, String>>,
}

/// Build a [`FakeFs`] holding `paths`, each with dummy contents.
fn fake(paths: &[&str]) -> FakeFs {
    let map = paths
        .iter()
        .map(|p| ((*p).to_owned(), "x".to_owned()))
        .collect();
    FakeFs {
        map: RefCell::new(map),
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

/// Minimal counting agent, used only to seed real-fs fixtures.
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
    std::env::temp_dir().join(format!("ctx-scan-reconcile-{label}"))
}

/// Absolute path to the workspace prompt files, independent of cwd.
fn prompts_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../prompts")
        .to_string_lossy()
        .into_owned()
}

// ── hermetic tests ────────────────────────────────────────────────────────────

#[test]
fn find_flags_artifacts_of_deleted_and_descoped_sources() {
    // `gen/out.rs` still exists but is absent from the walker's target
    // list (scoped out); `src/old.rs` was deleted outright. Both kinds
    // of orphan reduce to "not in targets" — the walker is the single
    // scope source of truth.
    let fs = fake(&[
        "src/lib.rs",
        "gen/out.rs",
        ".context/rollup.ctx",
        ".context/hashes.json",
        ".context/src/lib.rs.ctx",
        ".context/src/old.rs.ctx",
        ".context/src/rollup.ctx",
        ".context/src/hashes.json",
        ".context/gen/out.rs.ctx",
        ".context/gen/rollup.ctx",
        ".context/gen/hashes.json",
    ]);
    let scan = find_orphan_artifacts(&fs, &["src/lib.rs".to_owned()]).expect("find");
    assert_eq!(
        scan.artifacts,
        vec![
            ".context/gen/hashes.json",
            ".context/gen/out.rs.ctx",
            ".context/gen/rollup.ctx",
            ".context/src/old.rs.ctx",
        ]
    );
}

#[test]
fn find_flags_whole_mirror_subtree_of_deleted_directory() {
    let fs = fake(&[
        "src/lib.rs",
        ".context/src/lib.rs.ctx",
        ".context/old/rollup.ctx",
        ".context/old/hashes.json",
        ".context/old/a.rs.ctx",
        ".context/old/deep/rollup.ctx",
        ".context/old/deep/b.rs.ctx",
    ]);
    let scan = find_orphan_artifacts(&fs, &["src/lib.rs".to_owned()]).expect("find");
    assert_eq!(scan.artifacts.len(), 5, "got: {:?}", scan.artifacts);
    assert!(scan
        .artifacts
        .iter()
        .all(|a| a.starts_with(".context/old/")));
}

#[test]
fn find_spares_intent_readme_and_runtime_dirs() {
    let fs = fake(&[
        ".context/README.md",
        ".context/intent.md",
        ".context/gone/intent.md",
        ".context/gone/rollup.ctx",
        ".context/.cache/task.ctx",
        ".context/.cache/rollup.ctx",
        ".context/.reports/report.rs.ctx",
    ]);
    let scan = find_orphan_artifacts(&fs, &[]).expect("find");
    assert_eq!(scan.artifacts, vec![".context/gone/rollup.ctx"]);
}

// ── real-filesystem tests ─────────────────────────────────────────────────────

/// Fixture seeded into a fresh summary tree: `keep.rs` stays alive for
/// the whole test; `top.rs` and `sub/` are deletion fodder. A test
/// helper, so it returns `Result` rather than unwrapping.
fn seeded_fixture(label: &str) -> Result<(PathBuf, StubAgent), Box<dyn std::error::Error>> {
    let base = test_dir(label);
    drop(std::fs::remove_dir_all(&base));
    std::fs::create_dir_all(base.join("sub"))?;
    std::fs::write(base.join("keep.rs"), "fn keep() {}")?;
    std::fs::write(base.join("top.rs"), "fn top() {}")?;
    std::fs::write(base.join("sub/x.rs"), "fn x() {}")?;
    let agent = StubAgent {
        calls: RefCell::new(0),
    };
    update_run(&base, &prompts_path(), &agent, false)?;
    Ok((base, agent))
}

#[test]
fn check_reports_orphans_read_only_then_prune_removes_them() {
    let (base, _) = seeded_fixture("subtree").expect("fixture");
    std::fs::remove_file(base.join("top.rs")).expect("delete file");
    std::fs::remove_dir_all(base.join("sub")).expect("delete dir");

    let report = check_run(&base).expect("check");
    // Deleted-file leaf: caught by the hash diff, deduplicated out of
    // orphan_artifacts. Deleted-directory subtree: invisible to the
    // diff (its sidecar is never loaded), caught by reconciliation.
    assert_eq!(report.orphan_leaves, vec![".context/top.rs.ctx".to_owned()]);
    assert_eq!(
        report.orphan_artifacts,
        vec![
            ".context/sub/hashes.json",
            ".context/sub/rollup.ctx",
            ".context/sub/x.rs.ctx",
        ]
    );
    // --check is read-only: everything it flagged is still on disk.
    assert!(base.join(".context/top.rs.ctx").is_file());
    assert!(base.join(".context/sub/rollup.ctx").is_file());

    let pruned = prune_run(&base).expect("prune");
    assert_eq!(pruned.len(), 4, "got: {pruned:?}");
    assert!(!base.join(".context/top.rs.ctx").exists());
    assert!(!base.join(".context/sub").exists(), "emptied dir swept");
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn prune_spares_intent_and_runtime_state() {
    let (base, _) = seeded_fixture("spare").expect("fixture");
    std::fs::write(base.join(".context/sub/intent.md"), "owner notes").expect("intent");
    std::fs::create_dir_all(base.join(".context/.cache")).expect("cache dir");
    std::fs::write(base.join(".context/.cache/tmp.rs.ctx"), "cache").expect("cache file");
    std::fs::create_dir_all(base.join(".context/.reports")).expect("reports dir");
    std::fs::write(base.join(".context/.reports/r.txt"), "report").expect("report file");
    std::fs::remove_dir_all(base.join("sub")).expect("delete dir");

    prune_run(&base).expect("prune");
    assert!(!base.join(".context/sub/rollup.ctx").exists());
    assert!(!base.join(".context/sub/x.rs.ctx").exists());
    assert!(
        base.join(".context/sub/intent.md").is_file(),
        "owner-authored intent.md must survive pruning"
    );
    assert!(base.join(".context/.cache/tmp.rs.ctx").is_file());
    assert!(base.join(".context/.reports/r.txt").is_file());
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn update_prunes_planted_orphan_without_model_when_otherwise_fresh() {
    let (base, agent) = seeded_fixture("no-model").expect("fixture");
    let seed_calls = *agent.calls.borrow();
    // A leftover artifact for a source that never existed: the tree is
    // hash-fresh, so pruning must happen without any model call.
    std::fs::write(base.join(".context/bogus.rs.ctx"), "stale").expect("plant");

    let report = update_run(&base, &prompts_path(), &agent, false).expect("update");
    assert_eq!(report.orphan_artifacts, vec![".context/bogus.rs.ctx"]);
    assert!(!report.needs_regeneration());
    assert_eq!(*agent.calls.borrow(), seed_calls, "pruning must not bill");
    assert!(!base.join(".context/bogus.rs.ctx").exists());
    assert!(check_run(&base).expect("recheck").is_fresh());
    drop(std::fs::remove_dir_all(&base));
}
