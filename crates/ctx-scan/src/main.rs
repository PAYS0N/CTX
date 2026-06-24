//! `ctx-scan` binary entry point.
//!
//! Thin shell: build [`SubprocessAgent`] from `CTX_AGENT_CMD`, parse argv,
//! run, stream human-readable output to stdout. All I/O through handles.

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use ctx_summarize::agent::SubprocessAgent;

use ctx_scan::cli::{dispatch, list_targets, Cli};

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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    if cli.dry_run() {
        return match list_targets(&cli, &mut out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&format!("ctx-scan: {e}"), 1),
        };
    }
    let agent = match SubprocessAgent::from_env() {
        Ok(a) => a,
        Err(e) => return fail(&format!("ctx-scan: {e}"), 2),
    };
    match dispatch(&agent, &cli, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("ctx-scan: {e}"), 1),
    }
}
