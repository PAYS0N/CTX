//! The filesystem + clock boundary.
//!
//! [`enforce`](crate::enforce) is pure over the [`Env`] trait. The real
//! implementation touches `std::fs`; tests use an in-memory fake; a future
//! `ctx-broker` implementation talks over a socket. This trait is the seam
//! `docs/SANDBOX.md` mandates so the broker split is a transport swap, not
//! a rewrite.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// Abstract filesystem and clock used by all enforcement logic.
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

    /// Sorted entry names directly within a directory.
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] if the directory cannot be listed.
    fn list_dir(&self, path: &RepoPath) -> Result<Vec<String>, CtxError>;

    /// Remove a file. Removing a missing file is not an error.
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] if removal fails for a reason other than absence.
    fn remove(&self, path: &RepoPath) -> Result<(), CtxError>;

    /// Seconds since the Unix epoch.
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] if the system clock is before the Unix epoch.
    fn now_unix(&self) -> Result<u64, CtxError>;

    /// Repo-relative paths git tracks — the manifest's source set
    /// (gitignored/untracked paths, e.g. `.env`, are absent).
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] if the tracked set cannot be determined.
    fn tracked_files(&self) -> Result<Vec<String>, CtxError>;

    /// Whether `path` is gitignored — the access gate's deny spine
    /// (catches `.env`, `target/`, caches; a new untracked-but-not-ignored
    /// source file is NOT ignored and stays accessible).
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] if ignore status cannot be determined.
    fn is_ignored(&self, path: &RepoPath) -> Result<bool, CtxError>;
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

    fn list_dir(&self, path: &RepoPath) -> Result<Vec<String>, CtxError> {
        let abs = self.resolve(path);
        let mut names = Vec::new();
        for entry in fs::read_dir(&abs).map_err(|e| Self::io(&abs, &e))? {
            let entry = entry.map_err(|e| Self::io(&abs, &e))?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }

    fn remove(&self, path: &RepoPath) -> Result<(), CtxError> {
        let abs = self.resolve(path);
        match fs::remove_file(&abs) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Self::io(&abs, &e)),
        }
    }

    fn now_unix(&self) -> Result<u64, CtxError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| CtxError::Io {
                path: "<clock>".to_owned(),
                detail: e.to_string(),
            })
    }

    fn tracked_files(&self) -> Result<Vec<String>, CtxError> {
        let git = |d: String| CtxError::Io {
            path: "<git>".to_owned(),
            detail: d,
        };
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.base)
            .args(["ls-files", "-z"])
            .output()
            .map_err(|e| git(e.to_string()))?;
        if !out.status.success() {
            return Err(git(String::from_utf8_lossy(&out.stderr).into_owned()));
        }
        Ok(out
            .stdout
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect())
    }

    fn is_ignored(&self, path: &RepoPath) -> Result<bool, CtxError> {
        let rel = path.as_string();
        let status = Command::new("git")
            .arg("-C")
            .arg(&self.base)
            .args(["check-ignore", "-q", "--"])
            .arg(&rel)
            .status()
            .map_err(|e| CtxError::Io {
                path: "<git>".to_owned(),
                detail: e.to_string(),
            })?;
        // git check-ignore: 0 = ignored, 1 = not ignored, >1 = error.
        match status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            other => Err(CtxError::Io {
                path: rel,
                detail: format!("git check-ignore exit {other:?}"),
            }),
        }
    }
}
