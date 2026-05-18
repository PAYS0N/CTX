//! Typed error taxonomy for ctx-check.

use thiserror::Error;

/// Every failure surfaced by ctx-check.
#[derive(Debug, Error)]
pub enum CheckError {
    /// The external tool binary was not found on `PATH`.
    #[error("tool not installed: {0}")]
    ToolMissing(String),
    /// Spawning or waiting on a tool failed for a reason other than absence.
    #[error("failed to run {tool}: {detail}")]
    Spawn {
        /// The tool that could not be run.
        tool: String,
        /// Human-readable cause.
        detail: String,
    },
    /// Serializing the final report failed.
    #[error("failed to encode report: {0}")]
    Encode(String),
    /// Writing the report to the output sink failed.
    #[error("failed to write report: {0}")]
    Write(String),
}
