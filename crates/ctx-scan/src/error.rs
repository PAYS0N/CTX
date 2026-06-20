//! Typed error taxonomy for the scan runner.

use std::path::PathBuf;

use ctx_summarize::error::SummError;
use thiserror::Error;

/// Every failure surfaced by the scanner.
#[derive(Debug, Error)]
pub enum ScanError {
    /// Target path does not exist or is not a directory.
    #[error("not a directory: {0}")]
    DirNotFound(PathBuf),

    /// A filesystem walk error (`read_dir` or entry access).
    #[error("walk error in {path}: {detail}")]
    Walk {
        /// Directory where the error occurred.
        path: String,
        /// Human-readable cause.
        detail: String,
    },

    /// A summarization step failed (scope, leaf, rollup, or prompt).
    #[error("summarization failed: {0}")]
    Summarize(#[from] SummError),

    /// An I/O error not covered by the summarizer (e.g., README write).
    #[error("io error on {path}: {detail}")]
    Io {
        /// Path the failed operation targeted.
        path: String,
        /// Human-readable cause.
        detail: String,
    },
}
