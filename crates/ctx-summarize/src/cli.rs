//! Argv parsing and JSON rendering — the thin `cli` layer.

use std::io::Write;

use clap::{Parser, Subcommand};

use crate::agent::Agent;
use crate::error::SummError;
use crate::fs::Fs;
use crate::progress::NoProgress;
use crate::runner;

/// Leaf-up context-tree summarization runner.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-summarize",
    about = "Summarize modified paths leaf-up via the prompt files"
)]
pub struct Cli {
    /// Which target set to summarize.
    #[command(subcommand)]
    command: Command,
    /// Directory holding the prompt files (repo-relative).
    #[arg(long, default_value = "prompts", global = true)]
    prompts: String,
    /// Permit a run over more than `MAX_TARGETS` files (cost/blast-radius
    /// guard; without this an over-large set is refused).
    #[arg(long, global = true)]
    approve: bool,
}

/// Target-selection mode.
#[derive(Debug, Subcommand)]
enum Command {
    /// Summarize an explicit list of source paths.
    Paths {
        /// Repo-relative source paths.
        paths: Vec<String>,
    },
}

/// Resolve targets, run the summarizer, write the JSON summary to `out`.
///
/// # Errors
///
/// Propagates prompt, filesystem, path, agent, or encoding errors.
pub fn dispatch<F: Fs, A: Agent, W: Write>(
    fs: &F,
    agent: &A,
    cli: &Cli,
    out: &mut W,
) -> Result<(), SummError> {
    let Command::Paths { paths } = &cli.command;
    let targets = paths.clone();
    runner::scope_check(targets.len(), cli.approve)?;
    let summary = runner::run(fs, agent, &cli.prompts, &targets, &NoProgress)?;
    serde_json::to_writer_pretty(&mut *out, &summary).map_err(|e| SummError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    })?;
    writeln!(out).map_err(|e| SummError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    })
}
