//! ctx-check: the token-frugal verification broker.
//!
//! Runs the cargo/clippy/script checks, parses their structured or
//! `FAIL:`-line output, and returns one capped JSON report (same schema
//! family as the spec's audit report) so the token cost of verifying a
//! change is bounded and deterministic. Layering mirrors `ctx-access`:
//! [`cli`] (argv/io) over [`checks`] (pure orchestration) over
//! [`runner`] (the process boundary).

pub mod checks;
pub mod cli;
pub mod error;
pub mod model;
pub mod parse;
pub mod runner;
