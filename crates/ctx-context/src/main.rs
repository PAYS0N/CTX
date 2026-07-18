//! ctx-context binary entry point.
//!
//! Thin shell: root a [`StdEnv`] at the current directory, parse argv,
//! read stdin only in `--hook` mode, dispatch, and map the outcome to a
//! process exit code. Output goes through writer handles, never
//! `println!`/`eprintln!`, so the `print_stdout`/`print_stderr`
//! restrictions hold without exception.

use std::io::{Read, Write};
use std::process::ExitCode;

use clap::Parser;
use ctx_context::cli::{dispatch, Cli};
use ctx_context::env::StdEnv;

/// Write a message to a handle, ignoring a failed write deliberately
/// (there is no recovery if the error channel itself is broken).
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

/// Read all of stdin (hook-mode input); an empty string on failure so
/// hook mode stays fail-open.
fn read_stdin() -> String {
    let mut buf = String::new();
    let result = std::io::stdin().lock().read_to_string(&mut buf);
    if result.is_err() {
        buf.clear();
    }
    buf
}

/// Repo root: the hook event's `cwd` when present (harnesses differ in
/// the working directory they spawn hooks with), else the process cwd.
fn resolve_root(hook_stdin: &str) -> Result<std::path::PathBuf, std::io::Error> {
    ctx_context::hook::event_cwd(hook_stdin).map_or_else(std::env::current_dir, Ok)
}

/// Whether `--contract` was passed. Handled before `clap` so it wins
/// over the positional-path parsing (the contract is a standalone probe,
/// not a normal invocation).
fn wants_contract() -> bool {
    std::env::args().skip(1).any(|a| a == "--contract")
}

fn main() -> ExitCode {
    if wants_contract() {
        emit(std::io::stdout().lock(), ctx_context::cli::CONTRACT);
        return ExitCode::SUCCESS;
    }
    run()
}

/// The normal (non-`--contract`) path: parse argv, resolve the root,
/// dispatch, and map the outcome to an exit code.
fn run() -> ExitCode {
    let cli = Cli::parse();
    let stdin = if cli.hook_mode() {
        read_stdin()
    } else {
        String::new()
    };
    let root = match resolve_root(&stdin) {
        Ok(dir) => dir,
        Err(e) => {
            emit(
                std::io::stderr().lock(),
                &format!("cannot resolve cwd: {e}"),
            );
            return ExitCode::from(1);
        },
    };
    let env = StdEnv::new(root.clone());
    let mut out = std::io::stdout().lock();
    match dispatch(&env, &root, cli, &stdin, &mut out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            emit(std::io::stderr().lock(), &format!("error: {e}"));
            ExitCode::from(1)
        },
    }
}
