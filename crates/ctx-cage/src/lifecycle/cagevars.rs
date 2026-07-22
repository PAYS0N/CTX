//! `.cagevars` loader: gitignored, non-secret sandbox-inclusion config
//! read from the invoking process's CWD (host-machine config — not
//! `cli.target`, which varies per session) before argv parsing, so
//! vars like `CTX_CAGE_EXTRA_PATH` can persist locally without
//! polluting `.env` (ADR-013 scopes that file to the summarizer
//! secret) or the shell profile.
//!
//! Mirrors `ctx-summarize`'s `parse_dotenv`/`resolve_config`: `#`/
//! blank-line skipping, quote stripping, and a process-env-wins merge.
//! A missing `.cagevars` is not an error.
//!
//! `CTX_CAGE_EXTRA_PATH` keeps its own dedicated handling (applied to
//! the *host* process env, since that's what `env::extra_path_dirs`/
//! `toolchain_home` read). Every other key is treated as an arbitrary
//! var: resolved against the host process env (which wins over the
//! file, same rule) and handed back to the caller so it can be
//! threaded into `Resolved::extra_env` and, from there, into the
//! cage's own `--clearenv` environment (`lifecycle::env::cage_env`) —
//! setting it in the *host* env alone would never reach the sandboxed
//! agent. `CTX_CAGE_ALLOW_SPEND` is excluded from that passthrough:
//! it's a spend-consent gate, not sandbox-inclusion or agent config.

use std::path::Path;

/// The `.cagevars` subset this loader recognizes.
#[derive(Debug, Default)]
struct CageVars {
    /// `CTX_CAGE_EXTRA_PATH=` value, when present and non-empty.
    extra_path: Option<String>,
    /// Every other non-blank `KEY=VALUE` pair, in file order (a
    /// repeated key keeps its last occurrence).
    vars: Vec<(String, String)>,
}

/// Parse `.cagevars` text: `KEY=VALUE` lines, `#` comments and blanks
/// skipped, optional surrounding quotes stripped. `CTX_CAGE_EXTRA_PATH`
/// and `CTX_CAGE_ALLOW_SPEND` are handled specially (see module doc);
/// every other key becomes an arbitrary passthrough var.
fn parse_cagevars(text: &str) -> CageVars {
    let mut out = CageVars::default();
    for line in text.lines() {
        apply_line(&mut out, line);
    }
    out
}

/// Parse and apply one `.cagevars` line to `out`; skips comments,
/// blanks, and lines that don't parse as `KEY=VALUE`.
fn apply_line(out: &mut CageVars, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return;
    }
    let Some((key, val)) = trimmed.split_once('=') else {
        return;
    };
    let key = key.trim();
    let clean = val.trim().trim_matches('"').trim_matches('\'').to_owned();
    let Some(value) = Some(clean).filter(|v| !v.is_empty()) else {
        return;
    };
    if key == "CTX_CAGE_EXTRA_PATH" {
        out.extra_path = Some(value);
        return;
    }
    if key == "CTX_CAGE_ALLOW_SPEND" {
        return;
    }
    if let Some(existing) = out.vars.iter_mut().find(|(k, _)| k == key) {
        existing.1 = value;
    } else {
        out.vars.push((key.to_owned(), value));
    }
}

/// Read `<dir>/.cagevars`, apply `CTX_CAGE_EXTRA_PATH` to the process
/// environment, and resolve every other parsed pair against the
/// process environment (which wins over the file).
///
/// `CTX_CAGE_EXTRA_PATH` is set only when not already present there
/// (process env wins; the file only fills the gap) — same rule applied
/// per-key to the returned arbitrary vars. A missing file is not an
/// error (same as missing `.env` in `ctx-summarize`'s
/// `from_env_or_dotenv`).
#[must_use]
pub fn load_cagevars(dir: &Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(dir.join(".cagevars")).unwrap_or_default();
    let vars = parse_cagevars(&text);
    if let Some(extra_path) = vars.extra_path {
        if std::env::var_os("CTX_CAGE_EXTRA_PATH").is_none() {
            std::env::set_var("CTX_CAGE_EXTRA_PATH", extra_path);
        }
    }
    vars.vars
        .into_iter()
        .map(|(key, file_val)| {
            let val = std::env::var(&key).unwrap_or(file_val);
            (key, val)
        })
        .collect()
}

/// [`load_cagevars`] from the process's current working directory.
///
/// The shared entry point both `ctx-cage` and `ctx-run` call before
/// parsing argv. A CWD lookup failure is treated like a missing file:
/// silently skipped, yielding no arbitrary vars.
#[must_use]
pub fn load_cagevars_from_cwd() -> Vec<(String, String)> {
    std::env::current_dir()
        .map(|cwd| load_cagevars(&cwd))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::parse_cagevars;

    /// A `.cagevars` covering the allowlisted key, arbitrary vars
    /// (including a repeated key), and noise, including a key that
    /// must never be recognized here (D3: spend gate).
    const CAGEVARS: &str = "\
# comment\n\
CTX_CAGE_EXTRA_PATH=\"/opt/tools:/opt/more\"\n\
SOMEVAR='this'\n\
OTHERVAR=first\n\
OTHERVAR=second\n\
CTX_CAGE_ALLOW_SPEND=1\n";

    #[test]
    fn extra_path_is_parsed() {
        let vars = parse_cagevars(CAGEVARS);
        assert_eq!(vars.extra_path, Some("/opt/tools:/opt/more".to_owned()));
    }

    #[test]
    fn arbitrary_vars_are_collected_and_allow_spend_is_excluded() {
        let vars = parse_cagevars(CAGEVARS);
        assert_eq!(
            vars.vars,
            vec![
                ("SOMEVAR".to_owned(), "this".to_owned()),
                ("OTHERVAR".to_owned(), "second".to_owned()),
            ],
            "CTX_CAGE_ALLOW_SPEND must never surface as a passthrough var, \
             and a repeated key keeps its last value"
        );
    }

    #[test]
    fn missing_file_default_yields_nothing() {
        let vars = parse_cagevars("");
        assert_eq!(vars.extra_path, None);
        assert!(vars.vars.is_empty());
    }

    #[test]
    fn blank_value_is_treated_as_absent() {
        let vars = parse_cagevars("CTX_CAGE_EXTRA_PATH=\nSOMEVAR=\n");
        assert_eq!(vars.extra_path, None);
        assert!(vars.vars.is_empty());
    }
}
