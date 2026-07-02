//! The filesystem boundary.
//!
//! [`serve`](crate::serve), [`session`](crate::session), and
//! [`hook`](crate::hook) are pure over the [`Env`] trait. The real
//! implementation touches `std::fs`; tests use an in-memory fake.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// Abstract filesystem used by the chain-serving logic.
pub trait Env {
    /// Read a file's bytes.
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] if the file cannot be read.
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, CtxError>;

    /// Write bytes to a file, creating parent directories as needed.
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] if the file or its parents cannot be written.
    fn write(&self, path: &RepoPath, bytes: &[u8]) -> Result<(), CtxError>;

    /// Whether a path currently exists.
    fn exists(&self, path: &RepoPath) -> bool;

    /// Whether a path is an existing directory.
    fn is_dir(&self, path: &RepoPath) -> bool;
}

/// Real implementation rooted at an absolute repository path.
#[derive(Debug, Clone)]
pub struct StdEnv {
    /// Absolute path the repo-relative [`RepoPath`]s resolve against.
    base: PathBuf,
}

impl StdEnv {
    /// Create an environment rooted at `base` (an absolute repo path).
    #[must_use]
    pub const fn new(base: PathBuf) -> Self {
        Self { base }
    }

    /// Resolve a [`RepoPath`] to an absolute filesystem path.
    fn resolve(&self, path: &RepoPath) -> PathBuf {
        let rel = path.as_string();
        if rel.is_empty() {
            self.base.clone()
        } else {
            self.base.join(rel)
        }
    }

    /// Build a [`CtxError::Io`] for `path` from a source error.
    fn io(path: &Path, detail: &dyn std::fmt::Display) -> CtxError {
        CtxError::Io {
            path: path.display().to_string(),
            detail: detail.to_string(),
        }
    }
}

impl Env for StdEnv {
    fn read(&self, path: &RepoPath) -> Result<Vec<u8>, CtxError> {
        let abs = self.resolve(path);
        fs::read(&abs).map_err(|e| Self::io(&abs, &e))
    }

    fn write(&self, path: &RepoPath, bytes: &[u8]) -> Result<(), CtxError> {
        let abs = self.resolve(path);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| Self::io(parent, &e))?;
        }
        fs::write(&abs, bytes).map_err(|e| Self::io(&abs, &e))
    }

    fn exists(&self, path: &RepoPath) -> bool {
        self.resolve(path).exists()
    }

    fn is_dir(&self, path: &RepoPath) -> bool {
        self.resolve(path).is_dir()
    }
}
