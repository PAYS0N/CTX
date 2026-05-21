//! Lifecycle phase 3: best-effort cleanup. Per ADR-027 hygiene, we
//! never destroy work outside our own sockdir.

use super::prepare::Prep;

/// Remove the per-run sockdir (it holds the rules file and socket).
/// Failures are ignored — `teardown_run` runs after `serve` has
/// already determined the cage's exit code.
pub fn teardown_run(prep: &Prep) {
    let _ = std::fs::remove_dir_all(&prep.sockdir);
}
