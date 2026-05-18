//! The capped JSON report model.
//!
//! Same schema family as the spec's audit report: a top-level status and
//! a per-check map, each check capped to a fixed diagnostic budget so the
//! token cost of a verification is deterministic.

use std::collections::BTreeMap;

use serde::Serialize;

/// Outcome of a check or of the whole run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// No diagnostics and the tool exited zero.
    Pass,
    /// Diagnostics found or the tool exited non-zero.
    Fail,
    /// The tool is not installed; not counted against the run.
    Skipped,
}

/// One normalized diagnostic, uniform across every wrapped tool.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// Repo-relative file the diagnostic concerns (may be empty).
    pub file: String,
    /// 1-based line, or 0 when not applicable.
    pub line: u64,
    /// 1-based column, or 0 when not applicable.
    pub col: u64,
    /// Lint/category identifier (e.g. a clippy lint, `rustfmt`, `script`).
    pub lint: String,
    /// Short human-readable message.
    pub message: String,
}

/// Per-check result with a bounded diagnostic list.
#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    /// Pass/fail/skipped for this check.
    pub status: Status,
    /// Total diagnostics found (before capping).
    pub count: usize,
    /// At most `max` diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// How many diagnostics were omitted by the cap.
    pub truncated: usize,
}

impl CheckReport {
    /// A skipped check (tool absent).
    #[must_use]
    pub const fn skipped() -> Self {
        Self {
            status: Status::Skipped,
            count: 0,
            diagnostics: Vec::new(),
            truncated: 0,
        }
    }

    /// A failed check with no parsable diagnostics (e.g. spawn error).
    #[must_use]
    pub const fn failed_bare() -> Self {
        Self {
            status: Status::Fail,
            count: 0,
            diagnostics: Vec::new(),
            truncated: 0,
        }
    }

    /// Build from a command's success flag and parsed diagnostics, capping
    /// the list to `max`.
    #[must_use]
    pub fn build(cmd_ok: bool, mut diagnostics: Vec<Diagnostic>, max: usize) -> Self {
        let count = diagnostics.len();
        let status = if count > 0 || !cmd_ok {
            Status::Fail
        } else {
            Status::Pass
        };
        let truncated = count.saturating_sub(max);
        diagnostics.truncate(max);
        Self {
            status,
            count,
            diagnostics,
            truncated,
        }
    }
}

/// The whole-run report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// `fail` if any non-skipped check failed, else `pass`.
    pub status: Status,
    /// Per-check reports, keyed by check name (stable order).
    pub checks: BTreeMap<String, CheckReport>,
}

impl Report {
    /// Assemble the top-level status from the per-check map.
    #[must_use]
    pub fn new(checks: BTreeMap<String, CheckReport>) -> Self {
        let any_fail = checks.values().any(|c| c.status == Status::Fail);
        let status = if any_fail { Status::Fail } else { Status::Pass };
        Self { status, checks }
    }
}
