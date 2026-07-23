//! ctx-status: structured, non-colliding backlog access.
//!
//! `docs/status.json` is the source of truth; `docs/STATUS.md` is a
//! *generated* human/agent-readable view rendered from it. Agents surface
//! priorities on demand (`ctx-status list`) and append proposals
//! (`ctx-status add-task`) instead of hand-editing the markdown table.
//! The crate mirrors the workspace layering: [`main`](../src/main.rs)
//! wires the real [`fs::StdFs`], [`cli`] parses argv, [`runner`]
//! orchestrates over the [`fs::Fs`] seam, and [`model`]/[`render`] are the
//! pure core (priority sort, markdown rendering) with no `Fs`/argv in
//! sight. The table parser/model itself lives in `ctx_core::status_table`,
//! shared with `ctx-brief` rather than duplicated.

pub mod cli;
pub mod contract;
pub mod error;
pub mod fs;
pub mod model;
pub mod render;
pub mod runner;
