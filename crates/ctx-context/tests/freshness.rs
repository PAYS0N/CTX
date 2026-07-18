//! Serve-time freshness markers over an in-memory `Env`.
//!
//! Pins the two degraded-tree cases the serving path must distinguish:
//! a present summary whose source has changed since the recorded hash
//! (STALE), and a directory/file with source but no summary on record
//! (NEVER GENERATED) — the latter kept distinct from the plain
//! `(absent: …)` marker used for legitimately sparse `intent.md`.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ctx_context::env::Env;
use ctx_context::error::CtxError;
use ctx_context::freshness::Assessment;
use ctx_context::repo_path::RepoPath;
use ctx_context::serve;
use ctx_core::hashtree::hex_hash;

/// In-memory environment: a flat path -> bytes map; directories are
/// implied by key prefixes (matching the main crate's `is_dir`).
struct FakeEnv {
    /// Backing store keyed by repo-relative path string.
    files: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl FakeEnv {
    /// Build from `(path, contents)` pairs.
    fn from(pairs: &[(&str, &str)]) -> Self {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert((*k).to_owned(), v.as_bytes().to_vec());
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

/// A `hashes.json` body recording one file child at `hash`.
fn sidecar(child: &str, hash: &str) -> String {
    format!("{{\"hash\":\"agg\",\"children\":{{\"{child}\":\"f:{hash}\"}}}}")
}

/// Find the served node with the given `.context` id (a test helper, so
/// it returns `Option` rather than unwrapping).
fn node<'a>(nodes: &'a [serve::ServedNode], id: &str) -> Option<&'a serve::ServedNode> {
    nodes.iter().find(|n| n.id == id)
}

#[test]
fn leaf_is_stale_when_source_outpaces_recorded_hash() {
    // Sidecar records an old hash; the source now differs.
    let env = FakeEnv::from(&[
        (".context/rollup.ctx", "root rollup"),
        (".context/crates/rollup.ctx", "crates rollup"),
        (".context/crates/foo/rollup.ctx", "foo rollup"),
        (".context/crates/foo/bar.rs.ctx", "bar leaf (from v1)"),
        (
            ".context/crates/foo/hashes.json",
            &sidecar(
                "bar.rs",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
        ),
        ("crates/foo/bar.rs", "fn bar() { /* edited to v2 */ }"),
    ]);
    let nodes = serve::chain_for(&env, "crates/foo/bar.rs").expect("chain");
    let leaf = node(&nodes, ".context/crates/foo/bar.rs.ctx").expect("leaf");
    assert_eq!(leaf.freshness, Assessment::Stale);
    assert!(serve::render(&nodes).contains("[STALE"));
}

#[test]
fn leaf_is_fresh_when_source_matches_recorded_hash() {
    let src = "fn bar() {}";
    let env = FakeEnv::from(&[
        (".context/rollup.ctx", "root rollup"),
        (".context/crates/rollup.ctx", "crates rollup"),
        (".context/crates/foo/rollup.ctx", "foo rollup"),
        (".context/crates/foo/bar.rs.ctx", "bar leaf"),
        (
            ".context/crates/foo/hashes.json",
            &sidecar("bar.rs", &hex_hash(src.as_bytes())),
        ),
        ("crates/foo/bar.rs", src),
    ]);
    let nodes = serve::chain_for(&env, "crates/foo/bar.rs").expect("chain");
    let leaf = node(&nodes, ".context/crates/foo/bar.rs.ctx").expect("leaf");
    assert_eq!(leaf.freshness, Assessment::Fresh);
    assert!(!serve::render(&nodes).contains("[STALE"));
    assert!(!serve::render(&nodes).contains("[NEVER GENERATED"));
}

#[test]
fn never_generated_is_distinct_from_absent_intent_and_from_stale() {
    // A new crate with source but no rollup/leaf/sidecar anywhere.
    let env = FakeEnv::from(&[
        (".context/rollup.ctx", "root rollup"),
        (".context/crates/rollup.ctx", "crates rollup"),
        ("crates/newcrate/src/lib.rs", "pub fn x() {}"),
    ]);
    let nodes = serve::chain_for(&env, "crates/newcrate/src/lib.rs").expect("chain");

    // Source exists but no rollup / no leaf on record -> NeverGenerated.
    let dir_rollup = node(&nodes, ".context/crates/newcrate/rollup.ctx").expect("rollup");
    assert_eq!(dir_rollup.freshness, Assessment::NeverGenerated);
    let leaf = node(&nodes, ".context/crates/newcrate/src/lib.rs.ctx").expect("leaf");
    assert_eq!(leaf.freshness, Assessment::NeverGenerated);

    // The new directory's intent is *legitimately* sparse: plain absent,
    // NOT a freshness marker — this is the distinction that must hold.
    let intent = node(&nodes, ".context/crates/newcrate/intent.md").expect("intent");
    assert_eq!(intent.freshness, Assessment::Unknown);

    let text = serve::render(&nodes);
    assert!(text.contains("[NEVER GENERATED"));
    assert!(!text.contains("[STALE"));
    // The sparse intent renders with the plain absent marker, unmarked.
    assert!(text.contains("=== .context/crates/newcrate/intent.md [intent] ===\n(absent:"));
}
