//! Repo-relative path validation and `.context` mirror mapping.
//!
//! NOTE: this duplicates ~20 lines of the path-safety + `.context`
//! mapping that also lives in `ctx-access` (`repo_path`/`chain`). The
//! runner is deliberately kept independent of the access-broker crate;
//! the shared logic should move to a future `ctx-core` crate (recorded in
//! `docs/UNIMPLEMENTED.md`). Until then this copy is intentionally small
//! and identical in spirit.

use crate::error::SummError;

/// Reject absolute paths and any `.`/`..`/empty component.
///
/// # Errors
///
/// [`SummError::PathEscape`] for an unsafe or empty path.
pub fn validate_rel(raw: &str) -> Result<(), SummError> {
    if raw.is_empty() || raw.starts_with('/') {
        return Err(SummError::PathEscape(raw.to_owned()));
    }
    for part in raw.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains('\0') {
            return Err(SummError::PathEscape(raw.to_owned()));
        }
    }
    Ok(())
}

/// The `.context`-mirrored directory for a repo directory (`""` = root).
#[must_use]
pub fn context_dir_of(dir: &str) -> String {
    if dir.is_empty() {
        ".context".to_owned()
    } else {
        format!(".context/{dir}")
    }
}

/// `.context/<src>.ctx` — the leaf summary path for a source file.
#[must_use]
pub fn leaf_ctx(src: &str) -> String {
    format!(".context/{src}.ctx")
}

/// `.context/<dir>/rollup.ctx`.
#[must_use]
pub fn rollup_of(dir: &str) -> String {
    format!("{}/rollup.ctx", context_dir_of(dir))
}

/// `.context/<dir>/intent.md`.
#[must_use]
pub fn intent_of(dir: &str) -> String {
    format!("{}/intent.md", context_dir_of(dir))
}

/// The parent directory of a source path (`""` when at repo root).
#[must_use]
pub fn dir_of(src: &str) -> String {
    src.rsplit_once('/')
        .map_or_else(String::new, |(d, _)| d.to_owned())
}

/// Directory prefixes from repo root down to `src`'s parent.
///
/// For `a/b/c.rs` -> `["", "a", "a/b"]`. These are the directories whose
/// rollups a change to `src` can affect.
#[must_use]
pub fn ancestor_dirs(src: &str) -> Vec<String> {
    let mut dirs = vec![String::new()];
    let parent = dir_of(src);
    if parent.is_empty() {
        return dirs;
    }
    let mut acc = String::new();
    for seg in parent.split('/') {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(seg);
        dirs.push(acc.clone());
    }
    dirs
}
