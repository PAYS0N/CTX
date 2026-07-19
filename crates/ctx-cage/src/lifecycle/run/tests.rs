//! Runtime tests for the `run` module: the [`super::wait_for_relay_snippet`]
//! delayed-bind race (plus a regression for the probe-connection bug it
//! introduced), and the interactive `setsid --ctty` controlling-terminal
//! fix (ADR-048).

use std::fs::File;
use std::io::Read;
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use nix::pty::openpty;

use super::wait_for_relay_snippet;

/// A high, pid-derived port so parallel `cargo test` runs don't
/// collide with each other or with anything already bound (dev
/// machines routinely have something on the real `RELAY_PORT`).
fn test_port(offset: u16) -> u16 {
    let pid = u16::try_from(std::process::id() % 10_000).expect("pid fits u16");
    30_000 + pid + offset
}

/// Run the wait snippet against `port` in a real shell and time it.
fn run_snippet(port: u16) -> (bool, Duration) {
    let start = Instant::now();
    let status = Command::new("sh")
        .arg("-c")
        .arg(wait_for_relay_snippet(port))
        .status()
        .expect("spawn sh");
    (status.success(), start.elapsed())
}

/// Poll `listener` non-blockingly for `window`, counting accepts.
fn count_accepts_for(listener: &TcpListener, window: Duration) -> usize {
    let deadline = Instant::now() + window;
    let mut accepted = 0;
    while Instant::now() < deadline {
        match listener.accept() {
            Ok(_) => accepted += 1,
            Err(_) => thread::sleep(Duration::from_millis(10)),
        }
    }
    accepted
}

/// Simulates `claude` racing a `socat &` that binds late — the exact
/// ECONNREFUSED window the wait loop exists to close. The cage must
/// still come up correctly: the probe should block past the delayed
/// bind and proceed promptly, neither falling through instantly (a
/// false pass) nor riding out the full 5s budget.
#[test]
fn waits_out_a_delayed_relay_bind_then_proceeds() {
    let port = test_port(1);
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind delayed listener");
        thread::sleep(Duration::from_secs(2));
        drop(listener);
    });

    let (ok, elapsed) = run_snippet(port);
    assert!(ok, "should detect the relay once it finally binds");
    assert!(
        elapsed >= Duration::from_millis(250),
        "returned in {elapsed:?} — too fast to have actually waited for the delayed bind"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "took {elapsed:?}, should detect the bind well inside the 5s budget"
    );
}

/// Regression test for the actual bug: the old probe opened a real
/// TCP connection to the relay port, which on the real `socat
/// ...,fork` relay gets forwarded straight into the host-side proxy
/// and surfaces as a spurious "EOF before request head" on every
/// startup. The `/proc/net/tcp`-based probe must never touch the
/// listener at all.
#[test]
fn never_opens_a_connection_to_the_listener() {
    let port = test_port(2);
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind listener");
    listener.set_nonblocking(true).expect("nonblocking");

    let (ok, _) = run_snippet(port);
    assert!(ok, "should detect the already-bound listener");

    let accepts = count_accepts_for(&listener, Duration::from_millis(200));
    assert_eq!(
        accepts, 0,
        "the readiness probe must never open a real connection to the relay port"
    );
}

/// Shell that prints the `/proc/self/stat` fields revealing controlling-
/// terminal state: own pid, process group, session, and the tty
/// foreground pgrp (`-1` == no controlling terminal). After the
/// `(comm)` field come `state ppid pgrp session tty_nr tpgid …`.
///
/// Reads via a `read` builtin redirect, not `$(cat …)`: the redirect is
/// performed by the shell itself, so `/proc/self` resolves to the shell
/// (the session leader) rather than a `cat` subprocess.
fn stat_probe_script() -> String {
    "read -r s < /proc/self/stat; pid=${s%% *}; r=${s##*) }; set -- $r; \
     echo \"PID=$pid PGRP=$3 SID=$4 TPGID=$6\""
        .to_owned()
}

/// Run `cmd` (a `sh -c` argument) with a private PTY slave as its
/// stdin/stdout/stderr — the exact stdio shape `pty::run_on_pty` hands
/// the cage — and return what it printed to that PTY.
fn run_on_private_pty(cmd: &str) -> String {
    let pair = openpty(None, None).expect("openpty");
    let mut master = File::from(pair.master);
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::from(pair.slave.try_clone().expect("dup slave")))
        .stdout(Stdio::from(pair.slave.try_clone().expect("dup slave")))
        .stderr(Stdio::from(pair.slave))
        .spawn()
        .expect("spawn sh on pty");
    let mut out = Vec::new();
    let mut buf = [0_u8; 1024];
    while let Ok(chunk) = master.read(&mut buf).map(|n| buf.get(..n)) {
        match chunk {
            Some([]) | None => break,
            Some(bytes) => out.extend_from_slice(bytes),
        }
    }
    child.wait().expect("wait child");
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse `KEY=<int>` out of a probe line.
fn field(line: &str, key: &str) -> i64 {
    line.split_whitespace()
        .find_map(|tok| tok.strip_prefix(key)?.parse().ok())
        .expect("probe line is missing an expected KEY=<int> field")
}

/// Assert a probe output shows a session leader whose PTY is its
/// controlling terminal with itself in the foreground — the state that
/// makes job control (and thus `tcgetpgrp`) resolve inside the cage.
fn assert_leader_on_ctty(out: &str) {
    let pid = field(out, "PID=");
    let pgrp = field(out, "PGRP=");
    assert_eq!(field(out, "SID="), pid, "not a session leader: {out:?}");
    assert_eq!(pgrp, pid, "session leader is its own pgrp: {out:?}");
    let tpgid = field(out, "TPGID=");
    assert_ne!(tpgid, -1, "no controlling terminal: {out:?}");
    assert_eq!(tpgid, pgrp, "not foreground pgrp of its ctty: {out:?}");
}

/// Regression for the interactive job-control spin (ADR-048). The cage
/// always runs under `--unshare-pid`; a caged process with no
/// controlling terminal inside its own namespace makes Node's readline
/// busy-spin at 100% CPU (`tcgetpgrp` → ENOTTY). [`super::claude_cmd`]
/// now wraps the interactive launch in `setsid --ctty`, which must make
/// the process a session leader whose controlling terminal is its PTY
/// stdin — the precise condition whose absence caused the spin. Checked
/// directly on a private PTY (no bwrap / userns needed).
#[test]
fn setsid_ctty_makes_the_pty_the_controlling_terminal() {
    let script = stat_probe_script();

    let fixed = run_on_private_pty(&format!("setsid --ctty --wait sh -c '{script}'"));
    assert_leader_on_ctty(&fixed);

    // Contrast: a bare launch never becomes a session leader, so it
    // never owns the PTY as a controlling terminal — the pre-fix state.
    let bare = run_on_private_pty(&format!("sh -c '{script}'"));
    assert_ne!(
        field(&bare, "SID="),
        field(&bare, "PID="),
        "a bare cage process must not be a session leader: {bare:?}"
    );
}

/// Tests for [`super::proxy_diagnostic`] (surfacing proxy trouble to the
/// user). Split into its own file purely to stay under the workspace
/// file-length tier alongside the relay-wait/PTY tests above.
mod diagnostic;
