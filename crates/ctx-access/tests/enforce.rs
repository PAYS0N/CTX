//! Behavioral tests for the enforcement core over an in-memory `Env`.
//!
//! No real filesystem: a `FakeEnv` backs every operation so the tests are
//! hermetic and assert the protocol (chain order, prefix caching, shallow
//! stop, stale banner, write-needs-read, list-needs-rollup, lifecycle).

use std::cell::RefCell;
use std::collections::BTreeMap;

use ctx_access::cache::TaskCache;
use ctx_access::enforce::{self, NoopSummarizer};
use ctx_access::env::Env;
use ctx_access::error::CtxError;
use ctx_access::repo_path::RepoPath;

/// In-memory environment: a flat path -> bytes map plus a fixed clock.
struct FakeEnv {
    /// Backing store keyed by repo-relative path string.
    files: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl FakeEnv {
    /// Seed a small context tree plus two sibling source files.
    fn seeded() -> Self {
        let mut m = BTreeMap::new();
        for (k, v) in [
            (".context/rollup.ctx", "root rollup"),
            (".context/intent.md", "root intent"),
            (".context/crates/rollup.ctx", "crates rollup"),
            (".context/crates/intent.md", "crates intent"),
            (".context/crates/foo/rollup.ctx", "foo rollup"),
            (".context/crates/foo/intent.md", "foo intent"),
            (".context/crates/foo/bar.rs.ctx", "bar leaf"),
            (".context/crates/foo/baz.rs.ctx", "baz leaf"),
            ("crates/foo/bar.rs", "fn bar() {}"),
            ("crates/foo/baz.rs", "fn baz() {}"),
        ] {
            m.insert(k.to_owned(), v.as_bytes().to_vec());
        }
        Self {
            files: RefCell::new(m),
        }
    }
}

impl Env for FakeEnv {
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, CtxError> {
        self.files
            .borrow()
            .get(&path.as_string())
            .cloned()
            .ok_or_else(|| CtxError::MissingNode(path.as_string()))
    }

    fn write(&self, path: &RepoPath, bytes: &[u8]) -> Result<(), CtxError> {
        self.files
            .borrow_mut()
            .insert(path.as_string(), bytes.to_vec());
        Ok(())
    }

    fn exists(&self, path: &RepoPath) -> bool {
        self.files.borrow().contains_key(&path.as_string())
    }

    fn list_dir(&self, path: &RepoPath) -> Result<Vec<String>, CtxError> {
        let dir = path.as_string();
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let mut names: Vec<String> = Vec::new();
        for key in self.files.borrow().keys() {
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

    fn remove(&self, path: &RepoPath) -> Result<(), CtxError> {
        self.files.borrow_mut().remove(&path.as_string());
        Ok(())
    }

    fn now_unix(&self) -> Result<u64, CtxError> {
        Ok(1_700_000_000)
    }
}

const TASK: &str = "task-1";

#[test]
fn init_is_idempotent_only_with_force() {
    let env = FakeEnv::seeded();
    enforce::init_task(&env, TASK, false).expect("first init");
    let again = enforce::init_task(&env, TASK, false);
    assert!(matches!(again, Err(CtxError::TaskExists(_))));
    enforce::init_task(&env, TASK, true).expect("forced reinit");
}

#[test]
fn read_requires_init() {
    let env = FakeEnv::seeded();
    let r = enforce::read(&env, TASK, "crates/foo/bar.rs", false);
    assert!(matches!(r, Err(CtxError::TaskMissing(_))));
}

#[test]
fn read_serves_full_chain_in_order_then_caches_prefix() {
    let env = FakeEnv::seeded();
    enforce::init_task(&env, TASK, false).expect("init");
    let first = enforce::read(&env, TASK, "crates/foo/bar.rs", false).expect("read bar");
    let ids: Vec<&str> = first.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            ".context/rollup.ctx",
            ".context/intent.md",
            ".context/crates/rollup.ctx",
            ".context/crates/intent.md",
            ".context/crates/foo/rollup.ctx",
            ".context/crates/foo/intent.md",
            ".context/crates/foo/bar.rs.ctx",
            "crates/foo/bar.rs",
        ]
    );
    // Re-reading the same path serves nothing (whole chain cached).
    let again = enforce::read(&env, TASK, "crates/foo/bar.rs", false).expect("reread");
    assert!(again.nodes.is_empty());
    // A sibling reuses the cached ancestors: only its leaf + source.
    let sib = enforce::read(&env, TASK, "crates/foo/baz.rs", false).expect("read baz");
    let sib_ids: Vec<&str> = sib.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        sib_ids,
        vec![".context/crates/foo/baz.rs.ctx", "crates/foo/baz.rs"]
    );
}

#[test]
fn shallow_stops_before_source() {
    let env = FakeEnv::seeded();
    enforce::init_task(&env, TASK, false).expect("init");
    let r = enforce::read(&env, TASK, "crates/foo/bar.rs", true).expect("shallow read");
    let last = r.nodes.last().expect("at least one node");
    assert_eq!(last.id, ".context/crates/foo/bar.rs.ctx");
    assert!(r.nodes.iter().all(|n| n.id != "crates/foo/bar.rs"));
}

// rationale: one linear protocol scenario (deny->read->write->reload->reread->banner); splitting across fns would fragment the sequence under test.
#[test]
fn write_needs_prior_nonshallow_read_then_banners_leaf_on_reread() {
    let env = FakeEnv::seeded();
    enforce::init_task(&env, TASK, false).expect("init");

    let denied = enforce::write(&env, TASK, "crates/foo/bar.rs", b"new");
    assert!(matches!(denied, Err(CtxError::WriteWithoutRead { .. })));

    enforce::read(&env, TASK, "crates/foo/bar.rs", false).expect("read");
    enforce::write(
        &env,
        TASK,
        "crates/foo/bar.rs",
        b"fn bar() { /* edited */ }",
    )
    .expect("write after read");

    let cache = TaskCache::load(&env, TASK).expect("cache");
    assert!(cache.has_written("crates/foo/bar.rs"));

    // Re-read in the SAME task: only the evicted leaf + source come back.
    let reread = enforce::read(&env, TASK, "crates/foo/bar.rs", false).expect("reread");
    let ids: Vec<&str> = reread.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![".context/crates/foo/bar.rs.ctx", "crates/foo/bar.rs"]
    );

    let leaf = reread.nodes.first().expect("leaf");
    assert!(leaf.body.starts_with("STALE — modified in current task"));
    let source = reread.nodes.last().expect("source");
    assert!(!source.body.contains("STALE"));
    assert!(source.body.contains("edited"));
}

#[test]
fn list_needs_rollup_served() {
    let env = FakeEnv::seeded();
    enforce::init_task(&env, TASK, false).expect("init");
    let denied = enforce::list(&env, TASK, "crates/foo");
    assert!(matches!(denied, Err(CtxError::ListWithoutRollup { .. })));

    enforce::read(&env, TASK, "crates/foo/bar.rs", false).expect("read serves foo rollup");
    let listed = enforce::list(&env, TASK, "crates/foo").expect("list ok");
    assert!(listed.contains(&"bar.rs".to_owned()));
    assert!(listed.contains(&"baz.rs".to_owned()));
}

#[test]
fn path_escape_is_rejected() {
    let env = FakeEnv::seeded();
    enforce::init_task(&env, TASK, false).expect("init");
    for bad in ["/etc/passwd", "../x", "a/../b", "a//b"] {
        let r = enforce::read(&env, TASK, bad, false);
        assert!(
            matches!(r, Err(CtxError::PathEscape(_))),
            "{bad} should be rejected"
        );
    }
}

#[test]
fn end_task_writes_report_and_clears_cache() {
    let env = FakeEnv::seeded();
    enforce::init_task(&env, TASK, false).expect("init");
    enforce::read(&env, TASK, "crates/foo/bar.rs", false).expect("read");
    let report = enforce::end_task(&env, TASK, &NoopSummarizer).expect("end");
    assert_eq!(report.task_id, TASK);
    assert!(report.divergences.is_empty());
    let report_path = RepoPath::parse(".context/.reports/task-1.json").expect("rp");
    assert!(env.exists(&report_path));
    // Cache gone: any further per-request command fails as uninitialized.
    let after = enforce::read(&env, TASK, "crates/foo/bar.rs", false);
    assert!(matches!(after, Err(CtxError::TaskMissing(_))));
}
