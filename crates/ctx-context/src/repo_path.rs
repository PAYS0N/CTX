//! `RepoPath`: a validated, repo-relative path.
//!
//! Centralizes the only place untrusted path strings enter the system.
//! Construction rejects absolute paths and any `.`/`..`/empty component,
//! so no later code can be tricked into escaping the repository root
//! (deferred dylint rule 8, observed early here as dogfood).

use crate::error::CtxError;

/// A repo-relative path as an ordered list of validated segments.
///
/// An empty segment list denotes the repository root directory itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPath {
    /// Path segments, each guaranteed non-empty and free of `/`, `.`,
    /// `..`, and NUL.
    segments: Vec<String>,
}

impl RepoPath {
    /// The repository root directory (zero segments).
    #[must_use]
    pub const fn root() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Parse and validate an untrusted repo-relative path string.
    ///
    /// # Errors
    ///
    /// Returns [`CtxError::PathEscape`] if `raw` is absolute, empty, or
    /// contains a `.`, `..`, or empty component.
    pub fn parse(raw: &str) -> Result<Self, CtxError> {
        if raw.is_empty() || raw.starts_with('/') {
            return Err(CtxError::PathEscape(raw.to_owned()));
        }
        let mut segments = Vec::new();
        for part in raw.split('/') {
            if part.is_empty() || part == "." || part == ".." || part.contains('\0') {
                return Err(CtxError::PathEscape(raw.to_owned()));
            }
            segments.push(part.to_owned());
        }
        Ok(Self { segments })
    }

    /// True when this denotes the repository root directory.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Render as a `/`-joined repo-relative string (empty for the root).
    #[must_use]
    pub fn as_string(&self) -> String {
        self.segments.join("/")
    }

    /// The final segment (file or directory name), if any.
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.segments.last().map(String::as_str)
    }

    /// Append one already-trusted segment, returning the new path.
    ///
    /// The segment is assumed valid (it originates from code, not user
    /// input); callers must not pass user data here.
    #[must_use]
    pub fn child(&self, segment: &str) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment.to_owned());
        Self { segments }
    }

    /// Prefix `base` in front of this path's segments.
    #[must_use]
    pub fn under(&self, base: &str) -> Self {
        let mut segments = vec![base.to_owned()];
        segments.extend(self.segments.iter().cloned());
        Self { segments }
    }

    /// Directory prefixes from the repo root down to this path's parent.
    ///
    /// For `a/b/c.rs` this is `["", "a", "a/b"]` as [`RepoPath`]s. For a
    /// path with a single segment it is just `[""]` (the root).
    #[must_use]
    pub fn dir_chain(&self) -> Vec<Self> {
        let mut chain = vec![Self::root()];
        let mut acc: Vec<String> = Vec::new();
        let parent_len = self.segments.len().saturating_sub(1);
        for segment in self.segments.iter().take(parent_len) {
            acc.push(segment.clone());
            chain.push(Self {
                segments: acc.clone(),
            });
        }
        chain
    }
}
