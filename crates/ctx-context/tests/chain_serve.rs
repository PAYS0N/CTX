//! Behavioral tests for chain serving and hook injection over an
//! in-memory `Env`.
//!
//! No real filesystem: a `FakeEnv` backs every operation so the tests are
//! hermetic and assert the protocol (chain order, directory targets,
//! absent markers, session dedup, hook fail-open).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use ctx_context::env::Env;
use ctx_context::error::CtxError;
use ctx_context::repo_path::RepoPath;
use ctx_context::{hook, serve, session};

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

/// Build a `PostToolUse` event for a Read of `file_path`.
fn read_event(session: &str, file_path: &str) -> String {
    format!(
        r#"{{"session_id":"{session}","tool_name":"Read","tool_input":{{"file_path":"{file_path}"}}}}"#
    )
}

#[test]
fn hook_injects_chain_then_dedups_within_session() {
    let env = FakeEnv::seeded();
    let base = Path::new("/repo");
    let first = hook::run(&env, base, &read_event("s1", "/repo/crates/foo/bar.rs"));
    assert!(first.contains("additionalContext"));
    assert!(first.contains("foo rollup"));
    // Same session, sibling file: ancestors are deduped; only the new
    // leaf level would remain, and it is absent -> nothing to inject.
    let second = hook::run(&env, base, &read_event("s1", "/repo/crates/foo/baz.rs"));
    assert!(second.is_empty());
    // A different session starts fresh.
    let other = hook::run(&env, base, &read_event("s2", "/repo/crates/foo/bar.rs"));
    assert!(other.contains("foo rollup"));
}

#[test]
fn hook_is_silent_outside_the_repo_and_on_scaffolding() {
    let env = FakeEnv::seeded();
    let base = Path::new("/repo");
    assert!(hook::run(&env, base, &read_event("s", "/etc/passwd")).is_empty());
    assert!(hook::run(&env, base, &read_event("s", "/repo/.context/rollup.ctx")).is_empty());
    assert!(hook::run(&env, base, &read_event("s", "/repo/.git/config")).is_empty());
}

#[test]
fn hook_is_silent_on_unparseable_or_pathless_input() {
    let env = FakeEnv::seeded();
    let base = Path::new("/repo");
    assert!(hook::run(&env, base, "not json").is_empty());
    assert!(hook::run(&env, base, r#"{"session_id":"s","tool_input":{}}"#).is_empty());
    assert!(hook::run(&env, base, "{}").is_empty());
}

#[test]
fn event_cwd_comes_from_the_event_json() {
    assert_eq!(
        hook::event_cwd(r#"{"cwd":"/repo","tool_name":"Read"}"#),
        Some(std::path::PathBuf::from("/repo"))
    );
    assert!(hook::event_cwd(r#"{"tool_name":"Read"}"#).is_none());
    assert!(hook::event_cwd("not json").is_none());
}

#[test]
fn corrupt_session_state_reads_as_empty() {
    let env = FakeEnv::seeded();
    let state = RepoPath::parse(".context/.cache/hook-s9.json").expect("path");
    env.write(&state, b"not json").expect("seed corrupt state");
    assert!(session::load(&env, "s9").is_empty());
    // And the hook still injects (fail-open, worst case re-injection).
    let out = hook::run(
        &env,
        Path::new("/repo"),
        &read_event("s9", "/repo/crates/foo/bar.rs"),
    );
    assert!(out.contains("foo rollup"));
}
