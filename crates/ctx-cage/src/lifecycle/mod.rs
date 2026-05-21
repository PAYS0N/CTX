//! Cage lifecycle orchestrator.
//!
//! Three short phases live in private submodules so the orchestrator
//! stays readable under the workspace length-tier policy:
//!
//! Phase 1 — `prepare`: mint a sockdir, materialize the cage-rules
//! file, run `ctx-access init-task` host-side.
//!
//! Phase 2 — `serve`: stand up the broker thread, exec the cage via
//! `bwrap`, signal the broker to stop on cage exit.
//!
//! Phase 3 — `teardown`: best-effort cleanup of the sockdir.

mod prepare;
mod serve;
mod teardown;

use std::path::PathBuf;

use crate::cli::{mode_is_billed, Mode};
use crate::error::CageError;
use crate::summarize;

/// Inputs the lifecycle needs to execute one run. Built by the host
/// binary from the parsed CLI plus path resolution of the brokered
/// tools.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Absolute host path of the target project root.
    pub target_root: PathBuf,
    /// Task identifier passed to `init-task` and exported as `TASK=`.
    pub task_id: String,
    /// Selected mode (turn 4 only knows `Mode::SelfTestStub`).
    pub mode: Mode,
    /// Absolute host path of the real `ctx-access` binary.
    pub ctx_access_bin: PathBuf,
    /// Absolute host path of the real `ctx-verify` binary.
    pub ctx_verify_bin: PathBuf,
    /// Absolute host path of the real `ctx-summarize` binary.
    pub ctx_summarize_bin: PathBuf,
    /// Absolute host path of `ctx-cage-client` (bound into the cage).
    pub client_bin: PathBuf,
    /// `true` ⇒ provision a real claude runtime in the cage. The
    /// `prepare` phase resolves the actual host paths and writes the
    /// synthesized config into the sockdir. Implied by `Mode::Task`
    /// and `Mode::Interactive`.
    pub claude_runtime: bool,
    /// Spend gate: pre/post auto-summarize and any billed mode only
    /// run when this is `true` (set from `--allow-spend` /
    /// `CTX_CAGE_ALLOW_SPEND=1`).
    pub allow_spend: bool,
}

/// Run the full lifecycle. Teardown runs even if `serve` fails, so a
/// crashed bwrap or broker thread does not strand the sockdir.
///
/// Auto-summarize is invoked **only** when `r.allow_spend` is true
/// AND the mode is billed. The `SelfTestStub` mode is no-spend by
/// definition, so the summarize hooks are no-ops for it — turn 6's
/// billed modes are the ones that actually exercise them.
///
/// # Errors
///
/// Any propagated [`CageError`] from `prepare`, `serve`, or summarize.
/// Teardown errors are swallowed (best-effort cleanup).
pub fn execute(r: &Resolved) -> Result<i32, CageError> {
    let prep = prepare::prepare_run(r)?;
    if r.allow_spend && mode_is_billed(&r.mode) {
        pre_summarize(r)?;
    }
    let result = serve::serve_until_exit(r, &prep);
    if r.allow_spend && mode_is_billed(&r.mode) {
        if let Err(e) = post_summarize(r) {
            // Don't mask the cage's own exit — surface summarize
            // failures only if serve itself succeeded.
            if result.is_ok() {
                teardown::teardown_run(&prep);
                return Err(e);
            }
        }
    }
    teardown::teardown_run(&prep);
    result
}

/// Detect stale leaves and feed them to `ctx-summarize paths`. Costs
/// one model call per stale leaf; a noop when the tree is already
/// fresh.
fn pre_summarize(r: &Resolved) -> Result<(), CageError> {
    let sources = summarize::list_tracked_sources(&r.target_root)?;
    let leaves = summarize::list_tracked_leaves(&r.target_root)?;
    let stale = summarize::compute_stale(&sources, &leaves);
    summarize::run_summarize_paths(&r.ctx_summarize_bin, &r.target_root, &stale)
}

/// Refresh leaves for whatever the agent wrote during the run.
fn post_summarize(r: &Resolved) -> Result<(), CageError> {
    summarize::run_summarize_from_cache(&r.ctx_summarize_bin, &r.target_root, &r.task_id)
}
