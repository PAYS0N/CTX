//! ctx-context: serve the context chain (rollups + intent) for a path.
//!
//! The read side of the CTX context tree after the lead-by-hooks pivot:
//! agents read and edit source with native tools; this crate computes and
//! prints the summary chain above a path, and its `--hook` mode injects
//! that chain into a Claude Code session fail-open. The binary is a thin
//! shell over [`cli::dispatch`]; [`serve`], [`session`], and [`hook`] are
//! pure over the [`env::Env`] boundary, so the core has no argv, process,
//! or network dependency.

pub mod chain;
pub mod cli;
pub mod env;
pub mod error;
pub mod hook;
pub mod repo_path;
pub mod serve;
pub mod session;
