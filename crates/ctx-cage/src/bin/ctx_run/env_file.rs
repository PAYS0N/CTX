//! Loading `~/.config/ctx/env`, the optional operator file that feeds
//! only the post-run summary refresh (`CTX_AGENT_CMD` +
//! `ANTHROPIC_API_KEY`); never the shell, never the cage.

use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

use ctx_cage::error::CageError;

/// `$HOME/.config/ctx/env`.
fn env_file_path() -> Result<PathBuf, CageError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| CageError::Protocol("HOME unset (need ~/.config/ctx/env)".to_owned()))?;
    Ok(PathBuf::from(home).join(".config/ctx/env"))
}

/// Load the operator env file into a map. The file is optional (it
/// only feeds the summary refresh); when present it must have no
/// group/other permissions — it holds the summarizer API key.
pub fn load_env_file() -> Result<HashMap<String, String>, CageError> {
    let path = env_file_path()?;
    let Ok(meta) = std::fs::metadata(&path) else {
        return Ok(HashMap::new());
    };
    if meta.mode() & 0o077 != 0 {
        return Err(CageError::Protocol(format!(
            "{} must be 0600 (it holds the API key)",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(parse_env(&text))
}

/// Parse `KEY=VALUE` lines (`#` comments and blanks skipped; optional
/// surrounding quotes stripped).
fn parse_env(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = trimmed.split_once('=') {
            let clean = val.trim().trim_matches('"').trim_matches('\'');
            map.insert(key.trim().to_owned(), clean.to_owned());
        }
    }
    map
}
