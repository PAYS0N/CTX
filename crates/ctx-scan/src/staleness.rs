//! What separates a fresh summary tree from one needing work.
//!
//! [`Staleness`] is the report shared by `--check`, `--update`, and the
//! Stop hook: hash-diff results (stale dirs/leaves), integrity gaps
//! (missing artifacts), and reconciliation orphans (artifacts whose
//! source is gone or descoped; see `crate::reconcile`).

/// What separates a fresh tree from a stale one.
#[derive(Debug, Default)]
pub struct Staleness {
    /// Directories whose rollup is stale, deepest-first.
    pub stale_dirs: Vec<String>,
    /// Source files whose leaf summary is stale (new or changed).
    pub changed_files: Vec<String>,
    /// Leaf `.ctx` paths whose source file no longer exists.
    pub orphan_leaves: Vec<String>,
    /// Expected `.ctx`/`rollup.ctx` artifacts the hash tree accounts for
    /// but that are absent on disk — deleted by hand, or never generated.
    /// Freshness ≠ integrity: the tree only hashes source, so a missing
    /// summary is otherwise invisible to `--check` (audit finding #11).
    pub missing_artifacts: Vec<String>,
    /// Derived mirror artifacts (leaf `.ctx`, `rollup.ctx`, sidecar)
    /// whose source no longer exists or is out of scope — the inverse
    /// of `missing_artifacts` (present but unexpected; see
    /// `crate::reconcile`). Excludes paths already in `orphan_leaves`.
    pub orphan_artifacts: Vec<String>,
}

impl Staleness {
    /// True when nothing needs regeneration or pruning and no expected
    /// artifact is missing.
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        self.stale_dirs.is_empty()
            && self.changed_files.is_empty()
            && self.orphan_leaves.is_empty()
            && self.missing_artifacts.is_empty()
            && self.orphan_artifacts.is_empty()
    }

    /// True when summaries must be (re)generated — a model concern —
    /// as opposed to a tree that only carries prunable orphan
    /// artifacts (pure filesystem work).
    #[must_use]
    pub const fn needs_regeneration(&self) -> bool {
        !(self.stale_dirs.is_empty()
            && self.changed_files.is_empty()
            && self.orphan_leaves.is_empty())
    }
}
