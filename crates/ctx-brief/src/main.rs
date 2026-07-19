//! ctx-brief binary entry point.
//!
//! Thin shell: answer `--contract` before `clap` (it wins over the required
//! `<request>` positional), otherwise wire the real [`StdFs`] (cwd) and
//! [`ClaudeCli`], resolve the target to an absolute working directory, and
//! dispatch. Output goes through writer handles, never `println!`.

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use ctx_brief::claude::ClaudeCli;
use ctx_brief::cli::{dispatch, Cli};
use ctx_brief::fs::StdFs;

/// Write a message to a handle, deliberately ignoring a failed write
/// (there is no recovery if the error channel itself is broken).
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

/// Report `msg` on stderr and resolve to exit `code`.
fn fail(msg: &str, code: u8) -> ExitCode {
    emit(std::io::stderr().lock(), msg);
    ExitCode::from(code)
}

/// Whether `--contract` was passed. Handled before `clap` so it wins over
/// the required `<request>` positional (the contract is a standalone probe).
fn wants_contract() -> bool {
    std::env::args().skip(1).any(|a| a == "--contract")
}

/// Parse argv, resolve the target directory, and run the pipeline.
fn run() -> ExitCode {
    let cli = Cli::parse();
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("cannot resolve cwd: {e}"), 2),
    };
    let target_abs = match std::fs::canonicalize(cli.target()) {
        Ok(p) => p,
        Err(e) => {
            return fail(
                &format!("ctx-brief error: target {:?}: {e}", cli.target()),
                2,
            )
        },
    };
    let fs = StdFs::new(cwd);
    let claude = ClaudeCli::default();
    let mut out = std::io::stdout().lock();
    match dispatch(&fs, &claude, &cli, &target_abs, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("ctx-brief error: {e}"), 1),
    }
}

fn main() -> ExitCode {
    if wants_contract() {
        emit(std::io::stdout().lock(), ctx_brief::contract::CONTRACT);
        return ExitCode::SUCCESS;
    }
    run()
}
