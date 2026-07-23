//! Argv parsing and the thin dispatch into [`runner`].

use std::io::Write;

use clap::{Parser, Subcommand};
use ctx_core::status_table::render_row;

use crate::error::StatusError;
use crate::fs::Fs;
use crate::runner::{self, Paths};

/// Structured backlog access: list the priority-sorted store, or append
/// one additive item to it.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-status",
    about = "List or append to the docs/STATUS.md backlog store"
)]
pub struct Cli {
    /// Override the JSON store path (cwd-relative).
    #[arg(long, default_value = "docs/status.json")]
    store: String,
    /// Override the rendered markdown view path (cwd-relative).
    #[arg(long, default_value = "docs/STATUS.md")]
    status: String,
    /// The subcommand to run.
    #[command(subcommand)]
    command: Command,
}

/// The one read (`List`) and two write surfaces (`AddTask`, `DeleteTask`)
/// an agent is documented to use; `Migrate` is the one-time bootstrap from
/// the pre-existing hand-maintained table and is hidden from `--help`.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the backlog, priority-sorted.
    List,
    /// Append a new backlog item. Additive only: never edits or reorders
    /// existing rows.
    ///
    /// --task, --impact, and --difficulty are all required. Example:
    /// ctx-status add-task "fix the thing" --task "fix thing" --impact
    /// medium --difficulty easy
    AddTask {
        /// The description column.
        description: String,
        /// Required. The impact column: high, medium, or low.
        #[arg(long)]
        impact: String,
        /// Required. The difficulty column: easy, medium, or hard.
        #[arg(long)]
        difficulty: String,
        /// Required. The task (short title) column — a distinct short
        /// title, not the full `description` again.
        #[arg(long)]
        task: String,
    },
    /// Remove a backlog item by id (shown as the leading column of
    /// `ctx-status list`'s output).
    DeleteTask {
        /// The id of the row to remove.
        id: u64,
    },
    /// One-time bootstrap: seed the store from an existing
    /// STATUS.md-shaped markdown table. Refuses to run against a store
    /// that already has rows.
    #[command(hide = true)]
    Migrate {
        /// Path (cwd-relative) to the markdown table to migrate.
        source: String,
    },
    /// Re-render `docs/STATUS.md` from the current store, without
    /// changing the store. Use after hand-editing the JSON store (the
    /// operator's edit/reorder/delete authority per the store's own
    /// design) to bring the view back in sync.
    #[command(hide = true)]
    Render,
}

impl Cli {
    /// The resolved store/view paths for this invocation.
    fn paths(&self) -> Paths {
        Paths {
            store: self.store.clone(),
            status_md: self.status.clone(),
        }
    }
}

/// Write one line, mapping any I/O failure to [`StatusError::Io`].
fn line<W: Write>(out: &mut W, text: &str) -> Result<(), StatusError> {
    writeln!(out, "{text}").map_err(|e| StatusError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    })
}

/// Print every backlog row, one table line each, prefixed with its id —
/// the handle `delete-task` takes. Ids are `ctx-status list`'s own
/// surfacing; `docs/STATUS.md` never shows them (its 4-column shape is
/// shared with `ctx-brief`).
fn print_list<F: Fs, W: Write>(fs: &F, paths: &Paths, out: &mut W) -> Result<(), StatusError> {
    for task in runner::list(fs, paths)? {
        line(out, &format!("| {} {}", task.id, render_row(&task.row)))?;
    }
    Ok(())
}

/// Run the parsed command against `fs`, writing output to `out`.
///
/// # Errors
///
/// Propagates every runner failure; a stdout write failure is reported as
/// [`StatusError::Io`].
pub fn dispatch<F: Fs, W: Write>(fs: &F, cli: &Cli, out: &mut W) -> Result<(), StatusError> {
    let paths = cli.paths();
    match &cli.command {
        Command::List => print_list(fs, &paths, out),
        Command::AddTask {
            description,
            impact,
            difficulty,
            task,
        } => {
            let id = runner::add_task(fs, &paths, task, description, impact, difficulty)?;
            line(out, &format!("added: [{id}] {task}"))
        },
        Command::DeleteTask { id } => {
            runner::delete_task(fs, &paths, *id)?;
            line(out, &format!("deleted: [{id}]"))
        },
        Command::Migrate { source } => {
            let count = runner::migrate(fs, &paths, source)?;
            line(out, &format!("migrated {count} rows from {source}"))
        },
        Command::Render => {
            runner::render(fs, &paths)?;
            line(out, &format!("rendered {}", paths.status_md))
        },
    }
}

#[cfg(test)]
mod tests;
