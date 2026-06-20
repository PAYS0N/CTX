//! Argument parsing and top-level dispatch.

use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use ctx_summarize::agent::Agent;

use crate::error::ScanError;
use crate::runner::{scan_run, ScanSummary};

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

/// Validate `cli.dir`, run the full scan, and write results to `out`.
///
/// # Errors
///
/// [`ScanError::DirNotFound`] if `dir` is not a directory;
/// propagates walk, summarization, and README write failures.
pub fn dispatch<A: Agent, W: Write>(agent: &A, cli: &Cli, out: &mut W) -> Result<(), ScanError> {
    if !cli.dir.is_dir() {
        return Err(ScanError::DirNotFound(cli.dir.clone()));
    }
    let base = cli.dir.canonicalize().map_err(|e| ScanError::Io {
        path: cli.dir.to_string_lossy().into_owned(),
        detail: e.to_string(),
    })?;
    let summary = scan_run(&base, agent, cli.approve)?;
    render(out, &summary)
}
