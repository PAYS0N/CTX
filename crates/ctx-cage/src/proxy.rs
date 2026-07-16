//! Host-side API proxy: the offline cage's sole egress.
//!
//! Listens on a UNIX socket (bind-mounted into the cage at
//! `/run/ctx/api.sock`); the in-cage relay forwards
//! `127.0.0.1:8080` to it. Per connection: read the HTTP/1.1 request
//! head, rewrite it (upstream `Host`, `Connection: close` so each
//! request gets its own connection), open a TLS stream to the API,
//! and pump bytes both ways.
//!
//! Two auth postures: with `api_key: Some(..)` the proxy strips any
//! client credential and injects the real `x-api-key` host-side (the
//! cage only ever sees a placeholder); with `None` it passes the
//! client's own `Authorization` through — the subscription-OAuth
//! posture, where the bound credential authenticates and the proxy's
//! job is purely being the sole, host-controlled egress.
//!
//! The header rewrite is a pure function over the head text; TLS and
//! sockets are quarantined behind the [`Upstream`] seam (the default
//! implementation shells `socat` with certificate verification, so no
//! TLS stack enters this crate).

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::error::CageError;

mod head;
use head::read_head;
pub use head::rewrite_head;

/// Idle-poll cadence for the nonblocking accept loop.
const ACCEPT_IDLE: Duration = Duration::from_millis(20);

/// What the proxy needs to rewrite and forward a request.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// `Some` ⇒ strip client credentials and inject this key as
    /// `x-api-key`; `None` ⇒ pass the client's `Authorization` through
    /// (subscription-OAuth posture).
    pub api_key: Option<String>,
    /// Upstream host, e.g. `api.anthropic.com`.
    pub upstream_host: String,
    /// Where best-effort diagnostics are appended, instead of the
    /// process's own inherited stdio (which, during an interactive run,
    /// is the host terminal `pty.rs` has put in raw mode — writing there
    /// directly races the PTY relay thread and corrupts the display).
    pub log_path: PathBuf,
}

/// A duplex byte stream to the upstream API.
pub struct UpstreamIo {
    /// Write half (request bytes flow up).
    pub tx: Box<dyn Write + Send>,
    /// Read half (response bytes flow back).
    pub rx: Box<dyn Read + Send>,
}

/// Dialer seam: opens one TLS stream to the API per request.
pub trait Upstream: Send + Sync {
    /// Open a fresh upstream stream.
    ///
    /// # Errors
    ///
    /// Implementation-specific; the connection is dropped on failure.
    fn connect(&self) -> Result<UpstreamIo, CageError>;
}

/// Best-effort append-mode `Stdio` for `path`; falls back to discarding
/// output entirely if the log file can't be opened, so a logging
/// failure never blocks the dial it's diagnosing.
fn log_stdio(path: &Path) -> Stdio {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_or_else(|_| Stdio::null(), Stdio::from)
}

/// Best-effort append of one diagnostic line to `path`. Never panics or
/// propagates — a logging failure must not affect the connection it's
/// reporting on.
fn log_line(path: &Path, msg: &str) {
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}");
    }
}

/// Real dialer: `socat - OPENSSL:<host>:443` with certificate
/// verification, long half-close timeout (the ADR-028 lesson).
#[derive(Debug, Clone)]
pub struct SocatUpstream {
    /// Upstream host to dial.
    pub host: String,
    /// Where this `socat` child's stderr is appended (see
    /// [`ProxyConfig::log_path`] — same rationale, independent copy).
    pub log_path: PathBuf,
}

impl Upstream for SocatUpstream {
    fn connect(&self) -> Result<UpstreamIo, CageError> {
        let addr = format!(
            "OPENSSL:{}:443,verify=1,cafile=/etc/ssl/certs/ca-certificates.crt,commonname={}",
            self.host, self.host
        );
        let mut child: Child = Command::new("socat")
            .args(["-t", "86400", "-", &addr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log_stdio(&self.log_path))
            .spawn()?;
        let tx = child
            .stdin
            .take()
            .ok_or_else(|| CageError::Protocol("socat stdin unavailable".to_owned()))?;
        let rx = child
            .stdout
            .take()
            .ok_or_else(|| CageError::Protocol("socat stdout unavailable".to_owned()))?;
        Ok(UpstreamIo {
            tx: Box::new(tx),
            rx: Box::new(rx),
        })
    }
}

/// Pump bytes from `from` to `to` until EOF or error (best-effort).
/// Reused by the interactive PTY relay (`lifecycle::run::pty`).
pub(crate) fn pump(mut from: impl Read, mut to: impl Write) {
    let mut buf = [0_u8; 8192];
    loop {
        match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let Some(chunk) = buf.get(..n) else { break };
                if to.write_all(chunk).is_err() {
                    break;
                }
                if to.flush().is_err() {
                    break;
                }
            },
        }
    }
    let _ = to.flush();
}

/// Serve one client connection: rewrite the head, dial upstream, and
/// pump both directions until the response completes.
///
/// # Errors
///
/// Propagates head-read, dial, and initial-write failures; pump-phase
/// errors just end the connection.
pub fn serve_conn<U: Upstream>(
    mut client: UnixStream,
    cfg: &ProxyConfig,
    upstream: &U,
) -> Result<(), CageError> {
    let (head, leftover) = read_head(&mut client)?;
    let rewritten = rewrite_head(&head, cfg);
    let io = upstream.connect()?;
    let mut tx = io.tx;
    tx.write_all(rewritten.as_bytes())?;
    tx.write_all(&leftover)?;
    tx.flush()?;
    let client_read = client.try_clone()?;
    let up_pump = thread::spawn(move || pump(client_read, tx));
    pump(io.rx, &mut client);
    let _ = client.shutdown(std::net::Shutdown::Both);
    let _ = up_pump.join();
    Ok(())
}

/// Accept loop: one thread per connection, exits when `stop` is set.
///
/// # Errors
///
/// [`CageError::Io`] if the listener cannot be polled.
pub fn serve<U: Upstream + 'static>(
    listener: &UnixListener,
    cfg: &Arc<ProxyConfig>,
    upstream: &Arc<U>,
    stop: &AtomicBool,
) -> Result<(), CageError> {
    listener.set_nonblocking(true)?;
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                let cfg_conn = Arc::clone(cfg);
                let up = Arc::clone(upstream);
                thread::spawn(move || {
                    if let Err(e) = serve_conn(stream, &cfg_conn, up.as_ref()) {
                        log_line(&cfg_conn.log_path, &format!("connection failed: {e}"));
                    }
                });
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(ACCEPT_IDLE),
            Err(e) => return Err(CageError::Io(e)),
        }
    }
}
