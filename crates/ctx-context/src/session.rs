//! Per-session served-node record for hook deduplication.
//!
//! The hook fires on every Read/Grep/Glob; without a record the same
//! chain would be re-injected on every tool call. State is one
//! gitignored JSON file per Claude session under `.context/.cache/`.
//! Loading is deliberately forgiving: a missing or corrupt file reads as
//! "nothing served yet" — the worst outcome is a re-injection, never a
//! block (the hook is fail-open by owner decision).

use std::collections::BTreeSet;

use crate::env::Env;
use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// Environment variable the harness sets to the current session id.
///
/// CLI path mode reads this to fold manually-served node ids into the
/// same per-session ledger the `--hook` mode maintains, so a node shown
/// by one path is not re-shown by the other.
pub const ENV_SESSION_ID: &str = "CLAUDE_CODE_SESSION_ID";

/// State-file path for `session`, its id sanitized to `[A-Za-z0-9._-]`.
fn state_path(session: &str) -> RepoPath {
    let safe: String = session
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    RepoPath::root()
        .child(".context")
        .child(".cache")
        .child(&format!("hook-{safe}.json"))
}

/// Node ids already injected this session.
#[must_use]
pub fn load<E: Env>(env: &E, session: &str) -> BTreeSet<String> {
    let path = state_path(session);
    if !env.exists(&path) {
        return BTreeSet::new();
    }
    env.read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Persist the served set for `session`.
///
/// # Errors
///
/// [`CtxError::Io`] if the state file cannot be written or encoded.
pub fn save<E: Env>(env: &E, session: &str, served: &BTreeSet<String>) -> Result<(), CtxError> {
    let bytes = serde_json::to_vec(served).map_err(|e| CtxError::Io {
        path: "<session-state>".to_owned(),
        detail: e.to_string(),
    })?;
    env.write(&state_path(session), &bytes)
}
