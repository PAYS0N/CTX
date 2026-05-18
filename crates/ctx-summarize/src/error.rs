//! Typed error taxonomy for the summarization runner.

use thiserror::Error;

/// Every failure surfaced by the runner.
#[derive(Debug, Error)]
pub enum SummError {
    /// A user-supplied path is absolute or escapes the repository root.
    #[error("path {0:?} is absolute or escapes the repo root")]
    PathEscape(String),
    /// A required prompt file is missing or unreadable.
    #[error("prompt file unreadable: {0:?}")]
    MissingPrompt(String),
    /// The task cache could not be read or parsed.
    #[error("cannot read task cache for {0:?}")]
    CacheRead(String),
    /// No agent command was configured (env var unset/empty).
    #[error("no agent command configured (set CTX_AGENT_CMD)")]
    NoAgentCommand,
    /// The agent subprocess failed, exited non-zero, or produced nothing.
    #[error("agent failed: {0}")]
    Agent(String),
    /// An underlying filesystem operation failed.
    #[error("io error on {path:?}: {detail}")]
    Io {
        /// Path the failed operation targeted.
        path: String,
        /// Human-readable cause.
        detail: String,
    },
}
