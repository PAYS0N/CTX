//! `.cagevars` loader: gitignored, non-secret sandbox-inclusion config
//! read from the invoking process's CWD (host-machine config — not
//! `cli.target`, which varies per session) before argv parsing, so
//! vars like `CTX_CAGE_EXTRA_PATH` can persist locally without
//! polluting `.env` (ADR-013 scopes that file to the summarizer
//! secret) or the shell profile.
//!
//! Mirrors `ctx-summarize`'s `parse_dotenv`/`resolve_config`: a fixed
//! key allowlist (unknown keys silently ignored, no error/warning),
//! `#`/blank-line skipping, quote stripping, and a process-env-wins
//! merge. A missing `.cagevars` is not an error.

use std::path::Path;

/// The `.cagevars` subset this loader recognizes.
#[derive(Debug, Default)]
struct CageVars {
    /// `CTX_CAGE_EXTRA_PATH=` value, when present and non-empty.
    extra_path: Option<String>,
}

/// Parse `.cagevars` text: `KEY=VALUE` lines, `#` comments and blanks
/// skipped, optional surrounding quotes stripped. Keys outside the
/// fixed allowlist are silently ignored — same as `parse_dotenv`'s
/// `_ => {}` arm. `CTX_CAGE_ALLOW_SPEND` is deliberately not in this
/// allowlist: it's a spend-consent gate, not sandbox-inclusion config.
fn parse_cagevars(text: &str) -> CageVars {
    let mut out = CageVars::default();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, val)) = trimmed.split_once('=') else {
            continue;
        };
        let clean = val.trim().trim_matches('"').trim_matches('\'').to_owned();
        let value = Some(clean).filter(|v| !v.is_empty());
        if key.trim() == "CTX_CAGE_EXTRA_PATH" {
            out.extra_path = value;
        }
    }
    out
}

/// Read `<dir>/.cagevars` and apply its recognized keys to the process
/// environment.
///
/// Each key is set only when not already present there (process env
/// wins; the file only fills gaps). A missing file is not an error
/// (same as missing `.env` in `ctx-summarize`'s `from_env_or_dotenv`).
pub fn load_cagevars(dir: &Path) {
    let text = std::fs::read_to_string(dir.join(".cagevars")).unwrap_or_default();
    let vars = parse_cagevars(&text);
    if let Some(extra_path) = vars.extra_path {
        if std::env::var_os("CTX_CAGE_EXTRA_PATH").is_none() {
            std::env::set_var("CTX_CAGE_EXTRA_PATH", extra_path);
        }
    }
}

/// [`load_cagevars`] from the process's current working directory.
///
/// The shared entry point both `ctx-cage` and `ctx-run` call before
/// parsing argv. A CWD lookup failure is treated like a missing file:
/// silently skipped.
pub fn load_cagevars_from_cwd() {
    if let Ok(cwd) = std::env::current_dir() {
        load_cagevars(&cwd);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cagevars;

    /// A `.cagevars` covering the allowlisted key plus noise, including
    /// a key that must never be recognized here (D3: spend gate).
    const CAGEVARS: &str = "\
# comment\n\
CTX_CAGE_EXTRA_PATH=\"/opt/tools:/opt/more\"\n\
UNRELATED=x\n\
CTX_CAGE_ALLOW_SPEND=1\n";

    #[test]
    fn allowlisted_key_is_parsed_and_others_are_ignored() {
        let vars = parse_cagevars(CAGEVARS);
        assert_eq!(vars.extra_path, Some("/opt/tools:/opt/more".to_owned()));
    }

    #[test]
    fn missing_file_default_yields_nothing() {
        let vars = parse_cagevars("");
        assert_eq!(vars.extra_path, None);
    }

    #[test]
    fn blank_value_is_treated_as_absent() {
        let vars = parse_cagevars("CTX_CAGE_EXTRA_PATH=\n");
        assert_eq!(vars.extra_path, None);
    }
}
