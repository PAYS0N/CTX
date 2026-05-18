//! Minimal read-only view of the `ctx-access` per-task cache.
//!
//! The runner only needs `paths_written`; it does not write or delete the
//! cache (the spec makes `ctx-access end-task` the cache's deleter).

use serde::Deserialize;

use crate::error::SummError;
use crate::fs::Fs;

/// Just the field the runner consumes.
#[derive(Deserialize)]
struct TaskCacheView {
    /// Repo-relative source paths written during the task.
    paths_written: Vec<String>,
}

/// Whether a task id is the safe `[A-Za-z0-9._-]+` shape.
fn valid_task_id(task_id: &str) -> bool {
    !task_id.is_empty()
        && task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Read `paths_written` from `.context/.cache/<task-id>.json`.
///
/// # Errors
///
/// [`SummError::CacheRead`] if the id is malformed or the cache is
/// missing/unparseable.
pub fn paths_from_cache<F: Fs>(fs: &F, task_id: &str) -> Result<Vec<String>, SummError> {
    if !valid_task_id(task_id) {
        return Err(SummError::CacheRead(task_id.to_owned()));
    }
    let rel = format!(".context/.cache/{task_id}.json");
    let raw = fs
        .read(&rel)
        .map_err(|_| SummError::CacheRead(task_id.to_owned()))?;
    let view: TaskCacheView =
        serde_json::from_str(&raw).map_err(|_| SummError::CacheRead(task_id.to_owned()))?;
    Ok(view.paths_written)
}
