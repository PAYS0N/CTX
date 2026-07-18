//! `ctx-scan` binary entry point.
//!
//! Thin shell: build [`SubprocessAgent`] from `CTX_AGENT_CMD` (falling
//! back to the target's `.env` for the command and, child-env only, the
//! API key), parse argv, run, stream human-readable output to stdout.
//! All I/O through handles.

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use ctx_summarize::agent::SubprocessAgent;

use ctx_scan::cli::{check, dispatch, list_targets, stop_hook, Cli};

/// Write `msg` to `w`, ignoring a broken write channel.
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

/// Report `msg` on stderr and resolve to exit `code`.
fn fail(msg: &str, code: u8) -> ExitCode {
    emit(std::io::stderr().lock(), msg);
    ExitCode::from(code)
}

/// Stop-hook mode: report-only, and every failure is swallowed — the
/// hook must never break or bill the agent's session (fail-open;
/// regeneration happens post-session, not per turn).
fn run_stop_hook<W: Write>(cli: &Cli, out: &mut W) -> ExitCode {
    let _ = stop_hook(cli.dir(), out);
    ExitCode::SUCCESS
}

/// Whether `--contract` was passed. Handled before `clap` so it wins
/// over the required `<dir>` positional (the contract is a standalone
/// probe, not a scan invocation).
fn wants_contract() -> bool {
    std::env::args().skip(1).any(|a| a == "--contract")
}

fn main() -> ExitCode {
    if wants_contract() {
        emit(std::io::stdout().lock(), ctx_scan::contract::CONTRACT);
        return ExitCode::SUCCESS;
    }
    run()
}

/// The normal (non-`--contract`) path: parse argv and route to the
/// selected mode.
fn run() -> ExitCode {
    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    if cli.dry_run() {
        return match list_targets(&cli, &mut out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&format!("ctx-scan: {e}"), 1),
        };
    }
    if cli.check() {
        return match check(&cli, &mut out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&format!("ctx-scan: {e}"), 1),
        };
    }
    if cli.stop_hook() {
        return run_stop_hook(&cli, &mut out);
    }
    let agent = match SubprocessAgent::from_env_or_dotenv(cli.dir()) {
        Ok(a) => a,
        Err(e) => return fail(&format!("ctx-scan: {e}"), 2),
    };
    match dispatch(&agent, &cli, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("ctx-scan: {e}"), 1),
    }
}
