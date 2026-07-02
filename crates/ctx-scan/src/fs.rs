//! `ScanFs`: a [`StdFs`] wrapper that injects embedded prompt constants
//! and gracefully handles gitignore checks outside git repositories.

use std::path::PathBuf;

use ctx_summarize::error::SummError;
use ctx_summarize::fs::{Fs, StdFs};

/// Repo-relative path for the leaf-summarizer prompt (expected by runner).
const LEAF_PATH: &str = "prompts/summarizer-leaf.md";

/// Repo-relative path for the rollup-summarizer prompt (expected by runner).
const ROLLUP_PATH: &str = "prompts/summarizer-rollup.md";

/// Embedded leaf prompt, compiled in from the workspace prompts directory.
const LEAF_PROMPT: &str = include_str!("../../../prompts/summarizer-leaf.md");

/// Embedded rollup prompt, compiled in from the workspace prompts directory.
const ROLLUP_PROMPT: &str = include_str!("../../../prompts/summarizer-rollup.md");

/// Filesystem rooted at the scan target directory.
///
/// Intercepts reads at the two known prompt paths and returns the embedded
/// compile-time constants, so the binary works without a `prompts/`
/// directory present. All other operations delegate to the inner [`StdFs`].
/// Gitignore errors (e.g., not a git repo) are silently mapped to `false`.
pub struct ScanFs {
    /// Inner standard filesystem rooted at the scan target.
    inner: StdFs,
}

impl ScanFs {
    /// Root the filesystem at `base`.
    #[must_use]
    pub const fn new(base: PathBuf) -> Self {
        Self {
            inner: StdFs::new(base),
        }
    }
}

impl Fs for ScanFs {
    fn read(&self, rel: &str) -> Result<String, SummError> {
        if rel == LEAF_PATH {
            return Ok(LEAF_PROMPT.to_owned());
        }
        if rel == ROLLUP_PATH {
            return Ok(ROLLUP_PROMPT.to_owned());
        }
        self.inner.read(rel)
    }

    fn write(&self, rel: &str, contents: &str) -> Result<(), SummError> {
        self.inner.write(rel, contents)
    }

    fn exists(&self, rel: &str) -> bool {
        self.inner.exists(rel)
    }

    fn list_dir(&self, rel: &str) -> Result<Vec<String>, SummError> {
        self.inner.list_dir(rel)
    }

    fn is_ignored(&self, rel: &str) -> Result<bool, SummError> {
        self.inner.is_ignored(rel).or(Ok(false))
    }

    fn remove(&self, rel: &str) -> Result<(), SummError> {
        self.inner.remove(rel)
    }
}
