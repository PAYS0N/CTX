//! `ctx-run <dir> "<task>"` — one-command billed cage session.
//!
//! Agent auth is the operator's Claude Code subscription (the bound
//! credential; nothing to configure here). The optional
//! `~/.config/ctx/env` (must be `0600` when present) feeds only the
//! post-run summary refresh: `CTX_AGENT_CMD` + `ANTHROPIC_API_KEY`
//! are passed into the `ctx-scan --update` child, never the shell and
//! never the cage. Typing `ctx-run` *is* the explicit spend
//! authorization, so no separate `--allow-spend` is required here.
//!
//! The summary refresh pins its own model choice — `ctx-scan`'s
//! `--leaf-model`/`--rollup-model` stay required with no default, but
//! this is the one automated, unattended caller that always wants
//! haiku for leaves and sonnet for rollups, so `refresh_summaries`
//! hardcodes both rather than exposing them as `ctx-run` flags.

use std::collections::HashMap;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::Parser;

use ctx_cage::cli::Mode;
use ctx_cage::error::CageError;
use ctx_cage::lifecycle::execute;

use env_file::load_env_file;
use resolve::{build_resolved, ctx_run_bins};

#[path = "ctx_run/env_file.rs"]
mod env_file;
#[path = "ctx_run/resolve.rs"]
mod resolve;

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
    /// Skip the post-run summary refresh (and its y/n prompt) entirely.
    #[arg(long)]
    skip_summarize: bool,
}

/// Write a message to a handle, ignoring failure (in-tree convention).
fn emit<W: Write>(mut w: W, msg: &str) {
    let result: Result<(), std::io::Error> = writeln!(w, "{msg}");
    if result.is_err() {}
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

/// Ask the operator whether to regenerate context summaries now. Only
/// prompts when stdin is a real terminal (same `is_terminal()`
/// discipline as the PTY relay, ADR-048); piped/CI/test invocations
/// have no one to ask, so they fall back to "no" and the caller prints
/// the manual command instead.
fn confirm_summarize() -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    let mut out = std::io::stdout().lock();
    let wrote = write!(out, "ctx-run: regenerate context summaries now? [y/N] ").is_ok();
    if !wrote || out.flush().is_err() {
        return false;
    }
    drop(out);
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

/// Model `ctx-run`'s autosummarization pins for leaf summaries — cheap,
/// run far more often, one file of narrow context at a time.
const AUTO_LEAF_MODEL: &str = "claude-haiku-4-5-20251001";

/// Model `ctx-run`'s autosummarization pins for rollup summaries — run
/// less often, synthesizes multiple children, warrants a stronger model.
const AUTO_ROLLUP_MODEL: &str = "claude-sonnet-5";

/// The manual-recovery command printed when the refresh doesn't run,
/// pinning the same models as `refresh_summaries` so it actually
/// succeeds if copy-pasted.
fn manual_refresh_hint(dir: &Path) -> String {
    format!(
        "ctx-scan {} --update --approve --leaf-model {AUTO_LEAF_MODEL} --rollup-model {AUTO_ROLLUP_MODEL}",
        dir.display()
    )
}

/// Post-session summary refresh — the ONE place regeneration happens
/// (the Stop hook only reports): `ctx-scan <dir> --update`. Agent
/// config from `~/.config/ctx/env` is passed only into the child's
/// environment; when absent, ctx-scan falls back to the target's own
/// `.env`. Pins `--leaf-model`/`--rollup-model` to haiku/sonnet
/// (`AUTO_LEAF_MODEL`/`AUTO_ROLLUP_MODEL`) — the flags themselves stay
/// required with no default, this is just the one automated caller's
/// policy choice. Never fails the run: the session's deliverable
/// already landed; a refresh problem is maintenance, reported with the
/// manual command.
fn refresh_summaries(scan_bin: &Path, dir: &Path, env: &HashMap<String, String>) {
    let mut cmd = Command::new(scan_bin);
    cmd.arg(dir)
        .arg("--update")
        .arg("--leaf-model")
        .arg(AUTO_LEAF_MODEL)
        .arg("--rollup-model")
        .arg(AUTO_ROLLUP_MODEL);
    for key in ["CTX_AGENT_CMD", "ANTHROPIC_API_KEY"] {
        if let Some(val) = env.get(key) {
            cmd.env(key, val);
        }
    }
    if !cmd.status().is_ok_and(|s| s.success()) {
        emit(
            std::io::stderr().lock(),
            &format!(
                "ctx-run: summary refresh incomplete (see ctx-scan output above); run `{}` to refresh manually",
                manual_refresh_hint(dir)
            ),
        );
    }
}

/// Offers the post-run summary-refresh prompt and either refreshes or
/// prints the manual-recovery hint, depending on the answer.
fn maybe_refresh_summaries(dir: &Path, scan_bin: &Path, env: &HashMap<String, String>) {
    if confirm_summarize() {
        refresh_summaries(scan_bin, dir, env);
    } else {
        emit(
            std::io::stderr().lock(),
            &format!(
                "ctx-run: summary refresh skipped; run `{}` to refresh manually",
                manual_refresh_hint(dir)
            ),
        );
    }
}

/// Inner entry point: run the caged session (subscription auth),
/// refresh summaries on success.
fn run() -> Result<i32, CageError> {
    let extra_env = ctx_cage::lifecycle::load_cagevars_from_cwd();
    let cli = Cli::parse();
    let env = load_env_file()?;
    let mode = pick_mode(&cli)?;
    warn_if_no_hooks(&cli.dir);
    let bins = ctx_run_bins()?;
    let resolved = build_resolved(
        cli.dir.clone(),
        cli.task_id.clone(),
        cli.allow_dirty,
        mode,
        &bins,
        extra_env,
    );
    let outcome = execute(&resolved)?;
    if let Some(diag) = &outcome.diagnostic {
        emit(std::io::stderr().lock(), diag);
    }
    if outcome.exit_code == 0 && !cli.skip_summarize {
        maybe_refresh_summaries(&cli.dir, &bins.scan, &env);
    }
    Ok(outcome.exit_code)
}

/// The chain hook only leads when the target commits its own
/// `.claude/settings.json`; warn (don't fail) when it is absent.
fn warn_if_no_hooks(dir: &Path) {
    if !dir.join(".claude/settings.json").is_file() {
        emit(
            std::io::stderr().lock(),
            "ctx-run: warning — no .claude/settings.json in target; the context-chain hook will not lead this session",
        );
    }
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
