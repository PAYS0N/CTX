//! Host orchestrator for `ctx-cage`. Parses argv into a `Cli`,
//! resolves it into a `lifecycle::Resolved`, and runs the lifecycle.
//!
//! Billed modes are gated behind `--allow-spend` /
//! `CTX_CAGE_ALLOW_SPEND=1`; `ctx-run` is the convenient billed
//! launcher (clean-tree default + post-run summary refresh). The
//! always-available mode here is `--self-test stub` — the no-spend
//! containment probe.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use ctx_cage::cli::{resolve_mode, Cli, Mode, ResolveError};
use ctx_cage::error::CageError;
use ctx_cage::lifecycle::{execute, Resolved};

/// Write a message to a handle, ignoring failure. Mirrors the
/// in-tree convention so the `print_stdout`/`print_stderr`
/// restrictions hold without exception.
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

/// Errors the host binary surfaces. Distinct from `CageError` so the
/// CLI's rejection messages render cleanly.
#[derive(Debug, thiserror::Error)]
enum HostError {
    /// Bubbled from the lifecycle / bwrap / proxy stack.
    #[error("{0}")]
    Cage(#[from] CageError),
    /// A `cli::resolve_mode` rejection (unsupported combo or conflict).
    #[error("{0}")]
    Resolve(#[from] ResolveError),
}

/// Sibling CTX binaries bound into the cage under `/cage/bin`.
pub struct BinPaths {
    /// Real `ctx-verify` (the agent's checkpoint).
    pub verify: PathBuf,
    /// Real `ctx-context` (chain server; hook mode).
    pub context: PathBuf,
    /// Real `ctx-scan` (hash check / summary regeneration).
    pub scan: PathBuf,
}

/// Resolve sibling-binary paths from `current_exe`, with env
/// overrides (`CTX_VERIFY_BIN`, `CTX_CONTEXT_BIN`, `CTX_SCAN_BIN`)
/// for installations that place the tools elsewhere.
///
/// # Errors
///
/// [`CageError::Protocol`] if the running binary's directory cannot
/// be derived.
pub fn resolve_bin_paths() -> Result<BinPaths, CageError> {
    let me = std::env::current_exe()?;
    let bin_dir = me
        .parent()
        .ok_or_else(|| CageError::Protocol("cannot derive bin dir from current_exe".to_owned()))?;
    let pick = |env_key: &str, name: &str| -> PathBuf {
        std::env::var_os(env_key).map_or_else(|| bin_dir.join(name), PathBuf::from)
    };
    Ok(BinPaths {
        verify: pick("CTX_VERIFY_BIN", "ctx-verify"),
        context: pick("CTX_CONTEXT_BIN", "ctx-context"),
        scan: pick("CTX_SCAN_BIN", "ctx-scan"),
    })
}

/// Read `CTX_CAGE_ALLOW_SPEND=1` (or unset) as the spend-gate fallback
/// for when the `--allow-spend` flag is not set.
fn env_allow_spend() -> bool {
    std::env::var("CTX_CAGE_ALLOW_SPEND").is_ok_and(|v| v == "1")
}

/// Default task id when the user does not pass `--task-id`.
fn default_task_id() -> String {
    format!("cage-{}", std::process::id())
}

/// Build the `Resolved` from CLI + filesystem resolution. `ctx-cage`
/// never carries an API key, so billed modes are refused downstream.
fn build_resolved(cli: Cli, mode: Mode) -> Result<Resolved, CageError> {
    let bins = resolve_bin_paths()?;
    Ok(Resolved {
        target_root: cli.target,
        task_id: cli.task_id.unwrap_or_else(default_task_id),
        mode,
        ctx_verify_bin: bins.verify,
        ctx_context_bin: bins.context,
        ctx_scan_bin: bins.scan,
        allow_dirty: cli.spend_flags.allow_dirty,
    })
}

/// Inner entry point — returns the cage's process exit code so `main`
/// can map it to an `ExitCode`.
fn run() -> Result<i32, HostError> {
    let cli = Cli::parse();
    let allow_spend = cli.spend_flags.allow_spend || env_allow_spend();
    let mode = resolve_mode(&cli, allow_spend)?;
    let resolved = build_resolved(cli, mode)?;
    Ok(execute(&resolved)?)
}

/// Whether `--contract` was passed. Handled before `clap` so it wins
/// over the required `<target>` positional (the contract is a standalone
/// probe, not a cage invocation).
fn wants_contract() -> bool {
    std::env::args().skip(1).any(|a| a == "--contract")
}

/// Binary entry point. Propagates the cage's exit code (truncated to
/// `u8` per process-exit conventions); a host-side error prints to
/// stderr and exits `1`.
fn main() -> ExitCode {
    if wants_contract() {
        emit(std::io::stdout().lock(), ctx_cage::cli::CONTRACT);
        return ExitCode::SUCCESS;
    }
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            let stderr = std::io::stderr().lock();
            emit(stderr, &format!("ctx-cage: {e}"));
            ExitCode::FAILURE
        },
    }
}
