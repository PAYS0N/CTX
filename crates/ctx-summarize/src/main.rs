//! ctx-summarize binary entry point.
//!
//! Thin shell: build the real [`StdFs`] (cwd) and [`SubprocessAgent`]
//! (`CTX_AGENT_CMD`), parse argv, run, stream the JSON summary to stdout.
//! Output goes through writer handles, never `println!`/`eprintln!`.

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;
use ctx_summarize::agent::SubprocessAgent;
use ctx_summarize::cli::{dispatch, Cli};
use ctx_summarize::fs::StdFs;

/// Write a message to a handle, ignoring a failed write deliberately
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let base = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("cannot resolve cwd: {e}"), 2),
    };
    let agent = match SubprocessAgent::from_env() {
        Ok(a) => a,
        Err(e) => return fail(&format!("ctx-summarize error: {e}"), 2),
    };
    let fs = StdFs::new(base);
    let mut out = std::io::stdout().lock();
    match dispatch(&fs, &agent, &cli, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&format!("ctx-summarize error: {e}"), 1),
    }
}
