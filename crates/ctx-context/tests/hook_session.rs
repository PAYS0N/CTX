//! Behavioral tests for hook injection, session dedup, and how CLI path
//! mode shares the hook's per-session ledger.
//!
//! No real filesystem: a `FakeEnv` backs every operation.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use clap::Parser;
use ctx_context::cli::{dispatch, Cli};
use ctx_context::env::Env;
use ctx_context::error::CtxError;
use ctx_context::repo_path::RepoPath;
use ctx_context::{hook, session};

/// In-memory environment: a flat path -> bytes map, plus settable env
/// vars. Directories are implied by key prefixes.
struct FakeEnv {
    /// Backing store keyed by repo-relative path string.
    files: RefCell<BTreeMap<String, Vec<u8>>>,
    /// Environment variables visible to `env_var`.
    vars: RefCell<BTreeMap<String, String>>,
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
            vars: RefCell::new(BTreeMap::new()),
        }
    }

    /// Set an environment variable this fake reports via `env_var`.
    fn set_var(&self, key: &str, value: &str) {
        self.vars
            .borrow_mut()
            .insert(key.to_owned(), value.to_owned());
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

    fn env_var(&self, key: &str) -> Option<String> {
        self.vars.borrow().get(key).cloned()
    }
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
fn path_mode_records_into_session_so_hook_skips_repeat() {
    let env = FakeEnv::seeded();
    env.set_var(session::ENV_SESSION_ID, "s1");
    let mut out = Vec::new();
    let cli = Cli::parse_from(["ctx-context", "crates/foo/bar.rs"]);
    dispatch(&env, Path::new("/repo"), cli, "", &mut out).expect("dispatch");
    assert!(String::from_utf8(out).expect("utf8").contains("foo rollup"));

    // Same session via the hook: everything the manual call already
    // served (root/crates/foo rollups+intents, the bar.rs leaf) must be
    // treated as already shown, not re-injected.
    let hooked = hook::run(
        &env,
        Path::new("/repo"),
        &read_event("s1", "/repo/crates/foo/bar.rs"),
    );
    assert!(hooked.is_empty());
}

#[test]
fn path_mode_without_session_env_leaves_no_session_state() {
    let env = FakeEnv::seeded();
    let mut out = Vec::new();
    let cli = Cli::parse_from(["ctx-context", "."]);
    dispatch(&env, Path::new("/repo"), cli, "", &mut out).expect("dispatch");
    assert!(session::load(&env, "s1").is_empty());
    let state = RepoPath::parse(".context/.cache/hook-s1.json").expect("path");
    assert!(!env.exists(&state));
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
