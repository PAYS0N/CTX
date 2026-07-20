//! Bind + environment resolution for the cage: toolchain and extra
//! tool dirs, secret masks, tool binds, and the `--clearenv`
//! replacement environment `exec_cage` hands to `bwrap`. Split out of
//! `run.rs` to keep that file under the length tier.

use std::path::{Path, PathBuf};

use crate::bwrap::{CAGE_BIN, CAGE_CLAUDE_CONFIG, CAGE_CLAUDE_CRED, CAGE_LOCAL_CLAUDE};
use crate::cli::{mode_is_billed, Mode};
use crate::error::CageError;
use crate::runtime::CAGE_BASE_URL;

use super::prepare::Prep;
use super::Resolved;

/// Repo-relative secret paths that exist in the target and must be
/// masked even inside the writable workspace.
pub(super) fn detect_secret_masks(root: &Path) -> Vec<String> {
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

/// Extra host directories to RO-bind at their identical paths and add
/// to PATH — project-specific tools living outside the base isolation
/// dirs (`/usr`, `/bin`, `/lib`, `/lib64`, `/etc/alternatives`), e.g. a
/// neovim install under `/opt`. `:`-separated in `CTX_CAGE_EXTRA_PATH`;
/// a non-empty entry that isn't a directory is a hard error — silently
/// dropping it would surface only as a missing-tool failure deep
/// inside the cage (e.g. pointing at a binary instead of the directory
/// containing it).
fn extra_path_dirs() -> Result<Vec<PathBuf>, CageError> {
    let Ok(raw) = std::env::var("CTX_CAGE_EXTRA_PATH") else {
        return Ok(Vec::new());
    };
    raw.split(':')
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let dir = PathBuf::from(entry);
            if dir.is_dir() {
                Ok(dir)
            } else {
                Err(CageError::Protocol(format!(
                    "CTX_CAGE_EXTRA_PATH entry is not a directory: {entry}"
                )))
            }
        })
        .collect()
}

/// All directories to RO-bind at their identical host paths: the Rust
/// toolchain homes plus any `CTX_CAGE_EXTRA_PATH` entries. Feeds both
/// the bwrap bind list (`toolchain` field) and, via `base_env`, `PATH`
/// — keep both call sites in sync if the resolution logic changes.
pub(super) fn bound_tool_dirs() -> Result<Vec<PathBuf>, CageError> {
    Ok(toolchain_dirs()
        .into_iter()
        .chain(extra_path_dirs()?)
        .collect())
}

/// Host binaries bound under `/cage/bin`; billed modes add `claude`
/// (twice: on PATH, and at the installer-check path under the cage
/// `HOME`) and the RO subscription credential.
pub(super) fn tool_binds(r: &Resolved, prep: &Prep) -> Vec<(PathBuf, String)> {
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
pub(super) fn claude_rw_binds(prep: &Prep) -> Vec<(PathBuf, String)> {
    prep.claude.as_ref().map_or_else(Vec::new, |rt| {
        vec![(rt.claude_config_json.clone(), CAGE_CLAUDE_CONFIG.to_owned())]
    })
}

/// The complete cage environment (`--clearenv` wipes everything else).
/// No `ANTHROPIC_API_KEY` is ever set: auth is the bound subscription
/// credential, and a key in the env would trigger claude's "use the
/// detected API key?" prompt.
pub(super) fn cage_env(r: &Resolved) -> Result<Vec<(String, String)>, CageError> {
    let mut env = base_env(r)?;
    if let Mode::Task(brief) = &r.mode {
        env.push(("CTX_TASK_BRIEF".to_owned(), brief.clone()));
    }
    if mode_is_billed(&r.mode) {
        env.push(("ANTHROPIC_BASE_URL".to_owned(), CAGE_BASE_URL.to_owned()));
    }
    Ok(env)
}

/// Mode-independent environment: PATH (cage tools + cargo + extra +
/// system), identity, locale, and the offline toolchain homes.
fn base_env(r: &Resolved) -> Result<Vec<(String, String)>, CageError> {
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
    for dir in extra_path_dirs()? {
        path.push_str(&dir.to_string_lossy());
        path.push(':');
    }
    path.push_str("/usr/bin:/bin");
    env.push(("PATH".to_owned(), path));
    env.push(("HOME".to_owned(), "/tmp".to_owned()));
    env.push(("USER".to_owned(), "cage".to_owned()));
    env.push(("LANG".to_owned(), "C.UTF-8".to_owned()));
    env.push(("TERM".to_owned(), host_term()));
    env.push(("TASK".to_owned(), r.task_id.clone()));
    env.push(("CARGO_NET_OFFLINE".to_owned(), "true".to_owned()));
    Ok(env)
}

/// Inherit the host's `TERM` so Claude Code's TUI picks the right
/// palette; fall back to `xterm-256color`.
fn host_term() -> String {
    std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_owned())
}
