//! Pure parsers: tool output -> normalized [`Diagnostic`]s.
//!
//! No process execution here, so every parser is unit-testable against a
//! canned string.

use serde_json::Value;

use crate::model::Diagnostic;

/// String value at `key`, or empty.
fn str_at(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The primary span of a rustc message, or the first span.
fn primary_span(msg: &Value) -> Option<&Value> {
    let spans = msg.get("spans")?.as_array()?;
    spans
        .iter()
        .find(|s| {
            s.get("is_primary")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .or_else(|| spans.first())
}

/// Build a diagnostic from one rustc `message` object, if it is an
/// error/warning.
fn diag_from_message(msg: &Value) -> Option<Diagnostic> {
    let level = msg.get("level").and_then(Value::as_str)?;
    if level != "error" && level != "warning" {
        return None;
    }
    let lint_code = msg
        .get("code")
        .and_then(|c| c.get("code"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let (file, line, col) = primary_span(msg).map_or_else(
        || (String::new(), 0, 0),
        |s| {
            (
                str_at(s, "file_name"),
                s.get("line_start").and_then(Value::as_u64).unwrap_or(0),
                s.get("column_start").and_then(Value::as_u64).unwrap_or(0),
            )
        },
    );
    Some(Diagnostic {
        file,
        line,
        col,
        lint: lint_code,
        message: str_at(msg, "message"),
    })
}

/// Whether `out` already holds an equivalent diagnostic.
fn is_dup(out: &[Diagnostic], d: &Diagnostic) -> bool {
    out.iter()
        .any(|e| e.file == d.file && e.line == d.line && e.lint == d.lint && e.message == d.message)
}

/// Parse `cargo --message-format=json` stdout (clippy and rustdoc).
#[must_use]
pub fn parse_compiler_json(stdout: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("reason").and_then(Value::as_str) != Some("compiler-message") {
            continue;
        }
        if let Some(d) = v.get("message").and_then(diag_from_message) {
            if !is_dup(&out, &d) {
                out.push(d);
            }
        }
    }
    out
}

/// Split a leading `path:line` prefix from a script FAIL line.
fn split_loc(s: &str) -> (String, u64) {
    if let Some(idx) = s.find(':') {
        let (path, rest) = s.split_at(idx);
        let digits: String = rest
            .trim_start_matches(':')
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        return (path.to_owned(), digits.parse::<u64>().unwrap_or(0));
    }
    (s.to_owned(), 0)
}

/// Parse `FAIL: ...` lines emitted by the bash/python check scripts.
#[must_use]
pub fn parse_fail_lines(text: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("FAIL:") else {
            continue;
        };
        let trimmed = rest.trim();
        let (file, ln) = split_loc(trimmed);
        out.push(Diagnostic {
            file,
            line: ln,
            col: 0,
            lint: "script".to_owned(),
            message: trimmed.to_owned(),
        });
    }
    out
}

/// Parse `cargo fmt --check` stdout (`Diff in <path>:...`).
#[must_use]
pub fn parse_fmt(stdout: &str) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    for line in stdout.lines() {
        let Some(rest) = line.strip_prefix("Diff in ") else {
            continue;
        };
        let file = rest.split(':').next().unwrap_or(rest).to_owned();
        if !out.iter().any(|e| e.file == file) {
            out.push(Diagnostic {
                file,
                line: 0,
                col: 0,
                lint: "rustfmt".to_owned(),
                message: "needs formatting".to_owned(),
            });
        }
    }
    out
}
