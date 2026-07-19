//! Pure `docs/STATUS.md` table parsing and request matching.
//!
//! The backlog is a single GitHub-flavoured markdown table with columns
//! `task | description | impact | difficulty`. A request is matched as a
//! case-insensitive substring of the task column: exactly one match yields
//! the formatted row as the TASK ITEM, several is an ambiguity error, and
//! none falls back to the raw request text.

use crate::error::BriefError;

/// One parsed backlog row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The task column (the short title matched against).
    pub task: String,
    /// The description column.
    pub description: String,
    /// The impact column.
    pub impact: String,
    /// The difficulty column.
    pub difficulty: String,
}

/// Split one table line into its four trimmed cells, or `None` if the line
/// is not a four-column table row.
fn split_row(line: &str) -> Option<[String; 4]> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return None;
    }
    let cells: Vec<String> = trimmed
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_owned())
        .collect();
    let [task, description, impact, difficulty] = <[String; 4]>::try_from(cells).ok()?;
    Some([task, description, impact, difficulty])
}

/// Whether every cell is a markdown separator run (`-`/`:` only).
fn is_separator(cells: &[String; 4]) -> bool {
    cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

/// Whether the cells are the literal header row.
fn is_header(cells: &[String; 4]) -> bool {
    let [task, description, ..] = cells;
    task.eq_ignore_ascii_case("task") && description.eq_ignore_ascii_case("description")
}

/// Parse every data row out of a STATUS.md document.
#[must_use]
pub fn parse_rows(status: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in status.lines() {
        let Some(cells) = split_row(line) else {
            continue;
        };
        if is_separator(&cells) || is_header(&cells) {
            continue;
        }
        let [task, description, impact, difficulty] = cells;
        rows.push(Row {
            task,
            description,
            impact,
            difficulty,
        });
    }
    rows
}

/// Render a matched row as the multi-line TASK ITEM handed to the model.
fn format_item(row: &Row) -> String {
    format!(
        "TASK: {}\nDESCRIPTION: {}\nIMPACT: {}\nDIFFICULTY: {}",
        row.task, row.description, row.impact, row.difficulty
    )
}

/// Resolve `request` against the STATUS.md table into a TASK ITEM string.
///
/// # Errors
///
/// [`BriefError::AmbiguousItem`] if the request matches more than one row.
pub fn resolve(status: &str, request: &str) -> Result<String, BriefError> {
    let rows = parse_rows(status);
    let needle = request.trim().to_lowercase();
    let matched: Vec<&Row> = rows
        .iter()
        .filter(|r| r.task.to_lowercase().contains(&needle))
        .collect();
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

#[cfg(test)]
mod tests {
    use super::{parse_rows, resolve, BriefError};

    /// A minimal two-row table with header + separator noise around it.
    const TABLE: &str = "\
# Status\n\
\n\
| task | description | impact | difficulty |\n\
|---|---|---|---|\n\
| wire the Stop-hook staleness report | not wired anywhere | high | easy |\n\
| take the other item | use subscription billing | high | easy |\n";

    #[test]
    fn parses_only_data_rows() {
        let rows = parse_rows(TABLE);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.first().map(|r| r.task.as_str()),
            Some("wire the Stop-hook staleness report")
        );
    }

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
}
