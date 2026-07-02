//! `ctx-run <dir> "<task>"` — one-command billed cage session.
//!
//! Agent auth is the operator's Claude Code subscription (the bound
//! credential; nothing to configure here). The optional
//! `~/.config/ctx/env` (must be `0600` when present) feeds only the
//! post-run summary refresh: `CTX_AGENT_CMD` + `ANTHROPIC_API_KEY`
//! are passed into the `ctx-scan --update` child, never the shell and
//! never the cage. Typing `ctx-run` *is* the explicit spend
//! authorization, so no separate `--allow-spend` is required here.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::Parser;

use ctx_cage::cli::Mode;
use ctx_cage::error::CageError;
use ctx_cage::lifecycle::{execute, Resolved};

/// One-command billed cage session over a target project.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-run",
    about = "Run a billed caged agent session: ctx-run <dir> \"<task>\""
)]
struct Cli {
    /// Target project root.
    dir: PathBuf,
    /// The task brief (omit with --interactive).
    task: Option<String>,
    /// Interactive session (bare `claude` TUI) instead of a headless task.
    #[arg(long)]
    interactive: bool,
    /// Task identifier (defaults to `run-<pid>`).
    #[arg(long)]
    task_id: Option<String>,
    /// Permit running on a dirty tree (default: refuse).
    #[arg(long)]
    allow_dirty: bool,
    /// Skip the post-run `ctx-scan --update` summary refresh.
    #[arg(long)]
    skip_summarize: bool,
}

/// Write a message to a handle, ignoring failure (in-tree convention).
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
}

/// `$HOME/.config/ctx/env`.
fn env_file_path() -> Result<PathBuf, CageError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| CageError::Protocol("HOME unset (need ~/.config/ctx/env)".to_owned()))?;
    Ok(PathBuf::from(home).join(".config/ctx/env"))
}

/// Load the operator env file into a map. The file is optional (it
/// only feeds the summary refresh); when present it must have no
/// group/other permissions — it holds the summarizer API key.
fn load_env_file() -> Result<HashMap<String, String>, CageError> {
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

/// The launcher's mode: headless task (with brief) or interactive.
fn pick_mode(cli: &Cli) -> Result<Mode, CageError> {
    if cli.interactive {
        return Ok(Mode::Interactive);
    }
    cli.task.clone().map(Mode::Task).ok_or_else(|| {
        CageError::Protocol("a task brief is required (or pass --interactive)".to_owned())
    })
}

/// Post-session summary refresh — the ONE place regeneration happens
/// (the Stop hook only reports): `ctx-scan <dir> --update` with the
/// agent command + key passed only into the child's environment. Never
/// fails the run: the session's deliverable already landed; a refresh
/// problem is maintenance, reported with the manual command.
fn refresh_summaries(scan_bin: &PathBuf, dir: &PathBuf, env: &HashMap<String, String>) {
    let Some(agent_cmd) = env.get("CTX_AGENT_CMD") else {
        emit(
            std::io::stderr().lock(),
            "ctx-run: CTX_AGENT_CMD not in ~/.config/ctx/env; skipping summary refresh",
        );
        return;
    };
    let mut cmd = Command::new(scan_bin);
    cmd.arg(dir).arg("--update").env("CTX_AGENT_CMD", agent_cmd);
    if let Some(key) = env.get("ANTHROPIC_API_KEY") {
        cmd.env("ANTHROPIC_API_KEY", key);
    }
    if !cmd.status().is_ok_and(|s| s.success()) {
        emit(
            std::io::stderr().lock(),
            &format!(
                "ctx-run: summary refresh incomplete (see ctx-scan output above); run `ctx-scan {} --update --approve` to refresh manually",
                dir.display()
            ),
        );
    }
}

/// Inner entry point: run the caged session (subscription auth),
/// refresh summaries on success.
fn run() -> Result<i32, CageError> {
    let cli = Cli::parse();
    let env = load_env_file()?;
    let mode = pick_mode(&cli)?;
    warn_if_no_hooks(&cli.dir);
    let bins = ctx_run_bins()?;
    let resolved = Resolved {
        target_root: cli.dir.clone(),
        task_id: cli
            .task_id
            .clone()
            .unwrap_or_else(|| format!("run-{}", std::process::id())),
        mode,
        ctx_verify_bin: bins.verify,
        ctx_context_bin: bins.context,
        ctx_scan_bin: bins.scan.clone(),
        allow_dirty: cli.allow_dirty,
    };
    let code = execute(&resolved)?;
    if code == 0 && !cli.skip_summarize {
        refresh_summaries(&bins.scan, &cli.dir, &env);
    }
    Ok(code)
}

/// The chain hook only leads when the target commits its own
/// `.claude/settings.json`; warn (don't fail) when it is absent.
fn warn_if_no_hooks(dir: &std::path::Path) {
    if !dir.join(".claude/settings.json").is_file() {
        emit(
            std::io::stderr().lock(),
            "ctx-run: warning — no .claude/settings.json in target; the context-chain hook will not lead this session",
        );
    }
}

/// Sibling CTX binary paths (same resolution rule as `ctx-cage`).
struct RunBins {
    /// Real `ctx-verify`.
    verify: PathBuf,
    /// Real `ctx-context`.
    context: PathBuf,
    /// Real `ctx-scan`.
    scan: PathBuf,
}

/// Resolve sibling binaries from `current_exe` with env overrides.
fn ctx_run_bins() -> Result<RunBins, CageError> {
    let me = std::env::current_exe()?;
    let bin_dir = me
        .parent()
        .ok_or_else(|| CageError::Protocol("cannot derive bin dir from current_exe".to_owned()))?;
    let pick = |env_key: &str, name: &str| -> PathBuf {
        std::env::var_os(env_key).map_or_else(|| bin_dir.join(name), PathBuf::from)
    };
    Ok(RunBins {
        verify: pick("CTX_VERIFY_BIN", "ctx-verify"),
        context: pick("CTX_CONTEXT_BIN", "ctx-context"),
        scan: pick("CTX_SCAN_BIN", "ctx-scan"),
    })
}

/// Binary entry point.
fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            emit(std::io::stderr().lock(), &format!("ctx-run: {e}"));
            ExitCode::FAILURE
        },
    }
}
