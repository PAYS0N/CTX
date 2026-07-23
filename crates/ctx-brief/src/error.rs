//! Typed error taxonomy for ctx-brief.

use thiserror::Error;

/// Every failure surfaced by the runner and its boundaries.
#[derive(Debug, Error)]
pub enum BriefError {
    /// The request matched more than one backlog row; the payload lists
    /// the competing task titles so the caller can disambiguate.
    #[error("request matched multiple status items; narrow it:\n{0}")]
    AmbiguousItem(String),
    /// A required prompt file is missing or unreadable.
    #[error("prompt file unreadable: {0:?}")]
    PromptMissing(String),
    /// A `claude` stage could not be run, exited non-zero, or (for the
    /// captured gather/headless passes) produced empty output.
    #[error("claude stage failed: {0}")]
    Claude(String),
    /// `--id` was given but `docs/status.json` has no row with that id,
    /// or the file couldn't be read/parsed as a task array.
    #[error("no docs/status.json row with id {0}")]
    TaskIdNotFound(u64),
    /// Neither `<request>` nor `--id` was given.
    #[error("either <request> or --id is required")]
    NoSelector,
    /// The interactive plan session exited without leaving a brief at the
    /// expected output path.
    #[error("interactive session exited without writing the brief to {0:?}")]
    BriefNotWritten(String),
    /// An underlying filesystem operation failed.
    #[error("io error on {path:?}: {detail}")]
    Io {
        /// Path the failed operation targeted.
        path: String,
        /// Human-readable cause.
        detail: String,
    },
}
