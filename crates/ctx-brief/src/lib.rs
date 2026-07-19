//! ctx-brief: turn a `docs/STATUS.md` backlog item (or free-text request)
//! into a self-contained executor brief.
//!
//! The crate mirrors the workspace layering: [`main`](../src/main.rs)
//! wires the real boundaries, [`cli`] parses argv, [`runner`] orchestrates
//! over the [`fs::Fs`] and [`claude::Claude`] trait seams, and the model
//! prompts live in files (never embedded in code). Two `claude` stages run
//! inside the target repo so its own context hooks ground every read: a
//! cheap read-only gather pass produces a verified dossier, then a plan
//! pass composes the brief (interactive interview by default, headless
//! adjudication under `--headless`).

pub mod claude;
pub mod cli;
pub mod contract;
pub mod error;
pub mod fs;
pub mod runner;
pub mod status_item;
