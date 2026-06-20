//! Recursive directory walker with access-gate pre-filtering.
//!
//! Pre-filters secrets and binaries (pure name/extension rules via
//! `ctx_core`) and gitignored paths (via `git check-ignore`, with graceful
//! fallback for non-git directories) before handing paths to the runner,
//! so the LLM is never called on inaccessible files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::ScanError;

/// Whether `rel` is gitignored under `base`.
///
/// Returns `false` on any error, including when `base` is not a git repo.
fn git_is_ignored(base: &Path, rel: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(base)
        .args(["check-ignore", "-q", "--"])
        .arg(rel)
        .status()
        .is_ok_and(|s| s.code() == Some(0))
}

/// Convert an absolute `path` to a dir-relative string using `base` as the root.
fn rel_path(base: &Path, path: &Path) -> Result<String, ScanError> {
    path.strip_prefix(base)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| ScanError::Walk {
            path: base.to_string_lossy().into_owned(),
            detail: e.to_string(),
        })
}

/// Whether `rel` should be included in the summarization target set.
fn is_allowed(base: &Path, rel: &str) -> bool {
    !ctx_core::access::is_secret(rel)
        && !ctx_core::access::is_binary(rel)
        && !git_is_ignored(base, rel)
}

/// Process one directory entry: push dirs onto `stack`, collect files into `out`.
fn visit(
    base: &Path,
    stack: &mut Vec<PathBuf>,
    out: &mut Vec<String>,
    entry: &fs::DirEntry,
) -> Result<(), ScanError> {
    let name = entry.file_name().to_string_lossy().into_owned();
    let path = entry.path();
    if path.is_dir() {
        if name != ".context" {
            stack.push(path);
        }
        return Ok(());
    }
    if !path.is_file() {
        return Ok(());
    }
    let rel = rel_path(base, &path)?;
    if is_allowed(base, &rel) {
        out.push(rel);
    }
    Ok(())
}

/// Collect and process entries for one directory level.
fn read_level(
    base: &Path,
    dir: &Path,
    stack: &mut Vec<PathBuf>,
    out: &mut Vec<String>,
) -> Result<(), ScanError> {
    let dir_str = dir.to_string_lossy().into_owned();
    let entries = fs::read_dir(dir).map_err(|e| ScanError::Walk {
        path: dir_str,
        detail: e.to_string(),
    })?;
    for raw in entries {
        let entry = raw.map_err(|e| ScanError::Walk {
            path: dir.to_string_lossy().into_owned(),
            detail: e.to_string(),
        })?;
        visit(base, stack, out, &entry)?;
    }
    Ok(())
}

/// Walk `base` recursively, returning dir-relative paths of summarizable files.
///
/// Excludes `.context/` subtrees, secrets, binaries, and gitignored files.
/// Results are returned in sorted order.
///
/// # Errors
///
/// [`ScanError::Walk`] if a directory cannot be read or an entry is inaccessible.
pub fn walk_dir(base: &Path) -> Result<Vec<String>, ScanError> {
    let mut result = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        read_level(base, &dir, &mut stack, &mut result)?;
    }
    result.sort();
    Ok(result)
}
