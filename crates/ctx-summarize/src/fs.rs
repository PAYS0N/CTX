//! Filesystem boundary. The runner is pure over [`Fs`]; the real impl
//! touches `std::fs`, tests use an in-memory fake.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::error::SummError;

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

    /// Whether `rel` is gitignored — the access gate's deny spine.
    ///
    /// # Errors
    ///
    /// [`SummError::Io`] if ignore status cannot be determined.
    fn is_ignored(&self, rel: &str) -> Result<bool, SummError>;
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

    fn is_ignored(&self, rel: &str) -> Result<bool, SummError> {
        let status = Command::new("git")
            .arg("-C")
            .arg(&self.base)
            .args(["check-ignore", "-q", "--"])
            .arg(rel)
            .status()
            .map_err(|e| SummError::Io {
                path: "<git>".to_owned(),
                detail: e.to_string(),
            })?;
        // git check-ignore: 0 = ignored, 1 = not ignored, >1 = error.
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            other => Err(SummError::Io {
                path: rel.to_owned(),
                detail: format!("git check-ignore exit {other:?}"),
            }),
        }
    }
}
