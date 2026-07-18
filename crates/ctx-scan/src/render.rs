//! Human-readable output rendering for the CLI entry points.
//!
//! Line-oriented, not a stable machine contract: `fresh`/`clean` on a
//! clean tree, otherwise one labeled line per item.

use std::io::Write;

use crate::error::ScanError;
use crate::hash::Staleness;
use crate::runner::ScanSummary;

/// Map a stdout write error to [`ScanError::Io`].
pub fn stdout_err(e: &std::io::Error) -> ScanError {
    ScanError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    }
}

/// Write a human-readable summary of a full scan to `out`.
pub fn render<W: Write>(out: &mut W, s: &ScanSummary) -> Result<(), ScanError> {
    writeln!(out, "leaves:  {}", s.leaves_written.len()).map_err(|ref e| stdout_err(e))?;
    writeln!(out, "rollups: {}", s.rollups_written.len()).map_err(|ref e| stdout_err(e))?;
    let readme = if s.readme_written {
        "written"
    } else {
        "skipped"
    };
    writeln!(out, "readme:  {readme}").map_err(|ref e| stdout_err(e))
}

/// Write a staleness report to `out` (`fresh` on a clean tree; otherwise
/// one `stale-dir:` / `stale-leaf:` / `orphan-leaf:` /
/// `missing-artifact:` / `orphan-artifact:` line per item).
pub fn render_staleness<W: Write>(out: &mut W, s: &Staleness) -> Result<(), ScanError> {
    if s.is_fresh() {
        return writeln!(out, "fresh").map_err(|ref e| stdout_err(e));
    }
    for d in &s.stale_dirs {
        let label = if d.is_empty() { "." } else { d };
        writeln!(out, "stale-dir: {label}").map_err(|ref e| stdout_err(e))?;
    }
    for f in &s.changed_files {
        writeln!(out, "stale-leaf: {f}").map_err(|ref e| stdout_err(e))?;
    }
    for l in &s.orphan_leaves {
        writeln!(out, "orphan-leaf: {l}").map_err(|ref e| stdout_err(e))?;
    }
    for m in &s.missing_artifacts {
        writeln!(out, "missing-artifact: {m}").map_err(|ref e| stdout_err(e))?;
    }
    for o in &s.orphan_artifacts {
        writeln!(out, "orphan-artifact: {o}").map_err(|ref e| stdout_err(e))?;
    }
    Ok(())
}

/// Write a prune report to `out` (`clean` when nothing was pruned;
/// otherwise one `pruned:` line per removed artifact).
pub fn render_pruned<W: Write>(out: &mut W, pruned: &[String]) -> Result<(), ScanError> {
    if pruned.is_empty() {
        return writeln!(out, "clean").map_err(|ref e| stdout_err(e));
    }
    for p in pruned {
        writeln!(out, "pruned: {p}").map_err(|ref e| stdout_err(e))?;
    }
    Ok(())
}

/// One line summarizing `s` for a human, with the refresh command
/// (the `--approve` cost gate is pre-hinted when the backlog exceeds
/// it, so following the suggestion never dead-ends on the gate).
pub fn staleness_message(dir: &std::path::Path, s: &Staleness) -> String {
    let approve = if s.changed_files.len() > ctx_summarize::runner::MAX_TARGETS {
        " --approve"
    } else {
        ""
    };
    format!(
        "ctx: context tree stale ({} dirs, {} leaves, {} missing, {} orphaned); run `ctx-scan {} --update{approve}` after the session",
        s.stale_dirs.len(),
        s.changed_files.len(),
        s.missing_artifacts.len(),
        s.orphan_artifacts.len(),
        dir.display()
    )
}
