//! Lifecycle phase 2: stand up the API proxy (billed modes), exec the
//! cage via `bwrap`, stop the proxy on cage exit.

use std::ffi::OsString;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::bwrap::{build_bwrap_args, BwrapConfig, API_SOCK_NAME, CAGE_RULES_PATH};
use crate::cli::{mode_is_billed, Mode};
use crate::error::CageError;
use crate::proxy::{self, ProxyConfig, SocatUpstream};

use super::env;
use super::prepare::Prep;
use super::{Resolved, RunOutcome};

/// The upstream API host the proxy dials.
const API_HOST: &str = "api.anthropic.com";

/// TCP port the in-cage relay listens on, and what `claude`'s
/// `ANTHROPIC_BASE_URL` (see `runtime::CAGE_BASE_URL`) points at.
const RELAY_PORT: u16 = 8080;

/// Filename of the proxy's best-effort diagnostic log inside the run
/// dir (see [`ProxyConfig::log_path`]).
const PROXY_LOG_NAME: &str = "proxy.log";

/// Lines kept from the tail of the log in `--verbose-proxy-log` mode.
const VERBOSE_LOG_TAIL: usize = 50;

/// Build the shell snippet that blocks until a TCP listener on
/// `127.0.0.1:port` is actually bound (up to 5s), so `claude`'s first
/// request can't race the `socat &` startup. Without this, an
/// ECONNREFUSED window surfaces to the user as a stuck-looking "API
/// error · Retrying" in Claude Code's own retry UI.
///
/// Checks `/proc/net/tcp` for a `LISTEN` (`0A`) row on
/// `127.0.0.1:port` (`0100007F:<hex port>`, little-endian hex) rather
/// than opening a real probe connection: the relay is `socat
/// ...,fork`, so *any* accepted TCP connection — including a probe
/// that sends no bytes — gets forked straight into the host-side
/// proxy, which reads an immediate EOF and logs a scary (but
/// harmless) "connection failed: EOF before request head" on every
/// single startup. Parameterized over `port` (rather than baking in
/// [`RELAY_PORT`]) so tests can probe a throwaway port instead of
/// racing whatever else has 8080 bound on the host.
fn wait_for_relay_snippet(port: u16) -> String {
    format!(
        "i=0; while [ $i -lt 50 ]; do \
         grep -qE '^[[:space:]]*[0-9]+: 0100007F:{port:04X} [0-9A-Fa-f]{{8}}:[0-9A-Fa-f]{{4}} 0A ' /proc/net/tcp 2>/dev/null && break; \
         i=$((i+1)); sleep 0.1; \
         done"
    )
}

/// A running proxy thread and its stop signal.
struct ProxyHandle {
    /// Set to make the accept loop exit.
    stop: Arc<AtomicBool>,
    /// The accept-loop thread.
    handle: JoinHandle<Result<(), CageError>>,
}

/// Start the proxy (billed modes only), exec the cage, stop the
/// proxy, and return the cage's exit code plus an optional proxy
/// diagnostic. The diagnostic is read from the run dir while it still
/// exists (before `execute` invokes `teardown_run`) and never affects
/// `exit_code` — a transient proxy dial failure shouldn't fail an
/// otherwise-fine billed session (ADR-027).
///
/// # Errors
///
/// [`CageError::Io`] on socket / bwrap spawn failures.
pub fn run_until_exit(r: &Resolved, prep: &Prep) -> Result<RunOutcome, CageError> {
    let proxy = if mode_is_billed(&r.mode) {
        Some(start_proxy(prep)?)
    } else {
        None
    };
    let status = exec_cage(r, prep);
    let diagnostic = proxy.map(|p| {
        p.stop.store(true, Ordering::Relaxed);
        let join = p.handle.join();
        proxy_diagnostic(&prep.rundir.join(PROXY_LOG_NAME), join, r.verbose_proxy_log)
    });
    Ok(RunOutcome {
        exit_code: status?.code().unwrap_or(1),
        diagnostic: diagnostic.flatten(),
    })
}

/// Join outcome of the proxy accept-loop thread.
type ProxyJoin = thread::Result<Result<(), CageError>>;

/// Build a capped diagnostic from the proxy's log file plus the accept
/// thread's join outcome, folding a panic or returned error into the
/// same message rather than surfacing it separately. `None` when
/// there's nothing to report: an empty/absent log and a clean join.
fn proxy_diagnostic(log_path: &Path, join: ProxyJoin, verbose: bool) -> Option<String> {
    let log = std::fs::read_to_string(log_path).unwrap_or_default();
    let mut lines: Vec<String> = log.lines().map(str::to_owned).collect();
    match join {
        Ok(Ok(())) => {},
        Ok(Err(e)) => lines.push(format!("proxy accept loop error: {e}")),
        Err(_) => lines.push("proxy accept loop panicked".to_owned()),
    }
    if lines.is_empty() {
        return None;
    }
    if verbose {
        let start = lines.len().saturating_sub(VERBOSE_LOG_TAIL);
        let tail = lines.get(start..).unwrap_or(&lines).join("\n");
        return Some(format!(
            "proxy diagnostics ({} lines):\n{tail}",
            lines.len()
        ));
    }
    lines.last().map(|last| {
        format!(
            "proxy diagnostics: {} issue(s) logged; last: {last}",
            lines.len()
        )
    })
}

/// Bind the proxy socket in the run dir and spawn the accept loop.
/// Passthrough auth: the caged claude's own `OAuth` `Authorization`
/// header travels through; no key is injected.
fn start_proxy(prep: &Prep) -> Result<ProxyHandle, CageError> {
    let sock = prep.rundir.join(API_SOCK_NAME);
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock)?;
    let log_path = prep.rundir.join(PROXY_LOG_NAME);
    let cfg = Arc::new(ProxyConfig {
        api_key: None,
        upstream_host: API_HOST.to_owned(),
        log_path: log_path.clone(),
    });
    let upstream = Arc::new(SocatUpstream {
        host: API_HOST.to_owned(),
        log_path,
    });
    let stop = Arc::new(AtomicBool::new(false));
    let stop_loop = Arc::clone(&stop);
    let handle = std::thread::spawn(move || proxy::serve(&listener, &cfg, &upstream, &stop_loop));
    Ok(ProxyHandle { stop, handle })
}

/// Exec the cage via `bwrap`, inheriting stdio so the caller sees the
/// caged process's output directly.
fn exec_cage(r: &Resolved, prep: &Prep) -> Result<std::process::ExitStatus, CageError> {
    let interactive = matches!(r.mode, Mode::Interactive);
    let cfg = BwrapConfig {
        target_root: r.target_root.clone(),
        secret_masks: env::detect_secret_masks(&r.target_root),
        mask_file: prep.mask_file.clone(),
        toolchain: env::bound_tool_dirs(),
        tool_binds: env::tool_binds(r, prep),
        rw_binds: env::claude_rw_binds(prep),
        rundir: prep.rundir.clone(),
        cage_rules_path: prep.rules_file.clone(),
        resolv_conf: prep.resolv_file.clone(),
        env: env::cage_env(r),
        new_session: !interactive,
        cage_cmd: cage_cmd_for_mode(&r.mode),
    };
    let argv = build_bwrap_args(&cfg);
    if interactive {
        if let Some(status) = pty::run_on_pty(&argv)? {
            return Ok(status);
        }
    }
    Ok(Command::new("bwrap").args(argv.iter().skip(1)).status()?)
}

/// Per-mode cage command.
fn cage_cmd_for_mode(mode: &Mode) -> Vec<OsString> {
    match mode {
        Mode::SelfTestStub => self_test_stub_cmd(),
        Mode::Task(_) => claude_cmd(true),
        Mode::Interactive => claude_cmd(false),
    }
}

/// Containment probe for `--self-test stub`: workspace writable,
/// secrets masked *but readable-as-empty* (a mask that breaks readers
/// broke git once), git usable, no network; writes only under the
/// gitignored `target/` dir (ADR-027). Cwd-relative (ADR-046).
fn self_test_stub_cmd() -> Vec<OsString> {
    let script = "\
set -e\n\
test -w .\n\
mkdir -p target && touch target/.cage-probe && rm target/.cage-probe\n\
if [ -e .env ] && [ -n \"$(cat .env 2>/dev/null)\" ]; then echo SECRET-LEAK; exit 1; fi\n\
if [ -e .git/config ] && ! cat .git/config >/dev/null 2>&1; then echo MASK-UNREADABLE; exit 1; fi\n\
if [ -d .git ] && ! git status --porcelain >/dev/null 2>&1; then echo GIT-BROKEN; exit 1; fi\n\
if timeout 2 socat -u OPEN:/dev/null TCP:1.1.1.1:80,connect-timeout=1 2>/dev/null; then echo NET-LEAK; exit 1; fi\n\
echo SELF-TEST-STUB-OK\n";
    vec!["sh".into(), "-c".into(), script.into()]
}

/// Billed command: start the in-cage relay (`127.0.0.1:8080` → the
/// bind-mounted proxy socket), then exec claude. The cage IS the
/// sandbox `--dangerously-skip-permissions` asks for.
fn claude_cmd(headless: bool) -> Vec<OsString> {
    let wait = wait_for_relay_snippet(RELAY_PORT);
    let relay = format!(
        "socat -t 86400 TCP-LISTEN:{RELAY_PORT},bind=127.0.0.1,reuseaddr,fork \
         UNIX-CONNECT:/run/ctx/{API_SOCK_NAME} &\n{wait}"
    );
    let claude = if headless {
        format!(
            "exec claude -p --dangerously-skip-permissions \
             --append-system-prompt-file {CAGE_RULES_PATH} \"$CTX_TASK_BRIEF\""
        )
    } else {
        // `setsid --ctty` gives claude a new session whose controlling
        // terminal is its stdin — the private PTY slave (see `pty.rs`).
        // Without it the caged process has no controlling TTY in its own
        // PID namespace and Node's readline busy-spins at 100% CPU.
        // `--wait` propagates claude's exit status back through setsid.
        format!(
            "exec setsid --ctty --wait claude --dangerously-skip-permissions \
             --append-system-prompt-file {CAGE_RULES_PATH}"
        )
    };
    vec![
        "sh".into(),
        "-c".into(),
        format!("{relay}\n{claude}").into(),
    ]
}

mod pty;

#[cfg(test)]
mod tests;
