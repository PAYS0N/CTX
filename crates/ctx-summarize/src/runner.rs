//! Pure summarization orchestration over [`Fs`] + [`Agent`].
//!
//! Leaf summaries first, then rollups walked leaf-up (deepest directory
//! first, repo root last). `intent.md` is never written.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::agent::Agent;
use crate::cpath;
use crate::error::SummError;
use crate::fs::Fs;

/// Prompt-file contents, loaded once per run (never embedded in code).
pub struct Prompts {
    /// `summarizer-leaf.md` contents.
    leaf: String,
    /// `summarizer-rollup.md` contents.
    rollup: String,
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
pub fn summarize_leaf<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    src: &str,
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
    let output = agent.complete(&prompts.leaf, &user)?;
    let dest = cpath::leaf_ctx(src);
    fs.write(&dest, &output)?;
    Ok(dest)
}

/// Display form of a directory (`""` -> `.`).
const fn dir_label(dir: &str) -> &str {
    if dir.is_empty() {
        "."
    } else {
        dir
    }
}

/// Whether `name` has a (case-insensitive) `.ctx` extension.
fn has_ctx_ext(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ctx"))
}

/// Assemble the rollup prompt's user input for `dir` from current
/// children summaries and the directory's `intent.md`.
fn assemble_rollup_input<F: Fs>(fs: &F, dir: &str) -> Result<String, SummError> {
    let mut buf = format!("DIR_PATH: {}\n", dir_label(dir));
    let intent_path = cpath::intent_of(dir);
    let intent = if fs.exists(&intent_path) {
        fs.read(&intent_path)?
    } else {
        String::new()
    };
    buf.push_str("\n--- intent.md ---\n");
    buf.push_str(if intent.is_empty() {
        "(none)\n"
    } else {
        &intent
    });
    let cdir = cpath::context_dir_of(dir);
    for name in fs.list_dir(&cdir)? {
        if name == "rollup.ctx" || name == "intent.md" {
            continue;
        }
        let child_rel = format!("{cdir}/{name}");
        if has_ctx_ext(&name) {
            push_section(&mut buf, &name, &fs.read(&child_rel)?);
        } else if fs.exists(&format!("{child_rel}/rollup.ctx")) {
            let sub = fs.read(&format!("{child_rel}/rollup.ctx"))?;
            push_section(&mut buf, &format!("{name}/rollup.ctx"), &sub);
        }
    }
    Ok(buf)
}

/// Append a labeled section to the rollup input buffer.
fn push_section(buf: &mut String, label: &str, body: &str) {
    buf.push_str("\n--- ");
    buf.push_str(label);
    buf.push_str(" ---\n");
    buf.push_str(body);
    if !body.ends_with('\n') {
        buf.push('\n');
    }
}

/// Summarize one directory into its `rollup.ctx`; returns the path.
///
/// # Errors
///
/// Propagates filesystem and agent failures.
pub fn summarize_rollup<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    dir: &str,
) -> Result<String, SummError> {
    let user = assemble_rollup_input(fs, dir)?;
    let output = agent.complete(&prompts.rollup, &user)?;
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
pub fn run<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    prompts_dir: &str,
    targets: &[String],
) -> Result<Summary, SummError> {
    let prompts = load_prompts(fs, prompts_dir)?;
    run_with_prompts(fs, agent, &prompts, targets)
}

/// Run the full leaf-up summarization over `targets` with already-loaded
/// `prompts`.
///
/// `fs` is the scan/write filesystem; `prompts` may have been loaded from
/// a different root (e.g. the process cwd) so the prompt source stays
/// decoupled from the tree being summarized.
///
/// # Errors
///
/// Propagates filesystem, path, and agent failures.
pub fn run_with_prompts<F: Fs, A: Agent>(
    fs: &F,
    agent: &A,
    prompts: &Prompts,
    targets: &[String],
) -> Result<Summary, SummError> {
    let mut leaves_written = Vec::new();
    for src in targets {
        leaves_written.push(summarize_leaf(fs, agent, prompts, src)?);
    }
    let mut rollups_written = Vec::new();
    for dir in affected_dirs(targets) {
        rollups_written.push(summarize_rollup(fs, agent, prompts, &dir)?);
    }
    Ok(Summary {
        leaves_written,
        rollups_written,
    })
}
