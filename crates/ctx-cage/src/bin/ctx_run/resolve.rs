//! Resolve `ctx-run`'s CLI input plus `.cagevars` passthrough into a
//! `lifecycle::Resolved`, and locate sibling CTX binaries next to the
//! running binary. Split out of `ctx_run.rs` to stay under the
//! file-length tier.

use std::path::PathBuf;

use ctx_cage::cli::Mode;
use ctx_cage::error::CageError;
use ctx_cage::lifecycle::Resolved;

/// Sibling CTX binary paths (same resolution rule as `ctx-cage`).
pub struct RunBins {
    /// Real `ctx-verify`.
    pub verify: PathBuf,
    /// Real `ctx-context`.
    pub context: PathBuf,
    /// Real `ctx-scan`.
    pub scan: PathBuf,
}

/// Resolve sibling binaries from `current_exe` with env overrides.
pub fn ctx_run_bins() -> Result<RunBins, CageError> {
    let me = std::env::current_exe()?;
    let bin_dir = me
        .parent()
        .ok_or_else(|| CageError::Protocol("cannot derive bin dir from current_exe".to_owned()))?;
    let pick = |env_key: &str, name: &str| -> PathBuf {
        std::env::var_os(env_key).map_or_else(|| bin_dir.join(name), PathBuf::from)
    };
    Ok(RunBins {
        verify: pick("CTX_VERIFY_BIN", "ctx-verify"),
        context: pick("CTX_CONTEXT_BIN", "ctx-context"),
        scan: pick("CTX_SCAN_BIN", "ctx-scan"),
    })
}

/// Assemble the `Resolved` lifecycle config from CLI fields + resolved
/// sibling binaries + `.cagevars` passthrough.
pub fn build_resolved(
    dir: PathBuf,
    task_id: Option<String>,
    allow_dirty: bool,
    mode: Mode,
    bins: &RunBins,
    extra_env: Vec<(String, String)>,
) -> Resolved {
    Resolved {
        target_root: dir,
        task_id: task_id.unwrap_or_else(|| format!("run-{}", std::process::id())),
        mode,
        ctx_verify_bin: bins.verify.clone(),
        ctx_context_bin: bins.context.clone(),
        ctx_scan_bin: bins.scan.clone(),
        allow_dirty,
        verbose_proxy_log: false,
        extra_env,
    }
}
