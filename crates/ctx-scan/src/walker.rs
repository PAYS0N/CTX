//! Recursive directory walker with access-gate and scope pre-filtering.
//!
//! Summarization scope is `.ctxignore` (gitignore syntax) — the ONLY
//! scope file. When it does not exist, the walker seeds one from the
//! repo's `.gitignore` (verbatim, under an explanatory header) exactly
//! once; after that hand-off `.gitignore` is never consulted, so scope
//! cannot silently track git state. A built-in `target/` exclusion
//! always applies, and secrets/binaries are pre-filtered via `ctx_core`
//! (that deny always applies and cannot be un-ignored) so the LLM is
//! never called on inaccessible files.

use std::fs;
use std::path::{Path, PathBuf};

use ctx_summarize::fs::scope_matcher;
use ignore::gitignore::Gitignore;

use crate::error::ScanError;

/// Build a [`ScanError::Walk`] for `path` from any displayable cause.
fn walk_err(path: &str, detail: &dyn std::fmt::Display) -> ScanError {
    ScanError::Walk {
        path: path.to_owned(),
        detail: detail.to_string(),
    }
}

/// Header written atop a seeded `.ctxignore`.
const SCOPE_HEADER: &str = "\
# .ctxignore — summarization scope for the ctx tools (gitignore syntax).
# Seeded once from .gitignore by ctx-scan; edit freely. This file is the
# ONLY scope source — .gitignore is never consulted after seeding.
# Always applied on top: target/ is excluded; secrets and binaries are
# denied and cannot be un-ignored here.

";

/// Ensure `.ctxignore` exists, seeding it once from `.gitignore`
/// (verbatim, under [`SCOPE_HEADER`]) or from the bare header when no
/// `.gitignore` exists. This is the one-time hand-off that decouples
/// scope from git state.
fn ensure_scope_file(base: &Path) -> Result<(), ScanError> {
    let path = base.join(".ctxignore");
    if path.is_file() {
        return Ok(());
    }
    let inherited = fs::read_to_string(base.join(".gitignore")).unwrap_or_default();
    fs::write(&path, format!("{SCOPE_HEADER}{inherited}")).map_err(|e| walk_err(".ctxignore", &e))
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
/// files. Seeds `.ctxignore` from `.gitignore` on first contact.
///
/// Excludes `.context/` and `.git/` subtrees, secrets, binaries, and
/// everything matched by `.ctxignore` (plus built-in defaults).
/// Results are sorted.
///
/// # Errors
///
/// [`ScanError::Walk`] if the scope file cannot be seeded or parsed, or
/// a directory cannot be read.
pub fn walk_dir(base: &Path) -> Result<Vec<String>, ScanError> {
    ensure_scope_file(base)?;
    let matcher = scope_matcher(base)?;
    walk_with(base, &matcher)
}

/// Walk with an already-built scope matcher.
fn walk_with(base: &Path, matcher: &Gitignore) -> Result<Vec<String>, ScanError> {
    let mut result = Vec::new();
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        read_level(base, matcher, &dir, &mut stack, &mut result)?;
    }
    result.sort();
    Ok(result)
}
