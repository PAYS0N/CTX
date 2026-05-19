//! Argv parsing and output rendering — the thin `cli` layer.
//!
//! Knows nothing about enforcement logic; it maps parsed arguments onto
//! [`crate::enforce`] calls and renders results to an injected writer
//! (never `println!`, so `clippy::print_stdout` need not be excepted).

use std::io::Write;

use clap::{Parser, Subcommand};

use crate::enforce::{self, NoopSummarizer};
use crate::env::Env;
use crate::error::CtxError;

/// The context-tree access broker.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-access",
    about = "Sanctioned path to source for agent tasks"
)]
pub struct Cli {
    /// Which operation to perform.
    #[command(subcommand)]
    command: Command,
}

/// One ctx-access operation.
#[derive(Debug, Subcommand)]
enum Command {
    /// Create a fresh per-task cache.
    InitTask {
        /// Task identifier (`[A-Za-z0-9._-]`).
        #[arg(long)]
        task_id: String,
        /// Reclaim an existing cache instead of failing.
        #[arg(long)]
        force: bool,
    },
    /// Serve the unserved prefix of a path's context chain.
    Read {
        /// Repo-relative source path to read.
        path: String,
        /// Task identifier.
        #[arg(long)]
        task_id: String,
        /// Stop before source (serve ancestors + leaf only).
        #[arg(long)]
        shallow: bool,
    },
    /// Write source; requires a prior non-shallow read of the same path.
    Write {
        /// Repo-relative source path to write.
        path: String,
        /// New file contents.
        content: String,
        /// Task identifier.
        #[arg(long)]
        task_id: String,
    },
    /// List a directory; requires its rollup to have been served.
    List {
        /// Repo-relative directory to list.
        path: String,
        /// Task identifier.
        #[arg(long)]
        task_id: String,
    },
    /// Finalize a task: audit, write report, delete cache.
    EndTask {
        /// Task identifier.
        #[arg(long)]
        task_id: String,
    },
    /// Print (and re-materialize) the deny-by-default manifest of
    /// readable source paths.
    Manifest {
        /// Task identifier.
        #[arg(long)]
        task_id: String,
    },
}

/// Wrap a writer error as [`CtxError::Io`].
fn out_err(e: &std::io::Error) -> CtxError {
    CtxError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    }
}

/// Render a `read` response: a header then body per served node.
fn render_read<W: Write>(resp: &enforce::ReadResponse, out: &mut W) -> Result<(), CtxError> {
    for node in &resp.nodes {
        writeln!(out, "=== {} [{}] ===", node.id, node.kind.label())
            .and_then(|()| writeln!(out, "{}", node.body))
            .map_err(|e| out_err(&e))?;
    }
    Ok(())
}

/// Handle `init-task`.
fn cmd_init<E: Env, W: Write>(
    env: &E,
    task_id: &str,
    force: bool,
    out: &mut W,
) -> Result<(), CtxError> {
    enforce::init_task(env, task_id, force)?;
    writeln!(out, "initialized task {task_id}").map_err(|e| out_err(&e))
}

/// Handle `read`.
fn cmd_read<E: Env, W: Write>(
    env: &E,
    task_id: &str,
    path: &str,
    shallow: bool,
    out: &mut W,
) -> Result<(), CtxError> {
    let resp = enforce::read(env, task_id, path, shallow)?;
    render_read(&resp, out)
}

/// Handle `write`.
fn cmd_write<E: Env, W: Write>(
    env: &E,
    task_id: &str,
    path: &str,
    content: &str,
    out: &mut W,
) -> Result<(), CtxError> {
    enforce::write(env, task_id, path, content.as_bytes())?;
    writeln!(out, "wrote {path}").map_err(|e| out_err(&e))
}

/// Handle `list`.
fn cmd_list<E: Env, W: Write>(
    env: &E,
    task_id: &str,
    path: &str,
    out: &mut W,
) -> Result<(), CtxError> {
    for name in enforce::list(env, task_id, path)? {
        writeln!(out, "{name}").map_err(|e| out_err(&e))?;
    }
    Ok(())
}

/// Handle `end-task`.
fn cmd_end<E: Env, W: Write>(env: &E, task_id: &str, out: &mut W) -> Result<(), CtxError> {
    let report = enforce::end_task(env, task_id, &NoopSummarizer)?;
    writeln!(
        out,
        "ended task {} ({} divergences)",
        task_id,
        report.divergences.len()
    )
    .map_err(|e| out_err(&e))
}

/// Handle `manifest`.
fn cmd_manifest<E: Env, W: Write>(env: &E, task_id: &str, out: &mut W) -> Result<(), CtxError> {
    let text = crate::manifest::build(env, task_id)?;
    write!(out, "{text}").map_err(|e| out_err(&e))
}

/// Execute the parsed command against `env`, rendering to `out`.
///
/// # Errors
///
/// Propagates any [`CtxError`] from the enforcement layer or writer.
pub fn dispatch<E: Env, W: Write>(env: &E, cli: Cli, out: &mut W) -> Result<(), CtxError> {
    match cli.command {
        Command::InitTask { task_id, force } => cmd_init(env, &task_id, force, out),
        Command::Read {
            path,
            task_id,
            shallow,
        } => cmd_read(env, &task_id, &path, shallow, out),
        Command::Write {
            path,
            content,
            task_id,
        } => cmd_write(env, &task_id, &path, &content, out),
        Command::List { path, task_id } => cmd_list(env, &task_id, &path, out),
        Command::EndTask { task_id } => cmd_end(env, &task_id, out),
        Command::Manifest { task_id } => cmd_manifest(env, &task_id, out),
    }
}
