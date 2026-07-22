//! ctx-summarize: the leaf-up context-tree summarization runner.
//!
//! A thin shell over the prompt files. Prompt content is loaded from
//! `prompts/` at runtime and never embedded in code; dynamic data is
//! passed only in the agent's user message. Layering mirrors the other
//! tools: [`cli`] (argv/io) over [`runner`] (pure orchestration) over the
//! [`fs`] and [`agent`] seams. `intent.md` is never written.

pub mod agent;
pub mod cli;
pub mod cpath;
pub mod error;
pub mod fs;
pub mod progress;
mod rollup_input;
pub mod runner;
