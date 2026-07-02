//! Host-side claude runtime resolution for billed modes.
//!
//! Auth is the operator's Claude Code **subscription** (`OAuth`): the
//! host's `~/.claude/.credentials.json` is bound read-only into the
//! cage (nothing else from `~/.claude`, preserving blinding), and the
//! synthesized `~/.claude.json` carries ONLY the host `oauthAccount`
//! object plus onboarding pre-completion — so claude auto-detects the
//! credential without prompting. API traffic still leaves the offline
//! cage only through the host proxy relay (`ANTHROPIC_BASE_URL` points
//! at it); the proxy passes the `Authorization` header through.

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::CageError;

/// In-cage base URL claude is pointed at (the socat relay's listener).
pub const CAGE_BASE_URL: &str = "http://127.0.0.1:8080";

/// Host paths a billed cage launch binds for the claude runtime.
#[derive(Debug, Clone)]
pub struct ClaudeRuntime {
    /// Resolved (`readlink -f`) host path of the `claude` binary.
    pub claude_binary: PathBuf,
    /// Host path of `~/.claude/.credentials.json` (bound RO).
    pub credentials: PathBuf,
    /// Host path of the synthesized `~/.claude.json` (rw, ephemeral).
    pub claude_config_json: PathBuf,
}

/// Resolve the claude binary + subscription credential and write the
/// synthesized config into `rundir`.
///
/// # Errors
///
/// [`CageError::Protocol`] when `claude` is not on `PATH` or the
/// credential/host config are missing; [`CageError::Io`] /
/// [`CageError::Json`] on config read/write.
pub fn resolve_claude_runtime(rundir: &Path) -> Result<ClaudeRuntime, CageError> {
    let claude_binary = which_claude_realpath()?;
    let credentials = home_credentials_path()?;
    let claude_config_json = synth_claude_config(rundir)?;
    Ok(ClaudeRuntime {
        claude_binary,
        credentials,
        claude_config_json,
    })
}

/// `readlink -f $(command -v claude)` — the real binary, with all
/// symlinks resolved (Claude Code installs a `bin/claude` symlink
/// pointing at a versioned ELF under `share/`).
fn which_claude_realpath() -> Result<PathBuf, CageError> {
    let out = Command::new("sh")
        .arg("-c")
        .arg("readlink -f \"$(command -v claude)\"")
        .output()?;
    if !out.status.success() {
        return Err(CageError::Protocol(
            "claude not found on PATH (needed for a billed mode)".to_owned(),
        ));
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if raw.is_empty() {
        return Err(CageError::Protocol(
            "claude resolved to an empty path".to_owned(),
        ));
    }
    Ok(PathBuf::from(raw))
}

/// `$HOME/.claude/.credentials.json` (the subscription credential
/// claude reads when no API key is set).
fn home_credentials_path() -> Result<PathBuf, CageError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| CageError::Protocol("HOME unset (needed for a billed mode)".to_owned()))?;
    let path = PathBuf::from(home).join(".claude/.credentials.json");
    if !path.exists() {
        return Err(CageError::Protocol(format!(
            "{} missing — log in to Claude Code once on the host first",
            path.display()
        )));
    }
    Ok(path)
}

/// Read and parse `$HOME/.claude.json` (for the `oauthAccount` object).
fn read_host_claude_config() -> Result<serde_json::Value, CageError> {
    let home =
        std::env::var_os("HOME").ok_or_else(|| CageError::Protocol("HOME unset".to_owned()))?;
    let path = PathBuf::from(home).join(".claude.json");
    let text = std::fs::read_to_string(&path)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    Ok(json)
}

/// Build a minimal `~/.claude.json` and drop it at `<rundir>/claude.json`.
///
/// Onboarding is pre-completed, `/work` is pre-trusted, and the host's
/// `oauthAccount` object is carried so the bound credential
/// auto-detects with no login prompt. No projects or history enter the
/// cage.
fn synth_claude_config(rundir: &Path) -> Result<PathBuf, CageError> {
    let host_config = read_host_claude_config()?;
    let oauth = host_config
        .get("oauthAccount")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let cfg = build_synth_config(&oauth);
    let path = rundir.join("claude.json");
    std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(path)
}

/// The synthesized config body, given an already-extracted `oauth`
/// object. Takes by reference + clones inside (clippy's
/// `needless_pass_by_value` doesn't trace through `json!`).
fn build_synth_config(oauth: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "hasCompletedOnboarding": true,
        "lastOnboardingVersion": "2.1.123",
        "firstStartTime": "2026-02-24T20:59:45.765Z",
        "numStartups": 22,
        "autoUpdates": false,
        "theme": "dark",
        "anonymousId": random_anonymous_id(),
        "oauthAccount": oauth.clone(),
        "projects": {
            "/work": {
                "hasTrustDialogAccepted": true,
                "projectOnboardingSeenCount": 9,
                "hasClaudeMdExternalIncludesWarningShown": true,
                "allowedTools": [],
            }
        }
    })
}

/// 32-hex-char fresh anonymous id from `/dev/urandom`. We don't need
/// cryptographic uniqueness — Claude Code uses it for telemetry; a
/// per-run fresh value is the blinding-preserving choice.
fn random_anonymous_id() -> String {
    let mut buf = [0_u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    let mut s = String::with_capacity(32);
    for byte in buf {
        // `write!` into a String is infallible; discard the Result so
        // `unused_must_use = "deny"` is satisfied.
        let _ = write!(s, "{byte:02x}");
    }
    s
}
