//! Shared `docs/STATUS.md`-shaped markdown table: the row model, parser,
//! and the pipe-escaping the parser's inverse (a renderer) needs.
//!
//! The backlog is a single GitHub-flavoured markdown table with columns
//! `task | description | impact | difficulty`. This module owns the one
//! parser for that shape so `ctx-brief` (matching a request against the
//! table) and `ctx-status` (migrating the table into its JSON store, and
//! rendering the store back into the table) never carry their own copies.

use serde::{Deserialize, Serialize};

/// One parsed backlog row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// One backlog row plus its stable id, as stored in `docs/status.json`.
///
/// Never rendered by [`render_row`] or parsed as a table column — it
/// exists so JSON-backed consumers (`ctx-status`, `ctx-brief`) can
/// address a row by a stable handle instead of matching task text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Stable id; `0` is a placeholder for "not yet assigned" (see
    /// `ctx-status`'s id-backfill).
    #[serde(default)]
    pub id: u64,
    /// The shared row fields.
    #[serde(flatten)]
    pub row: Row,
}

/// Split `s` on `|` delimiters, treating `\|` as a literal pipe rather than
/// a delimiter and unescaping it to `|` in the returned pieces. Any other
/// backslash is left untouched.
fn split_unescaped_pipes(s: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'|') {
            current.push('|');
            chars.next();
        } else if ch == '|' {
            cells.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    cells.push(current);
    cells
}

/// Split one table line into its four trimmed cells, or `None` if the line
/// is not a four-column table row. `|` is the cell delimiter; `\|` is a
/// literal pipe within a cell, not a delimiter.
fn split_row(line: &str) -> Option<[String; 4]> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return None;
    }
    let mut cells = split_unescaped_pipes(trimmed);
    if cells.first().is_some_and(String::is_empty) {
        cells.remove(0);
    }
    if cells.last().is_some_and(String::is_empty) {
        cells.pop();
    }
    let cells: Vec<String> = cells.iter().map(|c| c.trim().to_owned()).collect();
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

/// Parse every data row out of a STATUS.md-shaped document.
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

/// Escape literal `|` characters in a cell so it survives a round trip
/// through [`parse_rows`] as a single table cell rather than splitting.
///
/// The exact inverse of the pipe-unescaping [`parse_rows`] applies.
#[must_use]
pub fn escape_cell(cell: &str) -> String {
    cell.replace('|', "\\|")
}

/// Render one [`Row`] as a single markdown table line, escaping any
/// literal `|` in each cell so [`parse_rows`] reconstructs it unchanged.
#[must_use]
pub fn render_row(row: &Row) -> String {
    format!(
        "| {} | {} | {} | {} |",
        escape_cell(&row.task),
        escape_cell(&row.description),
        escape_cell(&row.impact),
        escape_cell(&row.difficulty)
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_rows, render_row, split_row, Row};

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
    fn escaped_pipe_is_not_a_delimiter() {
        let cells = split_row(r"| a \| b | description | impact | difficulty |")
            .expect("well-formed four-column row");
        assert_eq!(cells, ["a | b", "description", "impact", "difficulty"]);
    }

    #[test]
    fn escaped_pipe_survives_parse_round_trip() {
        let table = r"| foo \| bar | some description | high | easy |".to_owned() + "\n";
        let rows = parse_rows(&table);
        assert_eq!(rows.first().map(|r| r.task.as_str()), Some("foo | bar"));
        let rendered = render_row(rows.first().expect("one row"));
        assert_eq!(rendered, r"| foo \| bar | some description | high | easy |");
    }

    #[test]
    fn render_then_parse_recovers_the_same_row() {
        let row = Row {
            task: "a | b".to_owned(),
            description: "c \\| d".to_owned(),
            impact: "high".to_owned(),
            difficulty: "easy".to_owned(),
        };
        let line = render_row(&row);
        let parsed = parse_rows(&line);
        assert_eq!(parsed, vec![row]);
    }
}
