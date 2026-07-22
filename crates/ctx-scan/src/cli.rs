//! Argument parsing and top-level dispatch.

use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use ctx_summarize::agent::Agent;
use ctx_summarize::runner::Models;

use crate::error::ScanError;
use crate::render::{render, render_pruned, render_staleness, staleness_message, stdout_err};
use crate::runner::{check_run, prune_run, scan_run, update_run};
use crate::walker::walk_dir;

/// Self-contained directory scanner — walks `<dir>` and maintains a
/// `.context/` summary tree (with content-hash change detection)
/// alongside the source files.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-scan",
    about = "Scan a directory and maintain a .context/ summary tree"
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

    /// Directory holding the prompt files, resolved against the process
    /// cwd (not the scanned directory).
    #[arg(long, default_value = "prompts")]
    prompts: String,

    /// Model the agent should call for leaf summaries. Required; no default.
    #[arg(long)]
    leaf_model: String,

    /// Model the agent should call for rollup summaries. Required; no
    /// default.
    #[arg(long)]
    rollup_model: String,

    /// Claude Code Stop-hook mode: check staleness and emit a
    /// report-only `systemMessage`. Never calls the agent —
    /// regeneration is a post-session concern (`ctx-run`'s refresh or
    /// a manual `--update`). Always exits 0 (fail-open).
    #[arg(long, conflicts_with_all = ["dry_run", "check", "update", "prune"])]
    stop_hook: bool,

    /// Delete orphaned `.context/` artifacts — derived summaries and
    /// sidecars whose source was deleted, moved, or scoped out — and
    /// sweep emptied mirror directories, without calling the agent.
    /// Does not require `CTX_AGENT_CMD`.
    #[arg(long, conflicts_with_all = ["dry_run", "check", "update"])]
    prune: bool,

    /// Which mutually exclusive run mode is active (default: full scan).
    #[command(flatten)]
    mode: Mode,
}

/// Mutually exclusive run modes; all off means a full scan.
#[derive(Debug, clap::Args)]
pub struct Mode {
    /// List the files that would be summarized and exit without calling
    /// the agent. Does not require `CTX_AGENT_CMD` to be set.
    #[arg(long, conflicts_with_all = ["check", "update"])]
    dry_run: bool,

    /// Recompute content hashes and report stale directories/leaves,
    /// missing artifacts, and orphaned artifacts without calling the
    /// agent or writing anything. Does not require `CTX_AGENT_CMD`.
    #[arg(long, conflicts_with = "update")]
    check: bool,

    /// Prune→check→rebuild: delete orphaned artifacts, regenerate only
    /// stale leaf summaries and rollups, then rewrite the hash sidecars.
    #[arg(long)]
    update: bool,
}

impl Cli {
    /// Whether dry-run mode is active.
    #[must_use]
    pub const fn dry_run(&self) -> bool {
        self.mode.dry_run
    }

    /// Whether check mode is active.
    #[must_use]
    pub const fn check(&self) -> bool {
        self.mode.check
    }

    /// Whether prune mode is active.
    #[must_use]
    pub const fn prune(&self) -> bool {
        self.prune
    }

    /// Whether update mode is active.
    #[must_use]
    pub const fn update(&self) -> bool {
        self.mode.update
    }

    /// Whether Stop-hook mode is active.
    #[must_use]
    pub const fn stop_hook(&self) -> bool {
        self.stop_hook
    }

    /// The target directory.
    #[must_use]
    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }

    /// The prompt-files directory (cwd-relative).
    #[must_use]
    pub fn prompts(&self) -> &str {
        &self.prompts
    }

    /// The model the agent should call for leaf summaries.
    #[must_use]
    pub fn leaf_model(&self) -> &str {
        &self.leaf_model
    }

    /// The model the agent should call for rollup summaries.
    #[must_use]
    pub fn rollup_model(&self) -> &str {
        &self.rollup_model
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

/// Recompute hashes and report staleness, without calling the agent.
///
/// # Errors
///
/// [`ScanError::DirNotFound`] if `dir` is not a directory; propagates
/// walk and hash failures.
pub fn check<W: Write>(cli: &Cli, out: &mut W) -> Result<(), ScanError> {
    let base = validate_dir(&cli.dir)?;
    let staleness = check_run(&base)?;
    render_staleness(out, &staleness)
}

/// Delete orphaned mirror artifacts, without calling the agent.
///
/// # Errors
///
/// [`ScanError::DirNotFound`] if `dir` is not a directory; propagates
/// walk and removal failures.
pub fn prune<W: Write>(cli: &Cli, out: &mut W) -> Result<(), ScanError> {
    let base = validate_dir(&cli.dir)?;
    let pruned = prune_run(&base)?;
    render_pruned(out, &pruned)
}

/// Claude Code Stop-hook mode: recompute staleness and report it as a
/// `systemMessage` — never regenerate.
///
/// The Stop event fires at the end of every turn, not at session end,
/// so billing here would race the session; regeneration is a
/// post-session concern with finalized state at both ends. A fresh
/// tree emits nothing.
///
/// # Errors
///
/// Propagates walk/hash failures and writer errors; the binary treats
/// any error as "say nothing" (fail-open).
pub fn stop_hook<W: Write>(dir: &std::path::Path, out: &mut W) -> Result<(), ScanError> {
    let base = validate_dir(dir)?;
    let staleness = check_run(&base)?;
    if staleness.is_fresh() {
        return Ok(());
    }
    let msg = staleness_message(dir, &staleness);
    let payload = serde_json::json!({ "systemMessage": msg });
    writeln!(out, "{payload}").map_err(|ref e| stdout_err(e))
}

/// Validate `cli.dir` and run either the full scan or (with `--update`)
/// the selective prune→check→rebuild, writing results to `out`.
///
/// # Errors
///
/// [`ScanError::DirNotFound`] if `dir` is not a directory;
/// propagates walk, summarization, hash, and README write failures.
pub fn dispatch<A: Agent, W: Write>(agent: &A, cli: &Cli, out: &mut W) -> Result<(), ScanError> {
    let base = validate_dir(&cli.dir)?;
    let models = Models {
        leaf: cli.leaf_model(),
        rollup: cli.rollup_model(),
    };
    if cli.update() {
        let staleness = update_run(&base, cli.prompts(), agent, &models, cli.approve)?;
        return render_staleness(out, &staleness);
    }
    let summary = scan_run(&base, cli.prompts(), agent, &models, cli.approve)?;
    render(out, &summary)
}
