//! Behavioral tests for chain resolution and rendering over an
//! in-memory `Env`.
//!
//! No real filesystem: a `FakeEnv` backs every operation so the tests
//! are hermetic and assert the protocol (chain order, directory
//! targets, absent markers). Hook/session behavior is covered in
//! `hook_session.rs`.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ctx_context::env::Env;
use ctx_context::error::CtxError;
use ctx_context::repo_path::RepoPath;
use ctx_context::serve;

/// In-memory environment: a flat path -> bytes map. Directories are
/// implied by key prefixes.
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
            (".context/crates/foo/rollup.ctx", "foo rollup"),
            (".context/crates/foo/intent.md", "foo intent"),
            (".context/crates/foo/bar.rs.ctx", "bar leaf"),
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
            .ok_or_else(|| CtxError::Io {
                path: path.as_string(),
                detail: "missing".to_owned(),
            })
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

    fn is_dir(&self, path: &RepoPath) -> bool {
        let prefix = format!("{}/", path.as_string());
        self.files.borrow().keys().any(|k| k.starts_with(&prefix))
    }

    fn env_var(&self, _key: &str) -> Option<String> {
        None
    }
}

#[test]
fn file_chain_is_rollup_intent_per_level_then_leaf_no_source() {
    let env = FakeEnv::seeded();
    let nodes = serve::chain_for(&env, "crates/foo/bar.rs").expect("chain");
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
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
        ]
    );
    assert!(ids.iter().all(|id| *id != "crates/foo/bar.rs"));
}

#[test]
fn directory_target_serves_its_own_rollup_and_intent() {
    let env = FakeEnv::seeded();
    let nodes = serve::chain_for(&env, "crates/foo/").expect("chain");
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            ".context/rollup.ctx",
            ".context/intent.md",
            ".context/crates/rollup.ctx",
            ".context/crates/intent.md",
            ".context/crates/foo/rollup.ctx",
            ".context/crates/foo/intent.md",
        ]
    );
}

#[test]
fn repo_root_target_serves_root_level_only() {
    let env = FakeEnv::seeded();
    let nodes = serve::chain_for(&env, ".").expect("chain");
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec![".context/rollup.ctx", ".context/intent.md"]);
}

#[test]
fn absent_scaffolding_is_soft_marked_not_an_error() {
    let env = FakeEnv::seeded();
    let nodes = serve::chain_for(&env, "crates/foo/baz.rs").expect("chain");
    let crates_intent = nodes
        .iter()
        .find(|n| n.id == ".context/crates/intent.md")
        .expect("crates intent node");
    assert!(!crates_intent.present);
    assert_eq!(crates_intent.body, "(absent: no intent at this level)");
    let leaf = nodes.last().expect("leaf node");
    assert!(!leaf.present);
    assert!(leaf.body.contains("no leaf"));
}

#[test]
fn path_escape_is_rejected() {
    let env = FakeEnv::seeded();
    for bad in ["/etc/passwd", "../x", "a/../b", "a//b"] {
        let r = serve::chain_for(&env, bad);
        assert!(
            matches!(r, Err(CtxError::PathEscape(_))),
            "{bad} should be rejected"
        );
    }
}

#[test]
fn render_labels_every_section() {
    let env = FakeEnv::seeded();
    let nodes = serve::chain_for(&env, ".").expect("chain");
    let text = serve::render(&nodes);
    assert!(text.contains("=== .context/rollup.ctx [rollup] ===\nroot rollup\n"));
    assert!(text.contains("=== .context/intent.md [intent] ===\nroot intent\n"));
}
