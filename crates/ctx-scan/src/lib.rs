//! ctx-scan: walk any directory and maintain a `.context/` summary tree.
//!
//! A self-contained alternative to `ctx-summarize` that takes a directory
//! rather than a file list and works outside a git repository. Prompt
//! files are read at runtime from a configurable directory (default
//! `prompts/`, relative to the process cwd), decoupled from the tree
//! being scanned. Change detection is content-hash based ([`hash`]):
//! `--check` reports staleness with no model call, `--update`
//! regenerates only what changed. Public surface: [`runner::scan_run`] /
//! [`runner::check_run`] / [`runner::update_run`] for programmatic use;
//! [`cli::dispatch`] for CLI-driven use.

pub mod cli;
pub mod error;
pub mod hash;
pub mod readme;
pub mod runner;
pub mod walker;
