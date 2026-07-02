//! Errors for `ctx-cage`. One enum, narrow variants — each variant
//! carries the smallest context useful at the boundary; richer error
//! types belong on the per-module result, not here.

use thiserror::Error;

/// Every recoverable failure `ctx-cage` can surface at a public API.
#[derive(Debug, Error)]
pub enum CageError {
    /// Underlying I/O failure (socket, file, process).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization failure.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// A precondition or invariant violated at a boundary (missing
    /// runtime, malformed HTTP head, dirty tree, spend gate, …).
    #[error("cage: {0}")]
    Protocol(String),
}
