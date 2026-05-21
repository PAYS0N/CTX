//! The broker: accepts a request on one bidirectional stream and
//! serves it back through the wire protocol.
//!
//! Two functions, both pure over the [`Spawner`] seam. [`serve_one`]
//! handles one request → response exchange; [`run`] is the accept
//! loop. The accept loop is sequential by design — a caged agent
//! issues tool calls one at a time, so adding concurrency would buy
//! nothing and complicate cleanup.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;

use crate::error::CageError;
use crate::protocol::{read_request, write_exit, write_output};
use crate::spawn::Spawner;

/// Exit code returned to the client when the requested tool is not in
/// the allowlist. Distinct from any real tool exit so callers can
/// recognize it.
pub const EXIT_NOT_ALLOWED: u32 = 94;

/// Exit code returned to the client when the spawner itself failed
/// (could not start the tool, I/O error mid-spawn, …).
pub const EXIT_SPAWN_ERROR: u32 = 95;

/// Serve a single request → response exchange over `stream`.
///
/// Returns `Ok(())` when the EXIT frame is successfully written —
/// even if the tool itself failed, because that is signalled by the
/// EXIT code, not by this function's `Result`. Returns `Err` only
/// when the protocol itself is broken (bad framing, I/O on the
/// socket, …).
///
/// # Errors
///
/// [`CageError::Io`] / [`CageError::Json`] / [`CageError::Protocol`] /
/// [`CageError::UnexpectedEof`] propagated from the protocol layer.
pub fn serve_one<S: Spawner, T: Read + Write>(
    spawner: &S,
    allowlist: &[&str],
    mut stream: T,
) -> Result<(), CageError> {
    let req = read_request(&mut stream)?;
    if !allowlist.contains(&req.tool.as_str()) {
        let msg = format!("broker: tool '{}' not in allowlist\n", req.tool);
        write_output(&mut stream, msg.as_bytes())?;
        write_exit(&mut stream, EXIT_NOT_ALLOWED)?;
        return Ok(());
    }
    let result = {
        let mut emit = |chunk: &[u8]| write_output(&mut stream, chunk);
        spawner.spawn(&req.tool, &req.args, &mut emit)
    };
    match result {
        Ok(code) => write_exit(&mut stream, code),
        Err(e) => write_spawn_error(&mut stream, &e),
    }
}

/// Helper for the spawner-error path: emit a one-line diagnostic then
/// the dedicated `EXIT_SPAWN_ERROR` frame so the client never gets a
/// silent failure (mirrors [[ADR-024]]'s never-silent rule).
fn write_spawn_error<W: Write>(mut w: W, e: &CageError) -> Result<(), CageError> {
    let msg = format!("broker: spawn error: {e}\n");
    write_output(&mut w, msg.as_bytes())?;
    write_exit(&mut w, EXIT_SPAWN_ERROR)
}

/// Accept-loop wrapper: serve connections sequentially until the
/// listener is closed or an accept fails.
///
/// A failure to serve one connection (broken protocol on that socket)
/// does **not** stop the loop — the broker must outlive one bad
/// request.
///
/// # Errors
///
/// [`CageError::Io`] only on a fatal accept failure (the listener
/// itself died). Per-connection protocol errors are logged via
/// `log_err` and swallowed so the loop continues.
pub fn run<S: Spawner>(
    listener: &UnixListener,
    spawner: &S,
    allowlist: &[&str],
    log_err: &mut dyn FnMut(&CageError),
) -> Result<(), CageError> {
    for incoming in listener.incoming() {
        let stream = incoming?;
        if let Err(e) = serve_one(spawner, allowlist, stream) {
            log_err(&e);
        }
    }
    Ok(())
}
