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

/// The one read (`List`) and one additive-write (`AddTask`) surface an
/// agent is documented to use; `Migrate` is the one-time bootstrap from
/// the pre-existing hand-maintained table and is hidden from `--help`.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the backlog, priority-sorted.
    List,
    /// Append a new backlog item. Additive only: never edits, reorders,
    /// or deletes existing rows.
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

/// Run the parsed command against `fs`, writing output to `out`.
///
/// # Errors
///
/// Propagates every runner failure; a stdout write failure is reported as
/// [`StatusError::Io`].
pub fn dispatch<F: Fs, W: Write>(fs: &F, cli: &Cli, out: &mut W) -> Result<(), StatusError> {
    let paths = cli.paths();
    match &cli.command {
        Command::List => {
            for row in runner::list(fs, &paths)? {
                line(out, &render_row(&row))?;
            }
            Ok(())
        },
        Command::AddTask {
            description,
            impact,
            difficulty,
            task,
        } => {
            runner::add_task(fs, &paths, task, description, impact, difficulty)?;
            line(out, &format!("added: {task}"))
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
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use clap::Parser;

    use super::{dispatch, Cli};
    use crate::error::StatusError;
    use crate::fs::Fs;

    #[derive(Default)]
    struct FakeFs {
        files: RefCell<BTreeMap<String, String>>,
    }

    impl Fs for FakeFs {
        fn read(&self, rel: &str) -> Result<String, StatusError> {
            self.files
                .borrow()
                .get(rel)
                .cloned()
                .ok_or_else(|| StatusError::Io {
                    path: rel.to_owned(),
                    detail: "missing".to_owned(),
                })
        }
        fn write(&self, rel: &str, contents: &str) -> Result<(), StatusError> {
            self.files
                .borrow_mut()
                .insert(rel.to_owned(), contents.to_owned());
            Ok(())
        }
        fn exists(&self, rel: &str) -> bool {
            self.files.borrow().contains_key(rel)
        }
    }

    #[test]
    fn add_task_requires_an_explicit_task_title() {
        let result = Cli::try_parse_from([
            "ctx-status",
            "add-task",
            "a fresh idea",
            "--impact",
            "medium",
            "--difficulty",
            "easy",
        ]);
        assert!(result.is_err(), "must refuse a missing --task");
    }

    #[test]
    fn add_task_uses_the_given_title() -> Result<(), StatusError> {
        let fs = FakeFs::default();
        fs.write("docs/status.json", "[]")?;
        let cli = Cli::parse_from([
            "ctx-status",
            "add-task",
            "a fresh idea",
            "--task",
            "fresh idea",
            "--impact",
            "medium",
            "--difficulty",
            "easy",
        ]);
        let mut out = Vec::new();
        dispatch(&fs, &cli, &mut out)?;
        assert_eq!(String::from_utf8(out).expect("utf8"), "added: fresh idea\n");
        assert!(fs.read("docs/status.json")?.contains("fresh idea"));
        Ok(())
    }

    #[test]
    fn list_prints_one_table_line_per_row() -> Result<(), StatusError> {
        let fs = FakeFs::default();
        fs.write(
            "docs/status.json",
            r#"[{"task":"t","description":"d","impact":"high","difficulty":"easy"}]"#,
        )?;
        let cli = Cli::parse_from(["ctx-status", "list"]);
        let mut out = Vec::new();
        dispatch(&fs, &cli, &mut out)?;
        assert_eq!(
            String::from_utf8(out).expect("utf8"),
            "| t | d | high | easy |\n"
        );
        Ok(())
    }
}
