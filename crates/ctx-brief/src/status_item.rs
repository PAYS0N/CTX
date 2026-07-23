//! Match a free-text request against the `docs/STATUS.md` table.
//!
//! A request is matched as a case-insensitive substring of the task
//! column: exactly one match yields the formatted row as the TASK ITEM,
//! several is an ambiguity error, and none falls back to the raw request
//! text. The table model and parser are shared with `ctx-status` via
//! [`ctx_core::status_table`] rather than duplicated here.

use ctx_core::status_table::{parse_rows, Row};

use crate::error::BriefError;

/// Render a matched row as the multi-line TASK ITEM handed to the model.
fn format_item(row: &Row) -> String {
    format!(
        "TASK: {}\nDESCRIPTION: {}\nIMPACT: {}\nDIFFICULTY: {}",
        row.task, row.description, row.impact, row.difficulty
    )
}

/// Rows whose task column contains `request` as a case-insensitive substring.
fn matching_rows<'a>(rows: &'a [Row], request: &str) -> Vec<&'a Row> {
    let needle = request.trim().to_lowercase();
    rows.iter()
        .filter(|r| r.task.to_lowercase().contains(&needle))
        .collect()
}

/// Resolve `request` against the STATUS.md table into a TASK ITEM string.
///
/// # Errors
///
/// [`BriefError::AmbiguousItem`] if the request matches more than one row.
pub fn resolve(status: &str, request: &str) -> Result<String, BriefError> {
    let rows = parse_rows(status);
    let matched = matching_rows(&rows, request);
    match matched.as_slice() {
        [] => Ok(request.trim().to_owned()),
        [row] => Ok(format_item(row)),
        many => Err(BriefError::AmbiguousItem(
            many.iter()
                .map(|r| format!("- {}", r.task))
                .collect::<Vec<_>>()
                .join("\n"),
        )),
    }
}

/// Whether `request` matches at least one row's task column, using the same
/// substring rule [`resolve`] applies.
///
/// Lets a caller tell a genuine match (or an ambiguous one, already an error
/// from `resolve`) from the no-match fallback without re-deriving the rule.
#[must_use]
pub fn matched(status: &str, request: &str) -> bool {
    !matching_rows(&parse_rows(status), request).is_empty()
}

#[cfg(test)]
mod tests {
    use super::{matched, resolve, BriefError};

    /// A minimal two-row table with header + separator noise around it.
    const TABLE: &str = "\
# Status\n\
\n\
| task | description | impact | difficulty |\n\
|---|---|---|---|\n\
| wire the Stop-hook staleness report | not wired anywhere | high | easy |\n\
| take the other item | use subscription billing | high | easy |\n";

    #[test]
    fn single_substring_match_formats_the_row() -> Result<(), BriefError> {
        let item = resolve(TABLE, "stop-hook")?;
        assert!(item.starts_with("TASK: wire the Stop-hook staleness report"));
        assert!(item.contains("DIFFICULTY: easy"));
        Ok(())
    }

    #[test]
    fn no_match_returns_raw_request() -> Result<(), BriefError> {
        let item = resolve(TABLE, "invent a teleporter")?;
        assert_eq!(item, "invent a teleporter");
        Ok(())
    }

    #[test]
    fn multiple_matches_are_ambiguous() {
        let err = resolve(TABLE, "the").expect_err("must be ambiguous");
        assert!(matches!(err, BriefError::AmbiguousItem(_)));
    }

    #[test]
    fn matched_is_true_for_a_single_substring_match() {
        assert!(matched(TABLE, "stop-hook"));
    }

    #[test]
    fn matched_is_false_for_a_no_match_request() {
        assert!(!matched(TABLE, "invent a teleporter"));
    }

    #[test]
    fn escaped_pipe_survives_in_parsed_rows_and_matching() -> Result<(), BriefError> {
        let table = r"| foo \| bar | some description | high | easy |".to_owned() + "\n";
        let item = resolve(&table, "foo | bar")?;
        assert!(item.starts_with("TASK: foo | bar"));
        Ok(())
    }
}
