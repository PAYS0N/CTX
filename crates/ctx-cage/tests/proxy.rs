//! Tests for the API proxy: pure head rewriting, one full connection
//! served over socketpairs with a fake upstream, and that connection
//! failures are logged to a file rather than the process's own stderr
//! (which, during an interactive cage run, is the raw-mode host
//! terminal — writing there directly corrupts the display).

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ctx_cage::error::CageError;
use ctx_cage::proxy::{rewrite_head, serve, serve_conn, ProxyConfig, Upstream, UpstreamIo};

/// Counter for unique tempfile names across tests.
static SEQ: AtomicU32 = AtomicU32::new(0);

/// A unique tempfile path; the test owns cleanup.
fn fresh_tempfile(label: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ctx-cage-proxy-{}-{}-{}.log",
        label,
        std::process::id(),
        n
    ))
}

/// Key-injection config (metered-key posture).
fn cfg() -> ProxyConfig {
    ProxyConfig {
        api_key: Some("sk-real-key".to_owned()),
        upstream_host: "api.anthropic.com".to_owned(),
        log_path: fresh_tempfile("cfg"),
    }
}

#[test]
fn rewrite_replaces_owned_headers_and_keeps_the_rest() {
    let head = "POST /v1/messages HTTP/1.1\r\n\
                Host: 127.0.0.1:8080\r\n\
                x-api-key: placeholder\r\n\
                Authorization: Bearer nope\r\n\
                Connection: keep-alive\r\n\
                content-type: application/json\r\n\
                anthropic-version: 2023-06-01";
    let out = rewrite_head(head, &cfg());
    assert!(out.starts_with("POST /v1/messages HTTP/1.1\r\n"));
    assert!(out.contains("content-type: application/json\r\n"));
    assert!(out.contains("anthropic-version: 2023-06-01\r\n"));
    assert!(out.contains("Host: api.anthropic.com\r\n"));
    assert!(out.contains("x-api-key: sk-real-key\r\n"));
    assert!(out.contains("Connection: close\r\n"));
    assert!(!out.contains("placeholder"));
    assert!(!out.contains("Bearer"));
    assert!(!out.contains("127.0.0.1"));
    assert!(out.ends_with("\r\n\r\n"));
}

#[test]
fn passthrough_mode_keeps_the_clients_oauth_authorization() {
    let passthrough = ProxyConfig {
        api_key: None,
        upstream_host: "api.anthropic.com".to_owned(),
        log_path: fresh_tempfile("passthrough"),
    };
    let head = "POST /v1/messages HTTP/1.1\r\n\
                Host: 127.0.0.1:8080\r\n\
                Authorization: Bearer oauth-token\r\n\
                Connection: keep-alive";
    let out = rewrite_head(head, &passthrough);
    assert!(out.contains("Authorization: Bearer oauth-token\r\n"));
    assert!(!out.to_ascii_lowercase().contains("x-api-key"));
    assert!(out.contains("Host: api.anthropic.com\r\n"));
    assert!(out.contains("Connection: close\r\n"));
    assert!(!out.contains("127.0.0.1"));
}

/// Fake upstream: hands the test the far end of a socketpair.
struct FakeUpstream {
    /// The test-side stream, deposited at connect time.
    server_end: Mutex<Option<UnixStream>>,
}

impl Upstream for FakeUpstream {
    fn connect(&self) -> Result<UpstreamIo, CageError> {
        let (proxy_end, server_end) = UnixStream::pair()?;
        if let Ok(mut slot) = self.server_end.lock() {
            *slot = Some(server_end);
        }
        Ok(UpstreamIo {
            tx: Box::new(proxy_end.try_clone()?),
            rx: Box::new(proxy_end),
        })
    }
}

/// Wait for the fake upstream's server end to appear.
fn take_server_end(up: &FakeUpstream) -> Option<UnixStream> {
    for _ in 0..200 {
        if let Ok(mut slot) = up.server_end.lock() {
            if let Some(s) = slot.take() {
                return Some(s);
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    None
}

// rationale: one linear request→rewrite→forward→response scenario; splitting across fns would fragment the sequence under test.
#[test]
fn serve_conn_rewrites_forwards_and_relays_the_response() {
    let (mut client, proxy_side) = UnixStream::pair().expect("pair");
    let upstream = Arc::new(FakeUpstream {
        server_end: Mutex::new(None),
    });
    let up_for_conn = Arc::clone(&upstream);
    let conn = thread::spawn(move || serve_conn(proxy_side, &cfg(), up_for_conn.as_ref()));

    client
        .write_all(b"POST /v1/messages HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nx-api-key: ph\r\nContent-Length: 2\r\n\r\nhi")
        .expect("send request");

    let mut server = take_server_end(&upstream).expect("upstream never connected");
    let mut got = vec![0_u8; 4096];
    let n = server.read(&mut got).expect("read upstream");
    let seen = String::from_utf8_lossy(got.get(..n).expect("slice"));
    assert!(seen.contains("x-api-key: sk-real-key\r\n"), "got: {seen}");
    assert!(seen.contains("Host: api.anthropic.com\r\n"));
    assert!(seen.ends_with("hi"), "body forwarded, got: {seen}");

    server
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
        .expect("send response");
    drop(server);

    let mut resp = String::new();
    client.read_to_string(&mut resp).expect("read response");
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.ends_with("ok"));
    conn.join().expect("join").expect("serve_conn");
}

/// Always fails to dial, to exercise `serve`'s error-logging path.
struct FailingUpstream;

impl Upstream for FailingUpstream {
    fn connect(&self) -> Result<UpstreamIo, CageError> {
        Err(CageError::Protocol("simulated dial failure".to_owned()))
    }
}

/// A unique socket path; the test owns cleanup.
fn fresh_socket_path() -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ctx-cage-proxy-serve-{}-{n}.sock",
        std::process::id()
    ))
}

/// Poll `log_path` for up to 1s until it contains `needle`; returns the
/// last-read contents either way (for the caller's assertion message).
fn wait_for_log_contains(log_path: &PathBuf, needle: &str) -> String {
    let mut seen = String::new();
    for _ in 0..200 {
        seen = std::fs::read_to_string(log_path).unwrap_or_default();
        if seen.contains(needle) {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    seen
}

/// Binds a fresh listener and returns it with a matching `ProxyConfig`
/// (pointing at its own fresh log file) for the `serve` failure-logging
/// test.
fn bind_serve_fixture() -> std::io::Result<(UnixListener, PathBuf, Arc<ProxyConfig>)> {
    let sock_path = fresh_socket_path();
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)?;
    let cfg = Arc::new(ProxyConfig {
        api_key: None,
        upstream_host: "api.anthropic.com".to_owned(),
        log_path: fresh_tempfile("serve"),
    });
    Ok((listener, sock_path, cfg))
}

/// `serve`'s per-connection failure branch must land in `cfg.log_path`,
/// not the process's own inherited stderr — during an interactive cage
/// run that stderr is the raw-mode host terminal, and writing there
/// directly corrupts the PTY relay's output (the bug this guards).
#[test]
fn serve_logs_connection_failures_to_the_log_file() {
    let (listener, sock_path, cfg) = bind_serve_fixture().expect("bind_serve_fixture");
    let log_path = cfg.log_path.clone();
    let upstream = Arc::new(FailingUpstream);
    let stop = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| {
        scope.spawn(|| serve(&listener, &cfg, &upstream, &stop));

        let mut client = UnixStream::connect(&sock_path).expect("connect");
        client
            .write_all(b"GET /v1/messages HTTP/1.1\r\nHost: x\r\n\r\n")
            .expect("send request");
        drop(client);

        let seen = wait_for_log_contains(&log_path, "connection failed");
        assert!(seen.contains("connection failed"), "log file: {seen:?}");
        stop.store(true, Ordering::Relaxed);
    });

    let _ = std::fs::remove_file(&sock_path);
    let _ = std::fs::remove_file(&log_path);
}
