//! Progress reporting seam for the summarization runner.
//!
//! Mirrors the [`crate::agent::Agent`] trait-seam pattern: the runner
//! notifies a [`Progress`] implementation with an item's label
//! immediately before handing it to [`crate::agent::Agent::complete`].
//! [`NoProgress`] is a no-op default that preserves the previously
//! silent behavior for callers that don't want incremental output.

use std::cell::RefCell;
use std::io::Write;

/// Notified just before a leaf or rollup is sent to the agent.
pub trait Progress {
    /// `src` is the repo-relative source path about to be summarized.
    fn leaf(&self, src: &str);
    /// `dir` is the display label (`.` for the repo root) of the
    /// directory about to be rolled up.
    fn rollup(&self, dir: &str);
}

/// A [`Progress`] that emits nothing.
pub struct NoProgress;

impl Progress for NoProgress {
    fn leaf(&self, _src: &str) {}
    fn rollup(&self, _dir: &str) {}
}

/// A [`Progress`] that appends one terse line per item to an injected writer.
///
/// Lines are `leaf: <path>` / `rollup: <dir>` — plain appended lines, no
/// carriage-return redraw. A write failure is silently ignored, matching
/// the rest of the workspace's best-effort output-channel convention.
pub struct WriteProgress<W: Write> {
    /// The writer lines are appended to (e.g. a locked stderr handle).
    /// `RefCell` gives interior mutability so `Progress`'s `&self`
    /// methods can write through a shared handle.
    out: RefCell<W>,
}

impl<W: Write> WriteProgress<W> {
    /// Wrap `out` as a [`Progress`].
    pub const fn new(out: W) -> Self {
        Self {
            out: RefCell::new(out),
        }
    }

    /// Append one labeled line, ignoring a broken write channel.
    fn line(&self, label: &str, value: &str) {
        let result: Result<(), std::io::Error> =
            writeln!(self.out.borrow_mut(), "{label}: {value}");
        if result.is_err() {}
    }
}

impl<W: Write> Progress for WriteProgress<W> {
    fn leaf(&self, src: &str) {
        self.line("leaf", src);
    }

    fn rollup(&self, dir: &str) {
        self.line("rollup", dir);
    }
}
