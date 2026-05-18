//! Argv parsing and JSON rendering — the thin `cli` layer.

use std::io::Write;

use clap::{Parser, Subcommand};

use crate::agent::Agent;
use crate::cache::paths_from_cache;
use crate::error::SummError;
use crate::fs::Fs;
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
}

/// Target-selection mode.
#[derive(Debug, Subcommand)]
enum Command {
    /// Summarize the `paths_written` of a task cache.
    FromCache {
        /// Task identifier whose cache to read.
        #[arg(long)]
        task_id: String,
    },
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
/// Propagates cache, prompt, filesystem, path, agent, or encoding errors.
pub fn dispatch<F: Fs, A: Agent, W: Write>(
    fs: &F,
    agent: &A,
    cli: &Cli,
    out: &mut W,
) -> Result<(), SummError> {
    let targets = match &cli.command {
        Command::FromCache { task_id } => paths_from_cache(fs, task_id)?,
        Command::Paths { paths } => paths.clone(),
    };
    let summary = runner::run(fs, agent, &cli.prompts, &targets)?;
    serde_json::to_writer_pretty(&mut *out, &summary).map_err(|e| SummError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    })?;
    writeln!(out).map_err(|e| SummError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    })
}
