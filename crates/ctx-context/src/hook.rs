//! Claude Code `PostToolUse` hook mode: fail-open chain injection.
//!
//! Reads the hook event JSON, resolves the touched path to a
//! repo-relative target, serves the not-yet-injected chain prefix for
//! this session, and emits a `hookSpecificOutput.additionalContext`
//! payload. Every failure degrades to silence or to an explicit
//! `(chain unavailable)` marker — the hook must never block the agent's
//! read (owner decision: the forcing function is fail-open, loudly).

use std::path::Path;

use serde::Deserialize;

use crate::env::Env;
use crate::serve;

/// The subset of the `PostToolUse` event this hook consumes.
#[derive(Debug, Deserialize)]
struct HookInput {
    /// Claude session id — the deduplication key.
    session_id: Option<String>,
    /// The tool's input parameters.
    tool_input: Option<ToolInput>,
}

/// The project directory named by the event itself (its `cwd` field).
/// The binary prefers this over its own working directory so the hook
/// behaves identically however the harness spawns it.
#[must_use]
pub fn event_cwd(input: &str) -> Option<std::path::PathBuf> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    value
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(std::path::PathBuf::from)
}

/// Path-bearing tool parameters (Read/Edit pass `file_path`; Grep/Glob
/// pass `path`).
#[derive(Debug, Deserialize)]
struct ToolInput {
    /// Absolute path of the file a Read/Edit touched.
    file_path: Option<String>,
    /// Path a Grep/Glob searched (absolute, or relative to the repo).
    path: Option<String>,
}

/// Wrap `text` as the `PostToolUse` additional-context JSON payload.
fn payload(text: &str) -> String {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": text,
        }
    })
    .to_string()
}

/// Repo-relative form of `target`, if it lies inside `base` and is not
/// part of the scaffolding itself (`.context`, `.git`).
fn rel_target(base: &Path, target: &str) -> Option<String> {
    let raw = Path::new(target);
    let rel = if raw.is_absolute() {
        raw.strip_prefix(base).ok()?
    } else {
        raw
    };
    let rel_str = rel.to_string_lossy().into_owned();
    let first = rel_str.split('/').next().unwrap_or_default();
    if rel_str.is_empty() || first == ".context" || first == ".git" {
        return None;
    }
    Some(rel_str)
}

/// Extract the touched path from the parsed event.
fn target_of(input: &HookInput) -> Option<String> {
    let ti = input.tool_input.as_ref()?;
    ti.file_path.clone().or_else(|| ti.path.clone())
}

/// Run hook mode over the raw stdin `input`; `base` is the repo root the
/// hook process runs in. Returns the JSON payload to print, or an empty
/// string meaning "emit nothing". Never errors.
#[must_use]
pub fn run<E: Env>(env: &E, base: &Path, input: &str) -> String {
    let Ok(parsed) = serde_json::from_str::<HookInput>(input) else {
        return String::new();
    };
    let Some(target) = target_of(&parsed) else {
        return String::new();
    };
    let Some(rel) = rel_target(base, &target) else {
        return String::new();
    };
    let session = parsed.session_id.unwrap_or_else(|| "default".to_owned());
    match serve::fresh_chain_for(env, &session, &rel) {
        Ok(Some(nodes)) => payload(&serve::render(&nodes)),
        Ok(None) => String::new(),
        Err(e) => payload(&format!("(ctx-context: chain unavailable for {rel}: {e})")),
    }
}
