//! ctx-access: the sanctioned path to source for agent tasks.
//!
//! The library exposes the enforcement core and its seams. The binary is a
//! thin shell over [`cli::dispatch`]. Layering (mandated by
//! `docs/SANDBOX.md`): [`cli`] (argv/io) over [`enforce`] (pure logic)
//! over [`mod@env`] (the filesystem/clock boundary). Nothing below `cli`
//! knows about the process boundary, so the future `ctx-broker` split is a
//! transport swap rather than a rewrite.

pub mod access;
pub mod cache;
pub mod chain;
pub mod cli;
pub mod enforce;
pub mod env;
pub mod error;
pub mod manifest;
pub mod repo_path;
pub mod report;
