//! Lifecycle phase 2: stand up the API proxy (billed modes), exec the
//! cage via `bwrap`, stop the proxy on cage exit.

use std::ffi::OsString;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::bwrap::{
    build_bwrap_args, BwrapConfig, API_SOCK_NAME, CAGE_BIN, CAGE_CLAUDE_CONFIG, CAGE_CLAUDE_CRED,
    CAGE_LOCAL_CLAUDE, CAGE_RULES_PATH,
};
use crate::cli::{mode_is_billed, Mode};
use crate::error::CageError;
use crate::proxy::{self, ProxyConfig, SocatUpstream};
use crate::runtime::CAGE_BASE_URL;

use super::prepare::Prep;
use super::Resolved;

/// The upstream API host the proxy dials.
const API_HOST: &str = "api.anthropic.com";

/// A running proxy thread and its stop signal.
struct ProxyHandle {
    /// Set to make the accept loop exit.
    stop: Arc<AtomicBool>,
    /// The accept-loop thread.
    handle: JoinHandle<Result<(), CageError>>,
}

/// Start the proxy (billed modes only), exec the cage, stop the
/// proxy, and return the cage's exit code.
///
/// # Errors
///
/// [`CageError::Io`] on socket / bwrap spawn failures.
pub fn run_until_exit(r: &Resolved, prep: &Prep) -> Result<i32, CageError> {
    let proxy = if mode_is_billed(&r.mode) {
        Some(start_proxy(prep)?)
    } else {
        None
    };
    let status = exec_cage(r, prep);
    if let Some(p) = proxy {
        p.stop.store(true, Ordering::Relaxed);
        let _ = p.handle.join();
    }
    Ok(status?.code().unwrap_or(1))
}

/// Bind the proxy socket in the run dir and spawn the accept loop.
/// Passthrough auth: the caged claude's own `OAuth` `Authorization`
/// header travels through; no key is injected.
fn start_proxy(prep: &Prep) -> Result<ProxyHandle, CageError> {
    let sock = prep.rundir.join(API_SOCK_NAME);
    let _ = std::fs::remove_file(&sock);
    let listener = UnixListener::bind(&sock)?;
    let cfg = Arc::new(ProxyConfig {
        api_key: None,
        upstream_host: API_HOST.to_owned(),
    });
    let upstream = Arc::new(SocatUpstream {
        host: API_HOST.to_owned(),
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
        secret_masks: detect_secret_masks(&r.target_root),
        mask_file: prep.mask_file.clone(),
        toolchain: toolchain_dirs(),
        tool_binds: tool_binds(r, prep),
        rw_binds: claude_rw_binds(prep),
        rundir: prep.rundir.clone(),
        cage_rules_path: prep.rules_file.clone(),
        env: cage_env(r),
        new_session: !interactive,
        cage_cmd: cage_cmd_for_mode(&r.mode),
    };
    let argv = build_bwrap_args(&cfg);
    Ok(Command::new("bwrap").args(argv.iter().skip(1)).status()?)
}

/// Repo-relative secret paths that exist in the target and must be
/// masked even inside the writable workspace.
fn detect_secret_masks(root: &std::path::Path) -> Vec<String> {
    [".env", ".git/config"]
        .iter()
        .filter(|rel| root.join(rel).exists())
        .map(|rel| (*rel).to_owned())
        .collect()
}

/// Resolve one toolchain home: `$<env_key>` when set, else
/// `~/<default>`; `None` when the directory does not exist.
fn toolchain_home(env_key: &str, default: &str) -> Option<PathBuf> {
    std::env::var_os(env_key)
        .map_or_else(
            || std::env::var_os("HOME").map(|h| PathBuf::from(h).join(default)),
            |v| Some(PathBuf::from(v)),
        )
        .filter(|d| d.is_dir())
}

/// Toolchain directories to bind read-only: `CARGO_HOME`/`RUSTUP_HOME`
/// when set, else `~/.cargo` / `~/.rustup` when present.
fn toolchain_dirs() -> Vec<PathBuf> {
    [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")]
        .iter()
        .filter_map(|(key, default)| toolchain_home(key, default))
        .collect()
}

/// Host binaries bound under `/cage/bin`; billed modes add `claude`
/// (twice: on PATH, and at the installer-check path under the cage
/// `HOME`) and the RO subscription credential.
fn tool_binds(r: &Resolved, prep: &Prep) -> Vec<(PathBuf, String)> {
    let mut binds = vec![
        (r.ctx_verify_bin.clone(), format!("{CAGE_BIN}/ctx-verify")),
        (r.ctx_context_bin.clone(), format!("{CAGE_BIN}/ctx-context")),
        (r.ctx_scan_bin.clone(), format!("{CAGE_BIN}/ctx-scan")),
    ];
    if let Some(rt) = &prep.claude {
        binds.push((rt.claude_binary.clone(), format!("{CAGE_BIN}/claude")));
        binds.push((rt.claude_binary.clone(), CAGE_LOCAL_CLAUDE.to_owned()));
        binds.push((rt.credentials.clone(), CAGE_CLAUDE_CRED.to_owned()));
    }
    binds
}

/// RW binds: only the synthesized `~/.claude.json` (claude rewrites it).
fn claude_rw_binds(prep: &Prep) -> Vec<(PathBuf, String)> {
    prep.claude.as_ref().map_or_else(Vec::new, |rt| {
        vec![(rt.claude_config_json.clone(), CAGE_CLAUDE_CONFIG.to_owned())]
    })
}

/// The complete cage environment (`--clearenv` wipes everything else).
/// No `ANTHROPIC_API_KEY` is ever set: auth is the bound subscription
/// credential, and a key in the env would trigger claude's "use the
/// detected API key?" prompt.
fn cage_env(r: &Resolved) -> Vec<(String, String)> {
    let mut env = base_env(r);
    if let Mode::Task(brief) = &r.mode {
        env.push(("CTX_TASK_BRIEF".to_owned(), brief.clone()));
    }
    if mode_is_billed(&r.mode) {
        env.push(("ANTHROPIC_BASE_URL".to_owned(), CAGE_BASE_URL.to_owned()));
    }
    env
}

/// Mode-independent environment: PATH (cage tools + cargo + system),
/// identity, locale, and the offline toolchain homes.
fn base_env(r: &Resolved) -> Vec<(String, String)> {
    let mut path = format!("{CAGE_BIN}:");
    let mut env = Vec::new();
    if let Some(cargo) = toolchain_home("CARGO_HOME", ".cargo") {
        let s = cargo.to_string_lossy().into_owned();
        path.push_str(&s);
        path.push_str("/bin:");
        env.push(("CARGO_HOME".to_owned(), s));
    }
    if let Some(rustup) = toolchain_home("RUSTUP_HOME", ".rustup") {
        env.push((
            "RUSTUP_HOME".to_owned(),
            rustup.to_string_lossy().into_owned(),
        ));
    }
    path.push_str("/usr/bin:/bin");
    env.push(("PATH".to_owned(), path));
    env.push(("HOME".to_owned(), "/tmp".to_owned()));
    env.push(("USER".to_owned(), "cage".to_owned()));
    env.push(("LANG".to_owned(), "C.UTF-8".to_owned()));
    env.push(("TERM".to_owned(), host_term()));
    env.push(("TASK".to_owned(), r.task_id.clone()));
    env.push(("CARGO_NET_OFFLINE".to_owned(), "true".to_owned()));
    env
}

/// Inherit the host's `TERM` so Claude Code's TUI picks the right
/// palette; fall back to `xterm-256color`.
fn host_term() -> String {
    std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_owned())
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
    let relay = format!(
        "socat -t 86400 TCP-LISTEN:8080,bind=127.0.0.1,reuseaddr,fork \
         UNIX-CONNECT:/run/ctx/{API_SOCK_NAME} &"
    );
    let claude = if headless {
        format!(
            "exec claude -p --dangerously-skip-permissions \
             --append-system-prompt-file {CAGE_RULES_PATH} \"$CTX_TASK_BRIEF\""
        )
    } else {
        format!(
            "exec claude --dangerously-skip-permissions \
             --append-system-prompt-file {CAGE_RULES_PATH}"
        )
    };
    vec![
        "sh".into(),
        "-c".into(),
        format!("{relay}\n{claude}").into(),
    ]
}
