//! The backlog store: a `Vec<Row>` (re-using `ctx-core`'s shared table
//! row), the fixed impact/difficulty vocabulary, and the priority sort.

use ctx_core::status_table::Row;

use crate::error::StatusError;

/// The backlog: an ordered list of rows. Order is insertion order (new
/// items append at the end); [`priority_sorted`] computes the *display*
/// order without touching this order.
pub type Store = Vec<Row>;

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
pub fn priority_sorted(store: &Store) -> Vec<Row> {
    let mut rows = store.clone();
    rows.sort_by_key(|r| {
        (
            rank(&r.impact, &IMPACTS),
            rank(&r.difficulty, &DIFFICULTIES),
        )
    });
    rows
}

#[cfg(test)]
mod tests {
    use super::{normalize_difficulty, normalize_impact, priority_sorted, Row};

    fn row(task: &str, impact: &str, difficulty: &str) -> Row {
        Row {
            task: task.to_owned(),
            description: "d".to_owned(),
            impact: impact.to_owned(),
            difficulty: difficulty.to_owned(),
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
        let tasks: Vec<&str> = sorted.iter().map(|r| r.task.as_str()).collect();
        assert_eq!(tasks, vec!["c", "b", "d", "a"]);
    }

    #[test]
    fn ties_keep_insertion_order() {
        let store = vec![row("first", "high", "easy"), row("second", "high", "easy")];
        let sorted = priority_sorted(&store);
        let tasks: Vec<&str> = sorted.iter().map(|r| r.task.as_str()).collect();
        assert_eq!(tasks, vec!["first", "second"]);
    }

    #[test]
    fn priority_sorted_does_not_mutate_the_store_order() {
        let store = vec![row("b", "low", "easy"), row("a", "high", "easy")];
        let _ = priority_sorted(&store);
        assert_eq!(store.first().map(|r| r.task.as_str()), Some("b"));
    }
}
