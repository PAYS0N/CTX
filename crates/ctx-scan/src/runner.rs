//! Scan orchestration: walk, summarize, hash, write README.
//!
//! [`scan_run`] is the full pipeline (summarize everything);
//! [`check_run`] recomputes the hash tree and reports staleness without
//! any model call; [`update_run`] is check→rebuild: it regenerates only
//! the stale leaf summaries and rollups, then rewrites the hash
//! sidecars. [`summarize`] is the inner step, generic over [`Fs`] and
//! [`Agent`], and is usable directly in tests with in-memory fakes.

use std::path::Path;

use ctx_summarize::agent::Agent;
use ctx_summarize::fs::Fs;
use ctx_summarize::runner as summ;

use crate::error::ScanError;
use crate::fs::ScanFs;
use crate::hash::{self, Staleness};
use crate::readme::write_readme;
use crate::walker::walk_dir;

/// Outcome of a full scan run.
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
    summ::scope_check(targets.len(), approve)?;
    let s = summ::run(fs, agent, "prompts", targets)?;
    write_readme(fs)?;
    Ok(ScanSummary {
        leaves_written: s.leaves_written,
        rollups_written: s.rollups_written,
        readme_written: true,
    })
}

/// Full pipeline: walk `base`, summarize everything, write the hash
/// sidecars and the README.
///
/// # Errors
///
/// Propagates walk, scope, summarization, hash, and README failures.
pub fn scan_run<A: Agent>(base: &Path, agent: &A, approve: bool) -> Result<ScanSummary, ScanError> {
    let fs = ScanFs::new(base.to_path_buf());
    let targets = walk_dir(base)?;
    let summary = summarize(&fs, agent, &targets, approve)?;
    hash::store(&fs, &hash::compute(base, &targets)?)?;
    Ok(summary)
}

/// Recompute the hash tree for `base` and report staleness against the
/// stored sidecars. No model is called; safe to run anywhere.
///
/// # Errors
///
/// Propagates walk and hash-computation failures.
pub fn check_run(base: &Path) -> Result<Staleness, ScanError> {
    let fs = ScanFs::new(base.to_path_buf());
    let targets = walk_dir(base)?;
    let current = hash::compute(base, &targets)?;
    let stored = hash::load_stored(&fs, &current);
    Ok(hash::diff(&current, &stored))
}

/// Regenerate exactly what `stale` names: changed leaves, orphan leaf
/// removal, then stale rollups (already deepest-first).
fn regenerate<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    stale: &Staleness,
    approve: bool,
) -> Result<(), ScanError> {
    summ::scope_check(stale.changed_files.len(), approve)?;
    let prompts = summ::load_prompts(fs, "prompts")?;
    for f in &stale.changed_files {
        summ::summarize_leaf(fs, agent, &prompts, f)?;
    }
    for leaf in &stale.orphan_leaves {
        fs.remove(leaf)?;
    }
    for d in &stale.stale_dirs {
        summ::summarize_rollup(fs, agent, &prompts, d)?;
    }
    Ok(())
}

/// Check→rebuild: recompute hashes and regenerate only stale summaries,
/// then persist the fresh hash tree. Returns what was regenerated (a
/// fresh tree regenerates nothing and is reported as such).
///
/// # Errors
///
/// Propagates walk, scope, summarization, and hash failures.
pub fn update_run<A: Agent>(base: &Path, agent: &A, approve: bool) -> Result<Staleness, ScanError> {
    let fs = ScanFs::new(base.to_path_buf());
    let targets = walk_dir(base)?;
    let current = hash::compute(base, &targets)?;
    let stored = hash::load_stored(&fs, &current);
    let stale = hash::diff(&current, &stored);
    if stale.is_fresh() {
        return Ok(stale);
    }
    regenerate(&fs, agent, &stale, approve)?;
    hash::store(&fs, &current)?;
    write_readme(&fs)?;
    Ok(stale)
}
