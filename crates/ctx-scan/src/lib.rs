//! ctx-scan: walk any directory and write a `.context/` summary tree.
//!
//! A self-contained alternative to `ctx-summarize` that takes a directory
//! rather than a file list, embeds its prompt files, and works outside a
//! git repository. Public surface: [`runner::scan_run`] for programmatic
//! use; [`cli::dispatch`] for CLI-driven use.

pub mod cli;
pub mod error;
pub mod fs;
pub mod readme;
pub mod runner;
pub mod walker;
