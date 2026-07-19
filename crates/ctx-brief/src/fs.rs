//! Filesystem boundary. The runner is pure over [`Fs`]; the real
//! implementation touches `std::fs`, tests use an in-memory fake.
//!
//! Paths are resolved relative to the process working directory (the tool
//! repo). The prompt files live there; the target repo's `docs/STATUS.md`
//! and the brief output are addressed by joining the (cwd-relative)
//! `--target` prefix, so a single root covers every read and write.

use std::fs;
use std::path::PathBuf;

use crate::error::BriefError;

/// Abstract filesystem rooted at the process working directory.
pub trait Fs {
    /// Read a file as a UTF-8 (lossy) string.
    ///
    /// # Errors
    ///
    /// [`BriefError::Io`] if the file cannot be read.
    fn read(&self, rel: &str) -> Result<String, BriefError>;

    /// Write a string to a file, creating parent directories.
    ///
    /// # Errors
    ///
    /// [`BriefError::Io`] if the file or its parents cannot be written.
    fn write(&self, rel: &str, contents: &str) -> Result<(), BriefError>;

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
    fn read(&self, rel: &str) -> Result<String, BriefError> {
        fs::read(self.at(rel))
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .map_err(|e| BriefError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            })
    }

    fn write(&self, rel: &str, contents: &str) -> Result<(), BriefError> {
        let p = self.at(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| BriefError::Io {
                path: rel.to_owned(),
                detail: e.to_string(),
            })?;
        }
        fs::write(&p, contents).map_err(|e| BriefError::Io {
            path: rel.to_owned(),
            detail: e.to_string(),
        })
    }

    fn exists(&self, rel: &str) -> bool {
        self.at(rel).exists()
    }
}
