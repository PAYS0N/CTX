//! Host orchestrator for `ctx-cage`. Parses argv into a `Cli`,
//! resolves it into a `lifecycle::Resolved`, and runs the lifecycle.
//! Turn 4 wires only `--self-test stub`; the other modes return a
//! clear "lands in turn N" error from `cli::resolve_mode`.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use ctx_cage::cli::{mode_is_billed, resolve_mode, Cli, Mode, ResolveError};
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
/// CLI's "not yet supported" messages render cleanly.
#[derive(Debug, thiserror::Error)]
enum HostError {
    /// Bubbled from the lifecycle / broker / bwrap stack.
    #[error("{0}")]
    Cage(#[from] CageError),
    /// A `cli::resolve_mode` rejection (unsupported combo or conflict).
    #[error("{0}")]
    Resolve(#[from] ResolveError),
}

/// Sibling-binary paths a host run depends on.
struct BinPaths {
    /// Real `ctx-access` (the brokered source-access tool).
    access: PathBuf,
    /// Real `ctx-verify` (the brokered verification tool).
    verify: PathBuf,
    /// Real `ctx-summarize` (host-side pre/post auto-summarize).
    summarize: PathBuf,
    /// Real `ctx-cage-client` (bound into the cage as the forwarder).
    client: PathBuf,
}

/// Resolve sibling-binary paths from `current_exe`, with env
/// overrides (`CTX_ACCESS_BIN`, `CTX_VERIFY_BIN`,
/// `CTX_SUMMARIZE_BIN`, `CTX_CAGE_CLIENT_BIN`) for installations that
/// place the tools elsewhere.
fn resolve_bin_paths() -> Result<BinPaths, CageError> {
    let me = std::env::current_exe()?;
    let bin_dir = me
        .parent()
        .ok_or_else(|| CageError::Protocol("cannot derive bin dir from current_exe".to_owned()))?;
    let pick = |env_key: &str, name: &str| -> PathBuf {
        std::env::var_os(env_key).map_or_else(|| bin_dir.join(name), PathBuf::from)
    };
    Ok(BinPaths {
        access: pick("CTX_ACCESS_BIN", "ctx-access"),
        verify: pick("CTX_VERIFY_BIN", "ctx-verify"),
        summarize: pick("CTX_SUMMARIZE_BIN", "ctx-summarize"),
        client: pick("CTX_CAGE_CLIENT_BIN", "ctx-cage-client"),
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

/// Build the `Resolved` from CLI + filesystem resolution. The
/// `claude_runtime` flag is true when the user passed `--claude` OR
/// the mode is billed (Task/Interactive both call the model).
fn build_resolved(cli: Cli, mode: Mode, allow_spend: bool) -> Result<Resolved, CageError> {
    let bins = resolve_bin_paths()?;
    let claude_runtime = cli.spend_flags.claude || mode_is_billed(&mode);
    Ok(Resolved {
        target_root: cli.target,
        task_id: cli.task_id.unwrap_or_else(default_task_id),
        mode,
        ctx_access_bin: bins.access,
        ctx_verify_bin: bins.verify,
        ctx_summarize_bin: bins.summarize,
        client_bin: bins.client,
        claude_runtime,
        allow_spend,
    })
}

/// Inner entry point — returns the cage's process exit code so `main`
/// can map it to an `ExitCode`.
fn run() -> Result<i32, HostError> {
    let cli = Cli::parse();
    let allow_spend = cli.spend_flags.allow_spend || env_allow_spend();
    let mode = resolve_mode(&cli, allow_spend)?;
    let resolved = build_resolved(cli, mode, allow_spend)?;
    Ok(execute(&resolved)?)
}

/// Binary entry point. Propagates the cage's exit code (truncated to
/// `u8` per process-exit conventions); a host-side error prints to
/// stderr and exits `1`.
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            let stderr = std::io::stderr().lock();
            emit(stderr, &format!("ctx-cage: {e}"));
            ExitCode::FAILURE
        },
    }
}
