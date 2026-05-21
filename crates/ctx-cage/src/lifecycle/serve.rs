//! Lifecycle phase 2: spin up the broker on a thread, exec the cage
//! command via `bwrap`, signal the broker to stop, and return the
//! cage's exit code.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::os::unix::net::UnixListener;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::broker::serve_one;
use crate::bwrap::{build_bwrap_args, discover_crate_dirs, BwrapConfig};
use crate::cli::Mode;
use crate::error::CageError;
use crate::spawn::StdSpawner;

use super::prepare::Prep;
use super::Resolved;

/// Tools the cage may invoke via the broker.
const ALLOWLIST: &[&str] = &["ctx-access", "ctx-verify"];

/// Idle-poll cadence for the broker's nonblocking accept loop.
const ACCEPT_IDLE: Duration = Duration::from_millis(20);

/// Stand up the broker thread, exec the cage, wait, signal stop,
/// return the cage's exit code (or `1` if the kernel killed the
/// process without leaving one).
///
/// # Errors
///
/// [`CageError::Io`] on bwrap spawn / socket failures;
/// [`CageError::Protocol`] on a malformed bwrap argv.
pub fn serve_until_exit(r: &Resolved, prep: &Prep) -> Result<i32, CageError> {
    let sock_path = prep.sockdir.join(&prep.sockname);
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path)?;
    listener.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let broker = spawn_broker_thread(listener, build_spawner(r), Arc::clone(&stop));

    let status = exec_cage(r, prep);

    stop.store(true, Ordering::Relaxed);
    let _ = broker.join();
    let _ = std::fs::remove_file(&sock_path);
    let exit = status?;
    Ok(exit.code().unwrap_or(1))
}

/// Build the broker's [`StdSpawner`] from the resolved tool paths.
fn build_spawner(r: &Resolved) -> Arc<StdSpawner> {
    let mut tool_paths = HashMap::new();
    tool_paths.insert("ctx-access".to_owned(), r.ctx_access_bin.clone());
    tool_paths.insert("ctx-verify".to_owned(), r.ctx_verify_bin.clone());
    Arc::new(StdSpawner {
        cwd: r.target_root.clone(),
        tool_paths,
    })
}

/// Spawn the broker thread. It owns the listener; the caller signals
/// shutdown via the `stop` flag and joins to wait.
fn spawn_broker_thread(
    listener: UnixListener,
    spawner: Arc<StdSpawner>,
    stop: Arc<AtomicBool>,
) -> thread::JoinHandle<Result<(), CageError>> {
    thread::spawn(move || run_broker_loop(&listener, spawner.as_ref(), &stop))
}

/// Nonblocking accept loop with a tiny sleep on `WouldBlock`; exits
/// the moment `stop` is set, so the host can return promptly.
fn run_broker_loop(
    listener: &UnixListener,
    spawner: &StdSpawner,
    stop: &AtomicBool,
) -> Result<(), CageError> {
    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                let _ = serve_one(spawner, ALLOWLIST, stream);
            },
            Err(e) if e.kind() == ErrorKind::WouldBlock => thread::sleep(ACCEPT_IDLE),
            Err(e) => return Err(CageError::Io(e)),
        }
    }
}

/// Exec the cage via `bwrap`, inheriting stdio so the caller sees the
/// caged process's output directly.
fn exec_cage(r: &Resolved, prep: &Prep) -> Result<std::process::ExitStatus, CageError> {
    let crates = discover_crate_dirs(&r.target_root)?;
    let interactive = matches!(r.mode, Mode::Interactive);
    let cfg = BwrapConfig {
        target_root: r.target_root.clone(),
        crates,
        sockdir: prep.sockdir.clone(),
        sockname: prep.sockname.clone(),
        task_id: r.task_id.clone(),
        client_binary: r.client_bin.clone(),
        cage_rules_path: prep.rules_file.clone(),
        term: host_term(),
        claude: prep.claude_binds.clone(),
        allow_net: prep.claude_binds.is_some(),
        new_session: !interactive,
        cage_cmd: cage_cmd_for_mode(&r.mode),
    };
    let argv = build_bwrap_args(&cfg)?;
    Ok(Command::new("bwrap").args(argv.iter().skip(1)).status()?)
}

/// Inherit the host's `TERM` so Claude Code's TUI picks the right
/// palette; fall back to `xterm-256color`.
fn host_term() -> String {
    std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_owned())
}

/// Per-mode cage command. The stub uses a brokered probe; both billed
/// modes invoke `claude` with the rules file as appended system
/// context plus `--dangerously-skip-permissions` (the cage IS the
/// sandbox that flag asks for).
fn cage_cmd_for_mode(mode: &Mode) -> Vec<OsString> {
    match mode {
        Mode::SelfTestStub => self_test_stub_cmd(),
        Mode::Task(brief) => claude_headless_cmd(brief),
        Mode::Interactive => claude_interactive_cmd(),
    }
}

/// Brokered shell probe for `--self-test stub`.
fn self_test_stub_cmd() -> Vec<OsString> {
    vec![
        "sh".into(),
        "-c".into(),
        "ctx-access manifest --task-id \"$TASK\" >/dev/null && echo SELF-TEST-STUB-OK".into(),
    ]
}

/// `claude -p <brief>` with the rules file appended as system context.
fn claude_headless_cmd(brief: &str) -> Vec<OsString> {
    let mut v: Vec<OsString> = vec!["claude".into(), "-p".into()];
    push_common_claude_flags(&mut v);
    v.push(brief.into());
    v
}

/// Bare `claude` (interactive TUI), same common flags, no brief.
fn claude_interactive_cmd() -> Vec<OsString> {
    let mut v: Vec<OsString> = vec!["claude".into()];
    push_common_claude_flags(&mut v);
    v
}

/// Common flags every billed claude run uses.
fn push_common_claude_flags(v: &mut Vec<OsString>) {
    v.push("--dangerously-skip-permissions".into());
    v.push("--append-system-prompt-file".into());
    v.push("/opt/cage/rules.md".into());
}
