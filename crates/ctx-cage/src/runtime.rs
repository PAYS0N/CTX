//! Host-side `--claude` runtime resolution.
//!
//! Finds the real `claude` binary, the host user's
//! `~/.claude/.credentials.json`, `/etc/resolv.conf` (followed through
//! its symlink), and materializes two ephemeral files in the run's
//! sockdir: the deterministic minimal `nsswitch.conf` (embedded as an
//! asset) and a **synthesized** `~/.claude.json` that pre-satisfies
//! Claude Code's first-run onboarding while carrying *only* the host
//! `oauthAccount` field (no projects/history — blinding preserved per
//! the original ADR-030).

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bwrap::ClaudeBinds;
use crate::error::CageError;

/// Deterministic minimal `nsswitch.conf` (the host's pulls
/// systemd-only NSS plugins absent in the cage).
pub const CAGE_NSSWITCH_CONF: &str = include_str!("../assets/cage-nsswitch.conf");

/// Resolve every host path the cage's `--claude` mode binds, writing
/// the two ephemeral files (nsswitch + synthesized claude.json) into
/// `sockdir`.
///
/// # Errors
///
/// [`CageError::Protocol`] when `claude` is not on `PATH`, when
/// `HOME` is unset, or when the host credential / config are missing;
/// [`CageError::Io`] on any file write; [`CageError::Json`] if the
/// host config is malformed.
pub fn resolve_claude_binds(sockdir: &Path) -> Result<ClaudeBinds, CageError> {
    let claude_binary = which_claude_realpath()?;
    let credentials = home_credentials_path()?;
    let resolv_conf = std::fs::canonicalize("/etc/resolv.conf")?;
    let nsswitch_conf = write_nsswitch(sockdir)?;
    let claude_config_json = synth_claude_config(sockdir)?;
    Ok(ClaudeBinds {
        claude_binary,
        credentials,
        resolv_conf,
        nsswitch_conf,
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
            "claude not found on PATH (need it for --claude)".to_owned(),
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
        .ok_or_else(|| CageError::Protocol("HOME unset (need it for --claude)".to_owned()))?;
    let path = PathBuf::from(home).join(".claude/.credentials.json");
    if !path.exists() {
        return Err(CageError::Protocol(format!(
            "{} missing (need it for --claude)",
            path.display()
        )));
    }
    Ok(path)
}

/// Drop the embedded nsswitch.conf at `<sockdir>/cage-nsswitch.conf`.
fn write_nsswitch(sockdir: &Path) -> Result<PathBuf, CageError> {
    let path = sockdir.join("cage-nsswitch.conf");
    std::fs::write(&path, CAGE_NSSWITCH_CONF)?;
    Ok(path)
}

/// Build a minimal `~/.claude.json` and drop it at `<sockdir>/claude.json`.
///
/// `oauthAccount` is copied from the host config so claude
/// auto-detects the subscription credential without prompting; every
/// other field is set to a sensible default (onboarding done, `/work`
/// pre-trusted, auto-updates off, theme dark, fresh anonymous id).
fn synth_claude_config(sockdir: &Path) -> Result<PathBuf, CageError> {
    let host_config = read_host_claude_config()?;
    let oauth = host_config
        .get("oauthAccount")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let cfg = build_synth_config(&oauth);
    let path = sockdir.join("claude.json");
    std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(path)
}

/// Read and parse `$HOME/.claude.json`. Errors if `HOME` is unset or
/// the file is missing/invalid.
fn read_host_claude_config() -> Result<serde_json::Value, CageError> {
    let home =
        std::env::var_os("HOME").ok_or_else(|| CageError::Protocol("HOME unset".to_owned()))?;
    let path = PathBuf::from(home).join(".claude.json");
    let text = std::fs::read_to_string(&path)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    Ok(json)
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
