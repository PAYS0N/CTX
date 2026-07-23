//! ctx-status binary entry point.
//!
//! Thin shell: answer `--contract` before `clap` (it wins over the
//! required subcommand), otherwise wire the real [`StdFs`] (cwd) and
//! dispatch. Output goes through writer handles, never `println!`.

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use ctx_status::cli::{dispatch, Cli};
use ctx_status::fs::StdFs;

/// Write a message to a handle, deliberately ignoring a failed write
/// (there is no recovery if the error channel itself is broken).
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

/// Whether `--contract` was passed. Handled before `clap` so it wins over
/// the required subcommand (the contract is a standalone probe).
fn wants_contract() -> bool {
    std::env::args().skip(1).any(|a| a == "--contract")
}

/// Parse argv and run the pipeline against the real filesystem.
fn run() -> ExitCode {
    let cli = Cli::parse();
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("cannot resolve cwd: {e}"), 2),
    };
    let fs = StdFs::new(cwd);
    let mut out = std::io::stdout().lock();
    match dispatch(&fs, &cli, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("ctx-status error: {e}"), 1),
    }
}

/// Report `msg` on stderr and resolve to exit `code`.
fn fail(msg: &str, code: u8) -> ExitCode {
    emit(std::io::stderr().lock(), msg);
    ExitCode::from(code)
}

fn main() -> ExitCode {
    if wants_contract() {
        emit(std::io::stdout().lock(), ctx_status::contract::CONTRACT);
        return ExitCode::SUCCESS;
    }
    run()
}
