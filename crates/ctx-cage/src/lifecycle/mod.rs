//! Cage lifecycle orchestrator.
//!
//! Three short phases live in private submodules so the orchestrator
//! stays readable under the workspace length-tier policy:
//!
//! Phase 1 — `prepare`: gate billed modes (API key present, tree
//! clean), mint a run dir, materialize the cage-rules file, resolve
//! the claude runtime.
//!
//! Phase 2 — `run`: stand up the API proxy thread (billed modes),
//! exec the cage via `bwrap`, stop the proxy on cage exit.
//!
//! Phase 3 — `teardown`: best-effort cleanup of the run dir.

mod prepare;
mod run;
mod teardown;

use std::path::PathBuf;

use crate::cli::Mode;
use crate::error::CageError;

/// Inputs the lifecycle needs to execute one run. Built by the host
/// binaries from the parsed CLI plus path resolution of the tools
/// bound into the cage.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Absolute host path of the target project root.
    pub target_root: PathBuf,
    /// Task identifier (names the run dir; exported as `TASK=`).
    pub task_id: String,
    /// Selected mode.
    pub mode: Mode,
    /// Absolute host path of the real `ctx-verify` binary.
    pub ctx_verify_bin: PathBuf,
    /// Absolute host path of the real `ctx-context` binary.
    pub ctx_context_bin: PathBuf,
    /// Absolute host path of the real `ctx-scan` binary.
    pub ctx_scan_bin: PathBuf,
    /// Permit a billed run on a dirty tree (default: refuse — plain
    /// git from a clean commit is the recovery story).
    pub allow_dirty: bool,
}

/// Run the full lifecycle. Teardown runs even if `run` fails, so a
/// crashed bwrap or proxy thread does not strand the run dir.
///
/// # Errors
///
/// Any propagated [`CageError`] from `prepare` or `run`. Teardown
/// errors are swallowed (best-effort cleanup).
pub fn execute(r: &Resolved) -> Result<i32, CageError> {
    let r = &canonicalize_target(r)?;
    let prep = prepare::prepare_run(r)?;
    let result = run::run_until_exit(r, &prep);
    teardown::teardown_run(&prep);
    result
}

/// Resolve `target_root` to its canonical absolute form. `bwrap` binds
/// it at the identical path inside the cage (ADR-046) alongside other
/// binds that are always absolute (`/cage/bin`, `/opt/cage/rules.md`,
/// `/run/ctx`); a relative `target_root` (e.g. `.`, from `ctx-run .`)
/// mixes relative and absolute dests in one `bwrap` invocation, which
/// makes `bwrap` create those mount points as real directories on the
/// host disk instead of inside the sandboxed root.
fn canonicalize_target(r: &Resolved) -> Result<Resolved, CageError> {
    let target_root = r.target_root.canonicalize()?;
    Ok(Resolved {
        target_root,
        ..r.clone()
    })
}
