//! Per-task cache: the rev-2 `.context/.cache/<task-id>.json` document.
//!
//! `served_nodes` is a set (stored as an order-stable list) of context
//! node and source identifiers already returned to the agent this task; it
//! fully expresses chain progress, so no per-step counter exists.

use serde::{Deserialize, Serialize};

use crate::env::Env;
use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// On-disk per-task state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCache {
    /// The validated task identifier this cache belongs to.
    pub task_id: String,
    /// Seconds since the Unix epoch at `init-task`.
    pub started_at: u64,
    /// Identifiers already served to the agent this task.
    pub served_nodes: Vec<String>,
    /// Repo-relative source paths written this task.
    pub paths_written: Vec<String>,
}

/// Validate an untrusted task id: non-empty, `[A-Za-z0-9._-]` only.
///
/// # Errors
///
/// [`CtxError::InvalidTaskId`] if empty or containing other characters.
pub fn validate_task_id(task_id: &str) -> Result<(), CtxError> {
    let ok = !task_id.is_empty()
        && task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(CtxError::InvalidTaskId(task_id.to_owned()))
    }
}

/// The cache file path for a (already validated) task id.
#[must_use]
pub fn cache_path(task_id: &str) -> RepoPath {
    RepoPath::root()
        .child(".context")
        .child(".cache")
        .child(&format!("{task_id}.json"))
}

impl TaskCache {
    /// A fresh cache for `task_id` started at `started_at`.
    #[must_use]
    pub const fn new(task_id: String, started_at: u64) -> Self {
        Self {
            task_id,
            started_at,
            served_nodes: Vec::new(),
            paths_written: Vec::new(),
        }
    }

    /// Whether `id` has already been served this task.
    #[must_use]
    pub fn has_served(&self, id: &str) -> bool {
        self.served_nodes.iter().any(|n| n == id)
    }

    /// Record `id` as served (idempotent).
    pub fn mark_served(&mut self, id: String) {
        if !self.has_served(&id) {
            self.served_nodes.push(id);
        }
    }

    /// Whether `path` was written this task.
    #[must_use]
    pub fn has_written(&self, path: &str) -> bool {
        self.paths_written.iter().any(|p| p == path)
    }

    /// Record `path` as written (idempotent).
    pub fn mark_written(&mut self, path: String) {
        if !self.has_written(&path) {
            self.paths_written.push(path);
        }
    }

    /// Drop every id in `ids` from the served set.
    ///
    /// Used by `write` to force the written path's leaf and source to be
    /// re-served (and thus re-bannered) on the next read.
    pub fn evict(&mut self, ids: &[String]) {
        self.served_nodes.retain(|n| !ids.contains(n));
    }

    /// Load the cache for `task_id`.
    ///
    /// # Errors
    ///
    /// [`CtxError::TaskMissing`] if absent; [`CtxError::CorruptCache`] if
    /// present but not valid JSON of this shape; [`CtxError::Io`] on read
    /// failure.
    pub fn load<E: Env>(env: &E, task_id: &str) -> Result<Self, CtxError> {
        let path = cache_path(task_id);
        if !env.exists(&path) {
            return Err(CtxError::TaskMissing(task_id.to_owned()));
        }
        let bytes = env.read(&path)?;
        serde_json::from_slice(&bytes).map_err(|_| CtxError::CorruptCache(task_id.to_owned()))
    }

    /// Persist this cache.
    ///
    /// # Errors
    ///
    /// [`CtxError::Io`] on write failure; [`CtxError::CorruptCache`] if
    /// serialization fails (should not happen for this shape).
    pub fn save<E: Env>(&self, env: &E) -> Result<(), CtxError> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|_| CtxError::CorruptCache(self.task_id.clone()))?;
        env.write(&cache_path(&self.task_id), &bytes)
    }
}
