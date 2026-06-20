//! Scan orchestration: walk, summarize leaf-up, write README.
//!
//! [`scan_run`] is the primary entry point for production use.
//! [`summarize`] is the inner step, generic over [`Fs`] and [`Agent`],
//! and is usable directly in tests with in-memory fakes.

use std::path::Path;

use ctx_summarize::agent::Agent;
use ctx_summarize::fs::Fs;

use crate::error::ScanError;
use crate::fs::ScanFs;
use crate::readme::write_readme;
use crate::walker::walk_dir;

/// Outcome of a scan run.
#[derive(Debug)]
pub struct ScanSummary {
    /// Leaf `.ctx` files written, in processing order.
    pub leaves_written: Vec<String>,
    /// `rollup.ctx` files written, leaf-up order.
    pub rollups_written: Vec<String>,
    /// Whether `.context/README.md` was written.
    pub readme_written: bool,
}

/// Summarize `targets` leaf-up and write the README using `fs` and `agent`.
///
/// Prompts are read via `fs` — a [`ScanFs`] intercepts the well-known paths
/// and returns embedded constants; a seeded in-memory fake works the same
/// way in tests.
///
/// # Errors
///
/// Propagates scope, summarization, and README write failures.
pub fn summarize<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    targets: &[String],
    approve: bool,
) -> Result<ScanSummary, ScanError> {
    ctx_summarize::runner::scope_check(targets.len(), approve)?;
    let s = ctx_summarize::runner::run(fs, agent, "prompts", targets)?;
    write_readme(fs)?;
    Ok(ScanSummary {
        leaves_written: s.leaves_written,
        rollups_written: s.rollups_written,
        readme_written: true,
    })
}

/// Full pipeline: walk `base`, summarize the results, write the README.
///
/// # Errors
///
/// Propagates walk, scope, summarization, and README write failures.
pub fn scan_run<A: Agent>(base: &Path, agent: &A, approve: bool) -> Result<ScanSummary, ScanError> {
    let fs = ScanFs::new(base.to_path_buf());
    let targets = walk_dir(base)?;
    summarize(&fs, agent, &targets, approve)
}
