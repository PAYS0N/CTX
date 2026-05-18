//! Typed error taxonomy for ctx-access.
//!
//! No `Box<dyn Error>` / `anyhow`: every failure is a concrete variant so
//! callers and tests can match specific modes (deferred dylint rule 7,
//! observed early here as dogfood).

use thiserror::Error;

/// Every failure surfaced by a ctx-access operation.
#[derive(Debug, Error)]
pub enum CtxError {
    /// The supplied task id is empty or contains disallowed characters.
    #[error("invalid task id {0:?}: must be non-empty [A-Za-z0-9._-]")]
    InvalidTaskId(String),
    /// `init-task` was asked to create a cache that already exists.
    #[error("task {0:?} already initialized (pass --force to reclaim)")]
    TaskExists(String),
    /// A per-request command ran without a prior `init-task`.
    #[error("task {0:?} not initialized (run init-task first)")]
    TaskMissing(String),
    /// `write` was attempted on a path whose source was not read this task.
    #[error("write denied: source of {path:?} not read in task {task:?}")]
    WriteWithoutRead {
        /// Repo-relative source path the write targeted.
        path: String,
        /// Task id under which the write was attempted.
        task: String,
    },
    /// `list` was attempted before the directory rollup was served.
    #[error("list denied: rollup for {dir:?} not read in task {task:?}")]
    ListWithoutRollup {
        /// Repo-relative directory whose listing was requested.
        dir: String,
        /// Task id under which the listing was attempted.
        task: String,
    },
    /// A user-supplied path is absolute or escapes the repository root.
    #[error("path {0:?} is absolute or escapes the repo root")]
    PathEscape(String),
    /// A required context-tree node does not exist on disk.
    #[error("missing context node: {0:?}")]
    MissingNode(String),
    /// An underlying filesystem operation failed.
    #[error("io error on {path:?}: {detail}")]
    Io {
        /// Path the failed operation targeted.
        path: String,
        /// Human-readable cause.
        detail: String,
    },
    /// A cache file existed but was not valid JSON of the expected shape.
    #[error("corrupt cache for task {0:?}")]
    CorruptCache(String),
}
