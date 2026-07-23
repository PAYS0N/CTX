//! The backlog store: a `Vec<Task>` pairing a stable id with `ctx-core`'s
//! shared table row, the fixed impact/difficulty vocabulary, and the
//! priority sort.

use serde::{Deserialize, Serialize};

use ctx_core::status_table::Row;

use crate::error::StatusError;

/// One backlog task: a stable identifier plus the shared markdown-table fields.
///
/// `id` is never shown in `docs/STATUS.md` (that table's shape is shared
/// with `ctx-brief` and stays untouched) — it surfaces only through
/// `ctx-status list`, as the handle `delete-task` targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Stable id, assigned once at `add-task`/migration time and never
    /// reassigned while the row exists. `0` is a placeholder meaning
    /// "not yet assigned" — real ids start at 1; see [`backfill_ids`].
    #[serde(default)]
    pub id: u64,
    /// The task/description/impact/difficulty fields.
    #[serde(flatten)]
    pub row: Row,
}

/// The backlog: an ordered list of tasks. Order is insertion order (new
/// items append at the end); [`priority_sorted`] computes the *display*
/// order without touching this order.
pub type Store = Vec<Task>;

/// Assign a fresh id to every task still at the `0` placeholder (data
/// written before the id field existed), preserving order and never
/// colliding with an id already present in `store`.
pub fn backfill_ids(store: &mut Store) {
    let mut next = store.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    for task in store.iter_mut() {
        if task.id == 0 {
            task.id = next;
            next += 1;
        }
    }
}

/// The fixed, case-insensitive impact vocabulary, high to low.
const IMPACTS: [&str; 3] = ["high", "medium", "low"];

/// The fixed, case-insensitive difficulty vocabulary, easy to hard.
const DIFFICULTIES: [&str; 3] = ["easy", "medium", "hard"];

/// Validate and normalize `impact` to its canonical lowercase spelling.
///
/// # Errors
///
/// [`StatusError::BadImpact`] if `impact` is not one of `IMPACTS`.
pub fn normalize_impact(impact: &str) -> Result<String, StatusError> {
    normalize(impact, &IMPACTS).ok_or_else(|| StatusError::BadImpact(impact.to_owned()))
}

/// Validate and normalize `difficulty` to its canonical lowercase spelling.
///
/// # Errors
///
/// [`StatusError::BadDifficulty`] if `difficulty` is not one of `DIFFICULTIES`.
pub fn normalize_difficulty(difficulty: &str) -> Result<String, StatusError> {
    normalize(difficulty, &DIFFICULTIES)
        .ok_or_else(|| StatusError::BadDifficulty(difficulty.to_owned()))
}

/// Case-insensitively match `value` against `known`, returning the
/// canonical (lowercase) spelling.
fn normalize(value: &str, known: &[&str; 3]) -> Option<String> {
    known
        .iter()
        .find(|k| k.eq_ignore_ascii_case(value.trim()))
        .map(|k| (*k).to_owned())
}

/// Rank of a known impact/difficulty value within `known` (its index),
/// or past the end for anything else — never hit once [`normalize_impact`]
/// / [`normalize_difficulty`] have validated a value, but total so sorting
/// never panics on unexpected store content (e.g. a hand-edited file).
fn rank(value: &str, known: &[&str; 3]) -> usize {
    known
        .iter()
        .position(|k| k.eq_ignore_ascii_case(value))
        .unwrap_or(known.len())
}

/// Sort a copy of `store` by the priority rule: impact high to low, then
/// difficulty easy to hard within an impact band. Stable, so rows that
/// tie on both keys keep their store (insertion) order.
#[must_use]
pub fn priority_sorted(store: &Store) -> Vec<Task> {
    let mut tasks = store.clone();
    tasks.sort_by_key(|t| {
        (
            rank(&t.row.impact, &IMPACTS),
            rank(&t.row.difficulty, &DIFFICULTIES),
        )
    });
    tasks
}

#[cfg(test)]
mod tests {
    use super::{backfill_ids, normalize_difficulty, normalize_impact, priority_sorted, Row, Task};

    fn row(task: &str, impact: &str, difficulty: &str) -> Task {
        Task {
            id: 1,
            row: Row {
                task: task.to_owned(),
                description: "d".to_owned(),
                impact: impact.to_owned(),
                difficulty: difficulty.to_owned(),
            },
        }
    }

    #[test]
    fn normalize_impact_accepts_case_insensitively() {
        assert_eq!(normalize_impact("High").expect("valid"), "high");
    }

    #[test]
    fn normalize_impact_rejects_unknown_values() {
        assert!(normalize_impact("urgent").is_err());
    }

    #[test]
    fn normalize_difficulty_rejects_unknown_values() {
        assert!(normalize_difficulty("impossible").is_err());
    }

    #[test]
    fn sorts_by_impact_then_difficulty() {
        let store = vec![
            row("a", "low", "easy"),
            row("b", "high", "hard"),
            row("c", "high", "easy"),
            row("d", "medium", "medium"),
        ];
        let sorted = priority_sorted(&store);
        let tasks: Vec<&str> = sorted.iter().map(|t| t.row.task.as_str()).collect();
        assert_eq!(tasks, vec!["c", "b", "d", "a"]);
    }

    #[test]
    fn ties_keep_insertion_order() {
        let store = vec![row("first", "high", "easy"), row("second", "high", "easy")];
        let sorted = priority_sorted(&store);
        let tasks: Vec<&str> = sorted.iter().map(|t| t.row.task.as_str()).collect();
        assert_eq!(tasks, vec!["first", "second"]);
    }

    #[test]
    fn priority_sorted_does_not_mutate_the_store_order() {
        let store = vec![row("b", "low", "easy"), row("a", "high", "easy")];
        let _ = priority_sorted(&store);
        assert_eq!(store.first().map(|t| t.row.task.as_str()), Some("b"));
    }

    #[test]
    fn backfill_ids_assigns_sequential_ids_to_placeholder_rows() {
        let mut store = vec![row("a", "high", "easy"), row("b", "low", "hard")];
        for task in &mut store {
            task.id = 0;
        }
        backfill_ids(&mut store);
        let ids: Vec<u64> = store.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn backfill_ids_never_collides_with_an_existing_id() {
        let mut store = vec![row("a", "high", "easy"), row("b", "low", "hard")];
        if let [first, second] = store.as_mut_slice() {
            first.id = 5;
            second.id = 0;
        }
        backfill_ids(&mut store);
        let ids: Vec<u64> = store.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec![5, 6]);
    }

    #[test]
    fn backfill_ids_leaves_already_assigned_ids_untouched() {
        let mut store = vec![row("a", "high", "easy")];
        if let [only] = store.as_mut_slice() {
            only.id = 3;
        }
        backfill_ids(&mut store);
        assert_eq!(store.first().map(|t| t.id), Some(3));
    }
}
