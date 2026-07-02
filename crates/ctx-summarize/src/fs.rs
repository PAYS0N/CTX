//! Filesystem boundary. The runner is pure over [`Fs`]; the real impl
//! touches `std::fs`, tests use an in-memory fake.
//!
//! This module also owns [`scope_matcher`] — the single implementation
//! of the summarization scope (`.ctxignore`, gitignore syntax, falling
//! back to `.gitignore`, always seeded with `target/`). It is
//! git-independent by design: scope must not couple to commit state or
//! require a git repository. `ctx-scan`'s walker reuses it, so the
//! walker and the per-target deny gate cannot drift.

use std::fs;
use std::path::{Path, PathBuf};

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::error::SummError;

/// Build the summarization scope matcher for `base`: built-in defaults
/// plus `.ctxignore` when present.
///
/// `.ctxignore` is the ONLY scope file — `.gitignore` is never
/// consulted here; `ctx-scan` seeds a `.ctxignore` from it once, and
/// from then on the two are decoupled.
///
/// # Errors
///
/// [`SummError::Io`] if `.ctxignore` exists but cannot be parsed.
pub fn scope_matcher(base: &Path) -> Result<Gitignore, SummError> {
    let scope_err = |path: &str, detail: &dyn std::fmt::Display| SummError::Io {
        path: path.to_owned(),
        detail: detail.to_string(),
    };
    let mut b = GitignoreBuilder::new(base);
    b.add_line(None, "target/")
        .map_err(|e| scope_err("<builtin-scope>", &e))?;
    let ctxignore = base.join(".ctxignore");
    if ctxignore.is_file() {
        if let Some(e) = b.add(&ctxignore) {
            return Err(scope_err(&ctxignore.to_string_lossy(), &e));
        }
    }
    b.build().map_err(|e| scope_err("<scope-matcher>", &e))
}

/// Abstract filesystem rooted at the repository.
pub trait Fs {
    /// Read a file as a UTF-8 (lossy) string.
    ///
    /// # Errors
    ///
    /// [`SummError::Io`] if the file cannot be read.
    fn read(&self, rel: &str) -> Result<String, SummError>;

    /// Write a string to a file, creating parent directories.
    ///
    /// # Errors
    ///
    /// [`SummError::Io`] if the file or its parents cannot be written.
    fn write(&self, rel: &str, contents: &str) -> Result<(), SummError>;

    /// Whether a path exists.
    fn exists(&self, rel: &str) -> bool;

    /// Sorted entry names directly within a directory (empty if absent).
    ///
    /// # Errors
    ///
    /// [`SummError::Io`] if the directory exists but cannot be listed.
    fn list_dir(&self, rel: &str) -> Result<Vec<String>, SummError>;

    /// Whether `rel` is outside the summarization scope — the access
    /// gate's ignore spine (`.ctxignore`, else `.gitignore`, plus
    /// built-in defaults; see [`scope_matcher`]).
    ///
    /// # Errors
    ///
    /// [`SummError::Io`] if ignore status cannot be determined.
    fn is_ignored(&self, rel: &str) -> Result<bool, SummError>;

    /// Remove a file. Removing a missing file is not an error.
    ///
    /// # Errors
    ///
    /// [`SummError::Io`] if removal fails for a reason other than absence.
    fn remove(&self, rel: &str) -> Result<(), SummError>;
}

/// Real filesystem rooted at an absolute repository path.
#[derive(Debug, Clone)]
pub struct StdFs {
    /// Absolute base the relative paths resolve against.
    base: PathBuf,
}

impl StdFs {
    /// Root the filesystem at `base`.
    #[must_use]
    pub const fn new(base: PathBuf) -> Self {
        Self { base }
    }

    /// Resolve a relative path against the base.
    fn at(&self, rel: &str) -> PathBuf {
        if rel.is_empty() {
            self.base.clone()
        } else {
            self.base.join(rel)
        }
    }
}

impl Fs for StdFs {
    fn read(&self, rel: &str) -> Result<String, SummError> {
        let p = self.at(rel);
        fs::read(&p)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .map_err(|e| SummError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            })
    }

    fn write(&self, rel: &str, contents: &str) -> Result<(), SummError> {
        let p = self.at(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| SummError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            })?;
        }
        fs::write(&p, contents).map_err(|e| SummError::Io {
            path: rel.to_owned(),
            detail: e.to_string(),
        })
    }

    fn exists(&self, rel: &str) -> bool {
        self.at(rel).exists()
    }

    fn list_dir(&self, rel: &str) -> Result<Vec<String>, SummError> {
        let p = self.at(rel);
        if !p.is_dir() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&p).map_err(|e| SummError::Io {
            path: rel.to_owned(),
            detail: e.to_string(),
        })? {
            let entry = entry.map_err(|e| SummError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            })?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }

    fn remove(&self, rel: &str) -> Result<(), SummError> {
        match fs::remove_file(self.at(rel)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SummError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            }),
        }
    }

    fn is_ignored(&self, rel: &str) -> Result<bool, SummError> {
        // Rebuilt per query: point lookups are rare (one per target)
        // and the ignore files are small; simplicity over caching.
        let matcher = scope_matcher(&self.base)?;
        Ok(matcher.matched_path_or_any_parents(rel, false).is_ignore())
    }
}
