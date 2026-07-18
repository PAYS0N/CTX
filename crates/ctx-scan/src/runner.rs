//! Scan orchestration: walk, summarize, hash, write README.
//!
//! [`scan_run`] is the full pipeline (prune orphans, summarize
//! everything); [`check_run`] recomputes the hash tree and reports
//! staleness — including orphaned mirror artifacts — without any model
//! call; [`update_run`] is prune→check→rebuild: it deletes orphaned
//! artifacts, regenerates only the stale leaf summaries and rollups,
//! then rewrites the hash sidecars; [`prune_run`] is the pruning step
//! alone (never a model call). [`summarize`] is the inner step, generic
//! over [`Fs`] and [`Agent`], and is usable directly in tests with
//! in-memory fakes.

use std::path::Path;

use ctx_summarize::agent::Agent;
use ctx_summarize::fs::{Fs, StdFs};
use ctx_summarize::runner::{self as summ, Prompts};

use crate::error::ScanError;
use crate::hash::{self, Staleness};
use crate::readme::write_readme;
use crate::reconcile;
use crate::walker::walk_dir;

/// Load the prompt files from `prompts_dir`, resolved against the process
/// cwd — deliberately independent of the scan target, which need not carry
/// CTX's prompts.
fn load_prompts(prompts_dir: &str) -> Result<Prompts, ScanError> {
    let cwd = std::env::current_dir().map_err(|e| ScanError::Io {
        path: ".".to_owned(),
        detail: e.to_string(),
    })?;
    Ok(summ::load_prompts(&StdFs::new(cwd), prompts_dir)?)
}

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

/// Summarize `targets` leaf-up with pre-loaded `prompts`, writing the
/// `.context/` tree and README through `fs` (the scan-target filesystem).
///
/// # Errors
///
/// Propagates scope, summarization, and README write failures.
pub fn summarize<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    targets: &[String],
    approve: bool,
) -> Result<ScanSummary, ScanError> {
    summ::scope_check(targets.len(), approve)?;
    let s = summ::run_with_prompts(fs, agent, prompts, targets)?;
    write_readme(fs)?;
    Ok(ScanSummary {
        leaves_written: s.leaves_written,
        rollups_written: s.rollups_written,
        readme_written: true,
    })
}

/// Full pipeline: walk `base`, prune orphaned mirror artifacts,
/// summarize everything, write the hash sidecars and the README.
/// Prompts are loaded from `prompts_dir` (cwd relative), not from
/// `base`.
///
/// # Errors
///
/// Propagates walk, scope, summarization, hash, and README failures.
pub fn scan_run<A: Agent>(
    base: &Path,
    prompts_dir: &str,
    agent: &A,
    approve: bool,
) -> Result<ScanSummary, ScanError> {
    let fs = StdFs::new(base.to_path_buf());
    let prompts = load_prompts(prompts_dir)?;
    let targets = walk_dir(base)?;
    reconcile::prune(base, &reconcile::find_orphan_artifacts(&fs, &targets)?)?;
    let summary = summarize(&fs, agent, &prompts, &targets, approve)?;
    hash::store(&fs, &hash::compute(base, &targets)?)?;
    Ok(summary)
}

/// Recompute the hash tree for `base` and report staleness against the
/// stored sidecars.
///
/// Also reports integrity gaps in both directions (missing artifacts,
/// orphaned artifacts). No model is called and nothing is written; safe
/// to run anywhere.
///
/// # Errors
///
/// Propagates walk and hash-computation failures.
pub fn check_run(base: &Path) -> Result<Staleness, ScanError> {
    let fs = StdFs::new(base.to_path_buf());
    let targets = walk_dir(base)?;
    let current = hash::compute(base, &targets)?;
    let stored = hash::load_stored(&fs, &current);
    let mut stale = hash::diff(&current, &stored);
    hash::record_missing_artifacts(&fs, &current, &mut stale);
    let scan = reconcile::find_orphan_artifacts(&fs, &targets)?;
    stale.orphan_artifacts = scan.artifacts_excluding(&stale.orphan_leaves);
    Ok(stale)
}

/// Prune orphaned mirror artifacts and sweep emptied mirror
/// directories, with no model call and no regeneration. Returns the
/// pruned `.context/...` paths.
///
/// # Errors
///
/// Propagates walk and removal failures.
pub fn prune_run(base: &Path) -> Result<Vec<String>, ScanError> {
    let fs = StdFs::new(base.to_path_buf());
    let targets = walk_dir(base)?;
    let scan = reconcile::find_orphan_artifacts(&fs, &targets)?;
    reconcile::prune(base, &scan)?;
    Ok(scan.artifacts)
}

/// Regenerate exactly what `stale` names: changed leaves, orphan leaf
/// removal, then stale rollups (already deepest-first).
fn regenerate<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    stale: &Staleness,
    approve: bool,
) -> Result<(), ScanError> {
    summ::scope_check(stale.changed_files.len(), approve)?;
    for f in &stale.changed_files {
        summ::summarize_leaf(fs, agent, prompts, f)?;
    }
    for leaf in &stale.orphan_leaves {
        fs.remove(leaf)?;
    }
    for d in &stale.stale_dirs {
        summ::summarize_rollup(fs, agent, prompts, d)?;
    }
    Ok(())
}

/// Prune→check→rebuild: delete orphaned mirror artifacts, regenerate
/// only stale summaries, then persist the fresh hash tree.
///
/// Pruning is pure filesystem work, done before rollups can inhale the
/// orphans; a tree that is stale only by orphan artifacts prunes
/// without loading prompts or touching the model. Returns what was
/// pruned/regenerated (a fresh tree does neither and is reported as
/// such).
///
/// # Errors
///
/// Propagates walk, scope, summarization, prune, and hash failures.
pub fn update_run<A: Agent>(
    base: &Path,
    prompts_dir: &str,
    agent: &A,
    approve: bool,
) -> Result<Staleness, ScanError> {
    let fs = StdFs::new(base.to_path_buf());
    let targets = walk_dir(base)?;
    let current = hash::compute(base, &targets)?;
    let stored = hash::load_stored(&fs, &current);
    let mut stale = hash::diff(&current, &stored);
    let scan = reconcile::find_orphan_artifacts(&fs, &targets)?;
    stale.orphan_artifacts = scan.artifacts_excluding(&stale.orphan_leaves);
    if stale.is_fresh() {
        return Ok(stale);
    }
    reconcile::prune(base, &scan)?;
    if stale.needs_regeneration() {
        let prompts = load_prompts(prompts_dir)?;
        regenerate(&fs, agent, &prompts, &stale, approve)?;
        hash::store(&fs, &current)?;
        write_readme(&fs)?;
    }
    Ok(stale)
}
