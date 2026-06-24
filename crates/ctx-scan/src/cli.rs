//! Argument parsing and top-level dispatch.

use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use ctx_summarize::agent::Agent;

use crate::error::ScanError;
use crate::runner::{scan_run, ScanSummary};
use crate::walker::walk_dir;

/// Self-contained directory scanner — walks `<dir>` and writes a
/// `.context/` summary tree alongside the source files.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-scan",
    about = "Scan a directory and write a .context/ summary tree"
)]
pub struct Cli {
    /// Directory to scan (must exist).
    dir: PathBuf,

    /// Permit a run over more than `MAX_TARGETS` files.
    ///
    /// Without this flag an over-large target set is refused as a
    /// cost/blast-radius guard.
    #[arg(long)]
    approve: bool,

    /// List the files that would be summarized and exit without calling
    /// the agent. Does not require `CTX_AGENT_CMD` to be set.
    #[arg(long)]
    dry_run: bool,
}

impl Cli {
    /// Whether dry-run mode is active.
    #[must_use]
    pub const fn dry_run(&self) -> bool {
        self.dry_run
    }
}

/// Resolve and validate `dir` as an existing directory.
fn validate_dir(dir: &std::path::Path) -> Result<std::path::PathBuf, ScanError> {
    if !dir.is_dir() {
        return Err(ScanError::DirNotFound(dir.to_path_buf()));
    }
    dir.canonicalize().map_err(|e| ScanError::Io {
        path: dir.to_string_lossy().into_owned(),
        detail: e.to_string(),
    })
}

/// Map a stdout write error to [`ScanError::Io`].
fn stdout_err(e: &std::io::Error) -> ScanError {
    ScanError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    }
}

/// Write a human-readable summary of the scan results to `out`.
fn render<W: Write>(out: &mut W, s: &ScanSummary) -> Result<(), ScanError> {
    writeln!(out, "leaves:  {}", s.leaves_written.len()).map_err(|ref e| stdout_err(e))?;
    writeln!(out, "rollups: {}", s.rollups_written.len()).map_err(|ref e| stdout_err(e))?;
    let readme = if s.readme_written {
        "written"
    } else {
        "skipped"
    };
    writeln!(out, "readme:  {readme}").map_err(|ref e| stdout_err(e))
}

/// List the files that `ctx-scan` would summarize, one per line, without
/// calling the agent. Safe to run without `CTX_AGENT_CMD` set.
///
/// # Errors
///
/// [`ScanError::DirNotFound`] if `dir` is not a directory;
/// propagates walk failures.
pub fn list_targets<W: Write>(cli: &Cli, out: &mut W) -> Result<(), ScanError> {
    let base = validate_dir(&cli.dir)?;
    let files = walk_dir(&base)?;
    for f in &files {
        writeln!(out, "{f}").map_err(|ref e| stdout_err(e))?;
    }
    Ok(())
}

/// Validate `cli.dir`, run the full scan, and write results to `out`.
///
/// # Errors
///
/// [`ScanError::DirNotFound`] if `dir` is not a directory;
/// propagates walk, summarization, and README write failures.
pub fn dispatch<A: Agent, W: Write>(agent: &A, cli: &Cli, out: &mut W) -> Result<(), ScanError> {
    let base = validate_dir(&cli.dir)?;
    let summary = scan_run(&base, agent, cli.approve)?;
    render(out, &summary)
}
