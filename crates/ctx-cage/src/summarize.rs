//! Auto-summarize integration.
//!
//! Pre-start: detect tracked sources whose `.context/<path>.ctx` leaf
//! is absent and feed them to `ctx-summarize paths …`. Post-stop:
//! `ctx-summarize from-cache --task-id <id>` refreshes whatever the
//! agent wrote during the run. Both are **billed** model operations
//! and only invoked under the spend gate.
//!
//! The pure piece is [`compute_stale`] + [`expected_leaf_path`]; the
//! impure wrappers [`list_tracked_sources`]/[`list_tracked_leaves`]
//! shell `git ls-files`, and [`run_summarize_paths`]/
//! [`run_summarize_from_cache`] shell `ctx-summarize`.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::CageError;

/// Repo-relative path of the `.context` leaf summary for `source`.
///
/// Mapping: `crates/x/src/foo.rs` ⇒ `.context/crates/x/src/foo.rs.ctx`
/// (the leaf is the source path prefixed with `.context/` and
/// suffixed with `.ctx`).
#[must_use]
pub fn expected_leaf_path(source: &Path) -> PathBuf {
    let Some(name) = source.file_name() else {
        return PathBuf::from(".context");
    };
    let mut leaf_name = name.to_owned();
    leaf_name.push(".ctx");
    let parent = source.parent().unwrap_or_else(|| Path::new(""));
    Path::new(".context").join(parent).join(leaf_name)
}

/// Sources whose corresponding `.ctx` leaf is missing from `leaves`.
/// Pure: testable without touching git or the filesystem.
#[must_use]
pub fn compute_stale(sources: &[PathBuf], leaves: &[PathBuf]) -> Vec<PathBuf> {
    let have: HashSet<&Path> = leaves.iter().map(PathBuf::as_path).collect();
    sources
        .iter()
        .filter(|src| !have.contains(expected_leaf_path(src).as_path()))
        .cloned()
        .collect()
}

/// Tracked source files under `target_root/crates/*/{src,tests}` via
/// `git ls-files`. The pathspec `*` in git matches across `/`, so a
/// single `crates/*/src/*.rs` covers nested module files too.
///
/// # Errors
///
/// [`CageError::Io`] on subprocess failure;
/// [`CageError::Protocol`] if `git ls-files` exits non-zero.
pub fn list_tracked_sources(target_root: &Path) -> Result<Vec<PathBuf>, CageError> {
    run_git_ls(target_root, &["crates/*/src/*.rs", "crates/*/tests/*.rs"])
}

/// Tracked `.ctx` files anywhere in the repo (we only consume the
/// `.context/` ones, but `git ls-files -- '*.ctx'` is the cheapest
/// pathspec that always covers them).
///
/// # Errors
///
/// See [`list_tracked_sources`].
pub fn list_tracked_leaves(target_root: &Path) -> Result<Vec<PathBuf>, CageError> {
    run_git_ls(target_root, &["*.ctx"])
}

/// Shell `git ls-files -z -- <pathspecs>` and parse the NUL-separated
/// output into `PathBuf`s.
fn run_git_ls(target_root: &Path, pathspecs: &[&str]) -> Result<Vec<PathBuf>, CageError> {
    let mut cmd = Command::new("git");
    cmd.args(["ls-files", "-z", "--"]).args(pathspecs);
    cmd.current_dir(target_root);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(CageError::Protocol(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|bytes| PathBuf::from(OsStr::from_bytes(bytes)))
        .collect())
}

/// `ctx-summarize paths <paths…>` in `target_root`. No-op for an
/// empty `paths` list. Inherits stdio so the user sees progress.
///
/// # Errors
///
/// [`CageError::Io`] on spawn failure;
/// [`CageError::Protocol`] on a non-zero exit.
pub fn run_summarize_paths(
    ctx_summarize_bin: &Path,
    target_root: &Path,
    paths: &[PathBuf],
) -> Result<(), CageError> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut cmd = Command::new(ctx_summarize_bin);
    cmd.arg("paths").current_dir(target_root);
    for p in paths {
        cmd.arg(p);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(CageError::Protocol(format!(
            "ctx-summarize paths failed: {status}"
        )));
    }
    Ok(())
}

/// `ctx-summarize from-cache --task-id <id>` to refresh leaves for
/// whatever the agent wrote during the run.
///
/// # Errors
///
/// See [`run_summarize_paths`].
pub fn run_summarize_from_cache(
    ctx_summarize_bin: &Path,
    target_root: &Path,
    task_id: &str,
) -> Result<(), CageError> {
    let status = Command::new(ctx_summarize_bin)
        .args(["from-cache", "--task-id", task_id])
        .current_dir(target_root)
        .status()?;
    if !status.success() {
        return Err(CageError::Protocol(format!(
            "ctx-summarize from-cache failed: {status}"
        )));
    }
    Ok(())
}
