//! Lifecycle phase 3: best-effort cleanup. Per ADR-027 hygiene, we
//! never destroy work outside our own run dir.

use super::prepare::Prep;

/// Remove the per-run dir (it holds the rules file, proxy socket, and
/// synthesized claude config). Failures are ignored — `teardown_run`
/// runs after `run` has already determined the cage's exit code.
pub fn teardown_run(prep: &Prep) {
    let _ = std::fs::remove_dir_all(&prep.rundir);
}
