//! The capped JSON report model.
//!
//! Same schema family as the spec's audit report: a top-level status and
//! a per-check map, each check capped to a fixed diagnostic budget so the
//! token cost of a verification is deterministic.

use std::collections::BTreeMap;

use serde::ser::SerializeStruct as _;
use serde::{Serialize, Serializer};

/// Outcome of a check or of the whole run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// No diagnostics and the tool exited zero.
    Pass,
    /// Diagnostics found or the tool exited non-zero.
    Fail,
    /// The check could NOT be executed (spawn/infra error) — a distinct,
    /// non-code outcome carrying the underlying message. Never silently
    /// conflated with `fail`; makes the run's interpretation
    /// deterministic (an infra hiccup is labelled, not a phantom fail).
    Errored,
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
///
/// Serialization is token-frugal: a non-failing check emits only
/// `status` (its `count`/`diagnostics`/`truncated` are always trivial);
/// the full detail is emitted only when `status` is `fail`.
#[derive(Debug, Clone)]
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

    /// The check could not be executed. Carries the underlying message
    /// as a diagnostic so it is never a silent/empty result.
    #[must_use]
    pub fn errored(detail: String) -> Self {
        let d = Diagnostic {
            file: "<runner>".to_owned(),
            line: 0,
            col: 0,
            lint: "exec".to_owned(),
            message: detail,
        };
        Self {
            status: Status::Errored,
            count: 1,
            diagnostics: vec![d],
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

impl Serialize for CheckReport {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let full = matches!(self.status, Status::Fail | Status::Errored);
        let fields = if full { 4 } else { 1 };
        let mut st = serializer.serialize_struct("CheckReport", fields)?;
        st.serialize_field("status", &self.status)?;
        if full {
            st.serialize_field("count", &self.count)?;
            st.serialize_field("diagnostics", &self.diagnostics)?;
            st.serialize_field("truncated", &self.truncated)?;
        }
        st.end()
    }
}

/// The whole-run report. Token-frugal: an all-pass run serializes to
/// just `{"status":"pass"}` (the per-check map is omitted entirely);
/// only a failing run carries the `checks` map.
#[derive(Debug, Clone)]
pub struct Report {
    /// `fail` if any non-skipped check failed, else `pass`.
    pub status: Status,
    /// Per-check reports, keyed by check name (stable order).
    pub checks: BTreeMap<String, CheckReport>,
}

impl Serialize for Report {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let full = self.status != Status::Pass;
        let mut st = serializer.serialize_struct("Report", if full { 2 } else { 1 })?;
        st.serialize_field("status", &self.status)?;
        if full {
            st.serialize_field("checks", &self.checks)?;
        }
        st.end()
    }
}

impl Report {
    /// Assemble the top-level status from the per-check map.
    #[must_use]
    pub fn new(checks: BTreeMap<String, CheckReport>) -> Self {
        // Precedence: an inconclusive run (a check could not execute)
        // outranks a code failure — "I could not verify" is more urgent
        // for the agent than "your code failed". Skipped is ignored.
        let any = |s: Status| checks.values().any(|c| c.status == s);
        let status = if any(Status::Errored) {
            Status::Errored
        } else if any(Status::Fail) {
            Status::Fail
        } else {
            Status::Pass
        };
        Self { status, checks }
    }
}
