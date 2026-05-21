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

    /// JSON serialization/deserialization of a protocol request body.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// Wire-format invariant violated by the peer (bad tag, oversize
    /// frame, mismatched payload length, …).
    #[error("protocol: {0}")]
    Protocol(String),

    /// The peer closed the connection in the middle of a frame.
    #[error("protocol: unexpected EOF (incomplete frame)")]
    UnexpectedEof,

    /// The broker received a request for a tool that is not in its
    /// allowlist.
    #[error("tool '{0}' is not in the broker allowlist")]
    UnknownTool(String),
}
