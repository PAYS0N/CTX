//! ctx-check binary entry point.
//!
//! Thin shell: parse argv, run the checks through the real
//! [`ProcRunner`], stream the JSON report to stdout, map pass/fail to a
//! process exit code. Output goes through writer handles, never
//! `println!`/`eprintln!`.

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use ctx_check::cli::{run, Cli};
use ctx_check::runner::ProcRunner;

/// Write a message to a handle, ignoring a failed write deliberately
/// (there is no recovery if the error channel itself is broken).
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut out = std::io::stdout().lock();
    match run(&ProcRunner, &cli, &mut out) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            emit(std::io::stderr().lock(), &format!("ctx-check error: {e}"));
            ExitCode::from(2)
        },
    }
}
