//! Orchestration over the [`Fs`] seam: load/save the JSON store, append
//! or delete rows, and keep the rendered `docs/STATUS.md` view in sync.

use ctx_core::status_table::{parse_rows, Row};

use crate::error::StatusError;
use crate::fs::Fs;
use crate::model::{
    backfill_ids, normalize_difficulty, normalize_impact, priority_sorted, Store, Task,
};
use crate::render::render_markdown;

/// Where the store and its rendered view live, relative to the process
/// working directory.
pub struct Paths {
    /// The JSON store: source of truth.
    pub store: String,
    /// The generated markdown view.
    pub status_md: String,
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            store: "docs/status.json".to_owned(),
            status_md: "docs/STATUS.md".to_owned(),
        }
    }
}

/// Load the store from `paths.store`, decoding its JSON array of tasks
/// and backfilling ids for any row written before the id field existed.
///
/// # Errors
///
/// [`StatusError::Io`] if the file cannot be read; [`StatusError::StoreCorrupt`]
/// if its contents are not a valid JSON array of tasks.
fn load<F: Fs>(fs: &F, paths: &Paths) -> Result<Store, StatusError> {
    let text = fs.read(&paths.store)?;
    let mut store: Store = serde_json::from_str(&text).map_err(|e| StatusError::StoreCorrupt {
        path: paths.store.clone(),
        detail: e.to_string(),
    })?;
    backfill_ids(&mut store);
    Ok(store)
}

/// Write `store` to `paths.store` as pretty-printed, git-diffable JSON,
/// and re-render `paths.status_md` from it in the same step so the view
/// can never lag the store it was written from.
///
/// # Errors
///
/// [`StatusError::Io`] if either file cannot be written.
fn save<F: Fs>(fs: &F, paths: &Paths, store: &Store) -> Result<(), StatusError> {
    let json = serde_json::to_string_pretty(store).map_err(|e| StatusError::StoreCorrupt {
        path: paths.store.clone(),
        detail: e.to_string(),
    })?;
    fs.write(&paths.store, &format!("{json}\n"))?;
    fs.write(&paths.status_md, &render_markdown(store))
}

/// `ctx-status list`: the priority-sorted backlog. Read-only — the
/// on-demand surfacing mechanism, no file is written.
///
/// # Errors
///
/// Propagates a load failure (missing or corrupt store).
pub fn list<F: Fs>(fs: &F, paths: &Paths) -> Result<Vec<Task>, StatusError> {
    let store = load(fs, paths)?;
    Ok(priority_sorted(&store))
}

/// `ctx-status add-task`: validate impact/difficulty, append one row to
/// the store (never reordering or touching existing rows) under a fresh
/// id, then persist the store and re-render the view.
///
/// # Errors
///
/// [`StatusError::BadImpact`]/[`StatusError::BadDifficulty`] on an
/// unrecognized value; otherwise propagates a load/save failure.
pub fn add_task<F: Fs>(
    fs: &F,
    paths: &Paths,
    task: &str,
    description: &str,
    impact: &str,
    difficulty: &str,
) -> Result<u64, StatusError> {
    let impact = normalize_impact(impact)?;
    let difficulty = normalize_difficulty(difficulty)?;
    let mut store = load(fs, paths)?;
    let id = store.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    store.push(Task {
        id,
        row: Row {
            task: task.trim().to_owned(),
            description: description.trim().to_owned(),
            impact,
            difficulty,
        },
    });
    save(fs, paths, &store)?;
    Ok(id)
}

/// Re-render `paths.status_md` from the current store, without changing
/// the store itself.
///
/// For re-syncing the view after an operator hand-edit to the JSON store
/// (D3's edit/reorder/delete authority stays with the operator, not this
/// tool, but the view must still catch up).
///
/// # Errors
///
/// Propagates a load/save failure.
pub fn render<F: Fs>(fs: &F, paths: &Paths) -> Result<(), StatusError> {
    let store = load(fs, paths)?;
    save(fs, paths, &store)
}

/// `ctx-status delete-task`: remove the one row with the given `id`, then
/// persist the store and re-render the view.
///
/// # Errors
///
/// [`StatusError::TaskIdNotFound`] if no row has that id; otherwise
/// propagates a load/save failure.
pub fn delete_task<F: Fs>(fs: &F, paths: &Paths, id: u64) -> Result<(), StatusError> {
    let mut store = load(fs, paths)?;
    if !store.iter().any(|t| t.id == id) {
        return Err(StatusError::TaskIdNotFound(id));
    }
    store.retain(|t| t.id != id);
    save(fs, paths, &store)
}

/// One-time bootstrap: parse the existing `docs/STATUS.md`-shaped table
/// at `source_md` into rows and seed `paths.store` with them.
///
/// Also renders `paths.status_md`. Refuses to run against a store that
/// already has content, so it can only ever be the one-time migration,
/// never a merge.
///
/// # Errors
///
/// [`StatusError::StoreNotEmpty`] if the store already has rows;
/// otherwise propagates a load/save failure. A missing store is treated
/// as empty (the expected pre-migration state), not an error.
pub fn migrate<F: Fs>(fs: &F, paths: &Paths, source_md: &str) -> Result<usize, StatusError> {
    let existing: Store = if fs.exists(&paths.store) {
        load(fs, paths)?
    } else {
        Store::new()
    };
    if !existing.is_empty() {
        return Err(StatusError::StoreNotEmpty(paths.store.clone()));
    }
    let text = fs.read(source_md)?;
    let rows = parse_rows(&text);
    let count = rows.len();
    let store: Store = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| Task {
            id: u64::try_from(i + 1).unwrap_or(u64::MAX),
            row,
        })
        .collect();
    save(fs, paths, &store)?;
    Ok(count)
}

#[cfg(test)]
mod tests;
