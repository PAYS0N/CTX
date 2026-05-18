//! Audit report data model and the report-file path.
//!
//! Split out of [`crate::enforce`] so the enforcement file stays within
//! the file-length tier and the report/audit types have a single home.

use serde::{Deserialize, Serialize};

use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// One directory's intent-vs-rollup judgement (auditor JSON, verbatim).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Divergence {
    /// Repo-relative directory path.
    pub path: String,
    /// `consistent` or `divergent`.
    pub verdict: String,
    /// `none` | `low` | `medium` | `high`.
    pub severity: String,
    /// One-to-three sentence justification.
    pub rationale: String,
}

/// The `.context/.reports/<task-id>.json` document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndReport {
    /// Task this report belongs to.
    pub task_id: String,
    /// Seconds since the Unix epoch at `end-task`.
    pub completed_at: u64,
    /// Per-directory auditor judgements, verbatim.
    pub divergences: Vec<Divergence>,
}

/// Produces audit divergences from the set of written paths.
///
/// The real implementation drives the summarizer + auditor agents; it is a
/// separate (deferred) runner phase. The trait is the seam.
pub trait Summarizer {
    /// Run summarization/audit over `paths_written`.
    ///
    /// # Errors
    ///
    /// Propagates any [`CtxError`] from the underlying runner.
    fn run(&self, paths_written: &[String]) -> Result<Vec<Divergence>, CtxError>;
}

/// MVP no-op: the summarization runner is a later phase; `end-task` still
/// produces a valid (empty) report and cleans up.
#[derive(Debug, Clone, Copy)]
pub struct NoopSummarizer;

impl Summarizer for NoopSummarizer {
    fn run(&self, _paths_written: &[String]) -> Result<Vec<Divergence>, CtxError> {
        Ok(Vec::new())
    }
}

/// The `.context/.reports/<task-id>.json` path for a validated task id.
pub(crate) fn report_path(task_id: &str) -> RepoPath {
    RepoPath::root()
        .child(".context")
        .child(".reports")
        .child(&format!("{task_id}.json"))
}
