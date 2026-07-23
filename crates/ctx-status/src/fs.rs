//! Filesystem boundary. The runner is pure over [`Fs`]; the real
//! implementation touches `std::fs`, tests use an in-memory fake.
//!
//! Paths are resolved relative to the process working directory (the tool
//! repo) — `ctx-status` operates on the repo it is run from, unlike
//! `ctx-brief`'s cross-repo `--target`.

use std::fs;
use std::path::PathBuf;

use crate::error::StatusError;

/// Abstract filesystem rooted at the process working directory.
pub trait Fs {
    /// Read a file as a UTF-8 (lossy) string.
    ///
    /// # Errors
    ///
    /// [`StatusError::Io`] if the file cannot be read.
    fn read(&self, rel: &str) -> Result<String, StatusError>;

    /// Write a string to a file, creating parent directories.
    ///
    /// # Errors
    ///
    /// [`StatusError::Io`] if the file or its parents cannot be written.
    fn write(&self, rel: &str, contents: &str) -> Result<(), StatusError>;

    /// Whether a path exists.
    fn exists(&self, rel: &str) -> bool;
}

/// Real filesystem rooted at an absolute base path.
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

    /// Resolve a relative path against the base (an absolute `rel`
    /// replaces the base, matching `PathBuf::join` semantics).
    fn at(&self, rel: &str) -> PathBuf {
        self.base.join(rel)
    }
}

impl Fs for StdFs {
    fn read(&self, rel: &str) -> Result<String, StatusError> {
        fs::read(self.at(rel))
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .map_err(|e| StatusError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            })
    }

    fn write(&self, rel: &str, contents: &str) -> Result<(), StatusError> {
        let p = self.at(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| StatusError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            })?;
        }
        fs::write(&p, contents).map_err(|e| StatusError::Io {
            path: rel.to_owned(),
            detail: e.to_string(),
        })
    }

    fn exists(&self, rel: &str) -> bool {
        self.at(rel).exists()
    }
}
