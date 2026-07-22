//! Pure summarization orchestration over [`Fs`] + [`Agent`].
//!
//! Leaf summaries first, then rollups walked leaf-up (deepest dir first, root last). `intent.md` is never written.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::agent::Agent;
use crate::cpath;
use crate::error::SummError;
use crate::fs::Fs;
use crate::progress::Progress;
use crate::rollup_input::{assemble_rollup_input, dir_label};

/// Prompt-file contents, loaded once per run (never embedded in code).
pub struct Prompts {
    /// `summarizer-leaf.md` contents.
    leaf: String,
    /// `summarizer-rollup.md` contents.
    rollup: String,
}

/// Leaf and rollup model choice for one summarization run. Bundled so
/// callers threading both through several layers pass one argument
/// instead of two (see ADR-057).
pub struct Models<'a> {
    /// Model for leaf summaries.
    pub leaf: &'a str,
    /// Model for rollup summaries.
    pub rollup: &'a str,
}

/// What a run produced.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// Leaf `.ctx` paths written, in processing order.
    pub leaves_written: Vec<String>,
    /// `rollup.ctx` paths written, leaf-up order.
    pub rollups_written: Vec<String>,
}

/// Load the two prompt files from `prompts_dir` (repo-relative).
///
/// # Errors
///
/// [`SummError::MissingPrompt`] if either file is unreadable.
pub fn load_prompts<F: Fs>(fs: &F, prompts_dir: &str) -> Result<Prompts, SummError> {
    let leaf_path = format!("{prompts_dir}/summarizer-leaf.md");
    let rollup_path = format!("{prompts_dir}/summarizer-rollup.md");
    let leaf = fs
        .read(&leaf_path)
        .map_err(|_| SummError::MissingPrompt(leaf_path))?;
    let rollup = fs
        .read(&rollup_path)
        .map_err(|_| SummError::MissingPrompt(rollup_path))?;
    Ok(Prompts { leaf, rollup })
}

/// Summarize one source file into its leaf `.ctx`; returns the `.ctx` path.
///
/// # Errors
///
/// [`SummError::PathEscape`] for an unsafe path, [`SummError::AccessDenied`]
/// for a gated target; propagates filesystem and agent failures.
pub fn summarize_leaf<F: Fs, A: Agent, P: Progress>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    src: &str,
    model: &str,
    progress: &P,
) -> Result<String, SummError> {
    cpath::validate_rel(src)?;
    if let Some(reason) = ctx_core::access::deny_reason(src, fs.is_ignored(src)?) {
        return Err(SummError::AccessDenied {
            path: src.to_owned(),
            reason: reason.to_owned(),
        });
    }
    let contents = fs.read(src)?;
    let user = format!("SOURCE_PATH: {src}\n\n{contents}");
    progress.leaf(src);
    let output = agent.complete(&prompts.leaf, &user, model)?;
    let dest = cpath::leaf_ctx(src);
    fs.write(&dest, &output)?;
    Ok(dest)
}

/// Summarize one directory into its `rollup.ctx`; returns the path.
///
/// # Errors
///
/// Propagates filesystem and agent failures.
pub fn summarize_rollup<F: Fs, A: Agent, P: Progress>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    dir: &str,
    model: &str,
    progress: &P,
) -> Result<String, SummError> {
    let user = assemble_rollup_input(fs, dir)?;
    progress.rollup(dir_label(dir));
    let output = agent.complete(&prompts.rollup, &user, model)?;
    let dest = cpath::rollup_of(dir);
    fs.write(&dest, &output)?;
    Ok(dest)
}

/// Affected directories for `targets`, ordered leaf-up (deepest first,
/// repo root last).
fn affected_dirs(targets: &[String]) -> Vec<String> {
    let mut set: BTreeSet<String> = BTreeSet::new();
    for t in targets {
        for d in cpath::ancestor_dirs(t) {
            set.insert(d);
        }
    }
    let mut dirs: Vec<String> = set.into_iter().collect();
    dirs.sort_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));
    dirs
}

/// Directory depth (`""` = 0).
fn depth(dir: &str) -> usize {
    if dir.is_empty() {
        0
    } else {
        dir.split('/').count()
    }
}

/// Targets a run may touch before requiring explicit `--approve`.
/// Guards against an accidental whole-repo (cost/blast-radius) run.
pub const MAX_TARGETS: usize = 50;

/// Gate an over-large run unless explicitly approved.
///
/// # Errors
///
/// [`SummError::ScopeTooLarge`] if `count` exceeds [`MAX_TARGETS`] and
/// `approve` is false.
pub const fn scope_check(count: usize, approve: bool) -> Result<(), SummError> {
    if count > MAX_TARGETS && !approve {
        return Err(SummError::ScopeTooLarge {
            count,
            max: MAX_TARGETS,
        });
    }
    Ok(())
}

/// Run the full leaf-up summarization over `targets`, loading prompts
/// from `prompts_dir` via `fs`.
///
/// # Errors
///
/// Propagates prompt, filesystem, path, and agent failures.
pub fn run<F: Fs, A: Agent, P: Progress>(
    fs: &F,
    agent: &A,
    prompts_dir: &str,
    targets: &[String],
    models: &Models,
    progress: &P,
) -> Result<Summary, SummError> {
    let prompts = load_prompts(fs, prompts_dir)?;
    run_with_prompts(fs, agent, &prompts, targets, models, progress)
}

/// Run the full leaf-up summarization over `targets` with already-loaded
/// `prompts`.
///
/// `fs` is the scan/write filesystem; `prompts` may come from a different
/// root (e.g. the process cwd), decoupling prompt source from the tree.
///
/// # Errors
///
/// Propagates filesystem, path, and agent failures.
pub fn run_with_prompts<F: Fs, A: Agent, P: Progress>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    targets: &[String],
    models: &Models,
    progress: &P,
) -> Result<Summary, SummError> {
    let leaves_written = write_leaves(fs, agent, prompts, targets, models.leaf, progress)?;
    let rollups_written = write_rollups(fs, agent, prompts, targets, models.rollup, progress)?;
    Ok(Summary {
        leaves_written,
        rollups_written,
    })
}

/// Write leaf `.ctx` files for every target, in order.
///
/// # Errors
///
/// Propagates filesystem, path, and agent failures.
fn write_leaves<F: Fs, A: Agent, P: Progress>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    targets: &[String],
    model: &str,
    progress: &P,
) -> Result<Vec<String>, SummError> {
    let mut leaves_written = Vec::new();
    for src in targets {
        leaves_written.push(summarize_leaf(fs, agent, prompts, src, model, progress)?);
    }
    Ok(leaves_written)
}

/// Write `rollup.ctx` files for every directory affected by `targets`,
/// leaf-up.
///
/// # Errors
///
/// Propagates filesystem, path, and agent failures.
fn write_rollups<F: Fs, A: Agent, P: Progress>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    targets: &[String],
    model: &str,
    progress: &P,
) -> Result<Vec<String>, SummError> {
    let mut rollups_written = Vec::new();
    for dir in affected_dirs(targets) {
        rollups_written.push(summarize_rollup(fs, agent, prompts, &dir, model, progress)?);
    }
    Ok(rollups_written)
}
