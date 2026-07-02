//! Typed error taxonomy for ctx-context.
//!
//! No `Box<dyn Error>` / `anyhow`: every failure is a concrete variant so
//! callers and tests can match specific modes.

use thiserror::Error;

/// Every failure surfaced by a ctx-context operation.
#[derive(Debug, Error)]
pub enum CtxError {
    /// A user-supplied path is absolute or escapes the repository root.
    #[error("path {0:?} is absolute or escapes the repo root")]
    PathEscape(String),
    /// An underlying filesystem operation failed.
    #[error("io error on {path:?}: {detail}")]
    Io {
        /// Path the failed operation targeted.
        path: String,
        /// Human-readable cause.
        detail: String,
    },
    /// The command line was malformed.
    #[error("usage: {0}")]
    Usage(String),
}
