//! End-to-end test of the broker over a real `UnixListener` /
//! `UnixStream` pair, with a fake [`Spawner`] standing in for the
//! brokered tool. No subprocess; no socket cleanup races (each test
//! uses a unique tmp path).

use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use ctx_cage::broker::{serve_one, EXIT_NOT_ALLOWED};
use ctx_cage::error::CageError;
use ctx_cage::protocol::{read_response_frame, write_request, Request, ResponseFrame};
use ctx_cage::spawn::Spawner;

/// A spawner that emits canned output bytes and returns a canned exit
/// code, recording the `(tool, args)` it was called with for the
/// caller to assert against.
pub struct FakeSpawner {
    /// Bytes to emit as a single `Output` frame.
    pub output: Vec<u8>,
    /// Exit code to return from `spawn`.
    pub code: u32,
    /// Captured `(tool, args)` of the last call.
    pub seen: Mutex<Option<(String, Vec<String>)>>,
}

impl Spawner for FakeSpawner {
    fn spawn(
        &self,
        tool: &str,
        args: &[String],
        emit: &mut dyn FnMut(&[u8]) -> Result<(), CageError>,
    ) -> Result<u32, CageError> {
        if let Ok(mut g) = self.seen.lock() {
            *g = Some((tool.to_owned(), args.to_vec()));
        }
        emit(&self.output)?;
        Ok(self.code)
    }
}

/// Tests share the OS temp dir; this gives each test a unique
/// socket path so they cannot collide if run in parallel.
fn unique_socket(label: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ctx-cage-test-{}-{}-{}.sock",
        label,
        std::process::id(),
        n,
    ))
}

/// Spawn a one-shot broker thread that accepts a single connection
/// from `listener` and serves it. Returns the thread handle so the
/// caller can `join` and surface any error.
fn spawn_broker_thread(
    listener: UnixListener,
    spawner: Arc<FakeSpawner>,
    allowlist: &[&'static str],
) -> thread::JoinHandle<Result<(), CageError>> {
    let allow_owned: Vec<String> = allowlist.iter().map(|s| (*s).to_owned()).collect();
    thread::spawn(move || -> Result<(), CageError> {
        let (stream, _addr) = listener.accept()?;
        let allow_refs: Vec<&str> = allow_owned.iter().map(String::as_str).collect();
        serve_one(spawner.as_ref(), &allow_refs, stream)
    })
}

/// Read response frames until `Exit`, returning the collected output
/// bytes and the exit code. The mirror of the client-side replay loop
/// in `ctx_cage_client.rs`.
fn drain_response(stream: &mut UnixStream) -> Result<(Vec<u8>, u32), CageError> {
    let mut output = Vec::new();
    loop {
        match read_response_frame(&mut *stream)? {
            ResponseFrame::Output(bytes) => output.extend_from_slice(&bytes),
            ResponseFrame::Exit(c) => return Ok((output, c)),
        }
    }
}

/// Drive a complete exchange against a one-shot broker thread,
/// returning `(output, exit)`. Helpers in `tests/` may not use
/// `unwrap`/`expect` (the clippy exemption is only for `#[test]`
/// bodies); errors propagate so the test body does the `expect`.
fn drive(
    req: &Request,
    spawner: &Arc<FakeSpawner>,
    allowlist: &[&'static str],
) -> Result<(Vec<u8>, u32), CageError> {
    let sock = unique_socket("drive");
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock)?;
    let server = spawn_broker_thread(listener, Arc::clone(spawner), allowlist);
    let mut client = UnixStream::connect(&sock)?;
    write_request(&mut client, req)?;
    let (output, exit) = drain_response(&mut client)?;
    server
        .join()
        .map_err(|_| CageError::Protocol("server thread panicked".to_owned()))??;
    let _ = std::fs::remove_file(&sock);
    Ok((output, exit))
}

#[test]
fn allowed_tool_round_trips_through_the_broker() {
    let req = Request {
        tool: "ctx-access".to_owned(),
        args: vec!["read".to_owned(), "lib.rs".to_owned()],
    };
    let spawner = Arc::new(FakeSpawner {
        output: b"SERVED CONTENT\n".to_vec(),
        code: 0,
        seen: Mutex::new(None),
    });
    let (out, code) = drive(&req, &spawner, &["ctx-access", "ctx-verify"]).expect("drive");
    assert_eq!(out, b"SERVED CONTENT\n");
    assert_eq!(code, 0);
}

#[test]
fn allowlist_rejection_emits_a_diagnostic_and_dedicated_exit() {
    let req = Request {
        tool: "rm".to_owned(),
        args: vec!["-rf".to_owned(), "/".to_owned()],
    };
    let spawner = Arc::new(FakeSpawner {
        output: Vec::new(),
        code: 0,
        seen: Mutex::new(None),
    });
    let (out, code) = drive(&req, &spawner, &["ctx-access", "ctx-verify"]).expect("drive");
    let msg = String::from_utf8_lossy(&out);
    assert!(msg.contains("not in allowlist"), "got: {msg}");
    assert_eq!(code, EXIT_NOT_ALLOWED);
}

#[test]
fn args_and_tool_reach_the_spawner_verbatim() {
    let req = Request {
        tool: "ctx-verify".to_owned(),
        args: vec!["mealplan".to_owned()],
    };
    let spawner = Arc::new(FakeSpawner {
        output: b"{\"status\":\"pass\"}".to_vec(),
        code: 0,
        seen: Mutex::new(None),
    });
    let _ = drive(&req, &spawner, &["ctx-access", "ctx-verify"]).expect("drive");
    let (got_tool, got_args) = {
        let guard = spawner.seen.lock().expect("lock");
        let seen = guard.as_ref().expect("spawner was called").clone();
        seen
    };
    assert_eq!(got_tool, "ctx-verify");
    assert_eq!(got_args, vec!["mealplan".to_owned()]);
}
