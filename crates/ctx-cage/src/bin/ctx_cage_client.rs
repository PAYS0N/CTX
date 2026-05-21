//! In-cage forwarder. The same binary is bound twice into the cage,
//! at `/cage/bin/ctx-access` and `/cage/bin/ctx-verify`, and chooses
//! its target tool from its own `argv[0]` (busybox-style). It reads
//! its own argv, connects to `$CTX_SOCK`, sends a [`Request`], and
//! streams response frames to its stdout until [`ResponseFrame::Exit`].
//!
//! It carries no logic of its own — every enforcement (deny gate,
//! write-requires-prior-read, verify) lives in the host-side broker.

use std::env;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::ExitCode;

use ctx_cage::error::CageError;
use ctx_cage::protocol::{read_response_frame, write_request, Request, ResponseFrame};

/// Reduce `argv[0]` (which may be an absolute path like
/// `/cage/bin/ctx-access`) to its basename, the tool selector.
fn tool_name(argv0: &str) -> String {
    Path::new(argv0)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or(argv0)
        .to_owned()
}

/// Replay response frames to `stdout` until `Exit`, returning that
/// exit code.
fn replay_response<R: std::io::Read, W: Write>(
    mut stream: R,
    mut stdout: W,
) -> Result<u32, CageError> {
    loop {
        match read_response_frame(&mut stream)? {
            ResponseFrame::Output(bytes) => stdout.write_all(&bytes)?,
            ResponseFrame::Exit(code) => return Ok(code),
        }
    }
}

/// Connect, send the request, replay until `Exit`. Returns the
/// forwarded tool's exit code.
fn run() -> Result<u32, CageError> {
    let all: Vec<String> = env::args().collect();
    let argv0 = all.first().map_or("ctx-cage-client", String::as_str);
    let tool = tool_name(argv0);
    let tool_args = all.get(1..).map(<[String]>::to_vec).unwrap_or_default();
    let sock =
        env::var("CTX_SOCK").map_err(|_| CageError::Protocol("CTX_SOCK unset".to_owned()))?;
    let mut stream = UnixStream::connect(sock)?;
    write_request(
        &mut stream,
        &Request {
            tool,
            args: tool_args,
        },
    )?;
    let stdout = std::io::stdout().lock();
    replay_response(&mut stream, stdout)
}

/// Write a message to a handle, ignoring write failures (no recovery
/// if the error channel is broken). Mirrors the in-tree convention.
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

/// Binary entry point. Propagates the brokered tool's exit code,
/// truncated to `u8` per process-exit conventions; protocol or I/O
/// errors print to stderr and exit `1`.
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            let stderr = std::io::stderr().lock();
            emit(stderr, &format!("ctx-cage-client: {e}"));
            ExitCode::FAILURE
        },
    }
}
