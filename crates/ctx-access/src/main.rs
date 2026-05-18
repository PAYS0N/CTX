//! ctx-access binary entry point.
//!
//! Thin shell: build the real [`StdEnv`] rooted at the current directory,
//! parse argv, dispatch, and map the outcome to a process exit code.
//! Output goes through writer handles, never `println!`/`eprintln!`, so
//! the `print_stdout`/`print_stderr` restrictions hold without exception.

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use ctx_access::cli::{dispatch, Cli};
use ctx_access::env::StdEnv;

/// Write a message to a handle, ignoring a failed write deliberately
/// (there is no recovery if the error channel itself is broken).
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let base = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            emit(
                std::io::stderr().lock(),
                &format!("cannot resolve cwd: {e}"),
            );
            return ExitCode::from(1);
        },
    };
    let env = StdEnv::new(base);
    let mut out = std::io::stdout().lock();
    match dispatch(&env, cli, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            emit(std::io::stderr().lock(), &format!("error: {e}"));
            ExitCode::from(1)
        },
    }
}
