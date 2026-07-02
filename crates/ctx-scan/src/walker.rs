//! Recursive directory walker with access-gate and scope pre-filtering.
//!
//! Summarization scope is defined by `.ctxignore` (gitignore syntax) when
//! present, else seeded from the repo's `.gitignore`, and always includes
//! a built-in `target/` exclusion. The filter is git-independent by
//! design: freshness must never couple to commit state, and gitignored
//! files stay out of scope even in a non-git directory. Secrets and
//! binaries are pre-filtered via `ctx_core` (that deny always applies and
//! cannot be un-ignored) so the LLM is never called on inaccessible
//! files.

use std::fs;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::error::ScanError;

/// Build a [`ScanError::Walk`] for `path` from any displayable cause.
fn walk_err(path: &str, detail: &dyn std::fmt::Display) -> ScanError {
    ScanError::Walk {
        path: path.to_owned(),
        detail: detail.to_string(),
    }
}

/// Build the scope matcher for `base`: built-in defaults, then
/// `.ctxignore` when present, else `.gitignore` when present.
fn scope_matcher(base: &Path) -> Result<Gitignore, ScanError> {
    let mut b = GitignoreBuilder::new(base);
    b.add_line(None, "target/")
        .map_err(|e| walk_err("<builtin-scope>", &e))?;
    let ctxignore = base.join(".ctxignore");
    let gitignore = base.join(".gitignore");
    let seed = if ctxignore.is_file() {
        Some(ctxignore)
    } else if gitignore.is_file() {
        Some(gitignore)
    } else {
        None
    };
    if let Some(file) = seed {
        if let Some(e) = b.add(&file) {
            return Err(walk_err(&file.to_string_lossy(), &e));
        }
    }
    b.build().map_err(|e| walk_err("<scope-matcher>", &e))
}

/// Convert an absolute `path` to a dir-relative string using `base` as the root.
fn rel_path(base: &Path, path: &Path) -> Result<String, ScanError> {
    path.strip_prefix(base)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| walk_err(&base.to_string_lossy(), &e))
}

/// Whether file `rel` is in the summarization target set.
fn is_allowed(matcher: &Gitignore, rel: &str) -> bool {
    !ctx_core::access::is_secret(rel)
        && !ctx_core::access::is_binary(rel)
        && !matcher.matched(rel, false).is_ignore()
}

/// Process one directory entry: push non-ignored dirs onto `stack`,
/// collect allowed files into `out`.
fn visit(
    base: &Path,
    matcher: &Gitignore,
    stack: &mut Vec<PathBuf>,
    out: &mut Vec<String>,
    entry: &fs::DirEntry,
) -> Result<(), ScanError> {
    let name = entry.file_name().to_string_lossy().into_owned();
    let path = entry.path();
    if path.is_dir() {
        let rel = rel_path(base, &path)?;
        if name != ".context" && name != ".git" && !matcher.matched(&rel, true).is_ignore() {
            stack.push(path);
        }
        return Ok(());
    }
    if !path.is_file() {
        return Ok(());
    }
    let rel = rel_path(base, &path)?;
    if is_allowed(matcher, &rel) {
        out.push(rel);
    }
    Ok(())
}

/// Collect and process entries for one directory level.
fn read_level(
    base: &Path,
    matcher: &Gitignore,
    dir: &Path,
    stack: &mut Vec<PathBuf>,
    out: &mut Vec<String>,
) -> Result<(), ScanError> {
    let dir_str = dir.to_string_lossy().into_owned();
    let entries = fs::read_dir(dir).map_err(|e| walk_err(&dir_str, &e))?;
    for raw in entries {
        let entry = raw.map_err(|e| walk_err(&dir_str, &e))?;
        visit(base, matcher, stack, out, &entry)?;
    }
    Ok(())
}

/// Walk `base` recursively, returning dir-relative paths of summarizable
/// files.
///
/// Excludes `.context/` and `.git/` subtrees, secrets, binaries, and
/// everything matched by the scope filter (`.ctxignore`, else
/// `.gitignore`, plus built-in defaults). Results are sorted.
///
/// # Errors
///
/// [`ScanError::Walk`] if the scope filter cannot be built or a directory
/// cannot be read.
pub fn walk_dir(base: &Path) -> Result<Vec<String>, ScanError> {
    let matcher = scope_matcher(base)?;
    let mut result = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        read_level(base, &matcher, &dir, &mut stack, &mut result)?;
    }
    result.sort();
    Ok(result)
}
