//! Command-line surface for `ctx-cage`. Parses argv into a
//! `lifecycle::Resolved` (via [`resolve_mode`] + path resolution in
//! the host binaries).
//!
//! Mode resolution rule: `--self-test <kind>` wins; otherwise
//! `--task` / `--task-file` ⇒ headless; otherwise interactive. Billed
//! modes are double-gated: `--allow-spend` here, and the host-side
//! API key (only `ctx-run` provides it) in the lifecycle.
//!
//! Flags are grouped into [`TaskFlags`] and [`SpendFlags`] sub-structs
//! to keep the `struct_excessive_bools` clippy threshold satisfied
//! without `#[allow]` (banned by the workspace policy).

use std::path::PathBuf;

use clap::{Args, Parser, ValueEnum};

/// The raw CLI parsed by `clap`.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-cage",
    about = "Generic sandboxed-agent harness: brokered ctx-access/ctx-verify over a project."
)]
pub struct Cli {
    /// Target project root (absolute path). REQUIRED — no default.
    pub target: PathBuf,

    /// Task identifier (defaults to `cage-<pid>`).
    #[arg(long)]
    pub task_id: Option<String>,

    /// No-spend self-test. Mutually exclusive with `--task*` /
    /// interactive default.
    #[arg(long, value_name = "KIND")]
    pub self_test: Option<SelfTestKind>,

    /// Task-shape flags (`--task`/`--task-file`/`--interactive`).
    #[command(flatten)]
    pub task_flags: TaskFlags,

    /// Spend / runtime-binding flags (`--claude`/`--net`/`--allow-spend`).
    #[command(flatten)]
    pub spend_flags: SpendFlags,
}

/// Task-shape flags. Reserved for turn 6.
#[derive(Debug, Args)]
pub struct TaskFlags {
    /// Headless task prompt (inline).
    #[arg(long)]
    pub task: Option<String>,
    /// Headless task prompt from a file.
    #[arg(long, value_name = "PATH")]
    pub task_file: Option<PathBuf>,
    /// Run with the cage's own PTY relayed to the terminal.
    #[arg(long)]
    pub interactive: bool,
}

/// Spend / safety flags. Two bools — under the
/// `struct_excessive_bools` threshold by design.
#[derive(Debug, Args)]
pub struct SpendFlags {
    /// Required for any billed mode.
    #[arg(long)]
    pub allow_spend: bool,
    /// Permit a billed run on a dirty tree (default: refuse — plain
    /// git from a clean commit is the recovery story).
    #[arg(long)]
    pub allow_dirty: bool,
}

/// Available no-spend self-tests. New kinds land in later turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SelfTestKind {
    /// Smoke-test the orchestration: cage runs `ctx-access manifest`
    /// over the broker and exits — no model, no network.
    Stub,
}

/// Mode the lifecycle should execute. Computed from [`Cli`] by
/// [`resolve_mode`] so the lifecycle does no argv parsing of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// `--self-test stub`: no spend; runs a brokered probe in the cage.
    SelfTestStub,
    /// Headless billed task: `claude -p <brief>` inside the cage.
    Task(String),
    /// Interactive billed session: bare `claude` with the parent's tty.
    Interactive,
}

/// Spend posture of a [`Mode`]. Used to gate billed runs behind
/// `--allow-spend` / `CTX_CAGE_ALLOW_SPEND=1`.
#[must_use]
pub const fn mode_is_billed(mode: &Mode) -> bool {
    match mode {
        Mode::SelfTestStub => false,
        Mode::Task(_) | Mode::Interactive => true,
    }
}

/// Resolve [`Cli`] flags into a [`Mode`], reading `--task-file` from
/// disk if used. Enforces the spend gate: any billed mode requires
/// `--allow-spend` or `CTX_CAGE_ALLOW_SPEND=1`.
///
/// # Errors
///
/// [`ResolveError::Conflict`] when two modes are requested at once;
/// [`ResolveError::NotAllowed`] when a billed mode is requested
/// without the spend gate; [`ResolveError::Io`] when `--task-file`
/// cannot be read.
pub fn resolve_mode(cli: &Cli, allow_spend: bool) -> Result<Mode, ResolveError> {
    check_flag_conflicts(cli)?;
    let mode = compute_mode(cli)?;
    if mode_is_billed(&mode) && !allow_spend {
        return Err(ResolveError::NotAllowed(
            "billed mode requires --allow-spend or CTX_CAGE_ALLOW_SPEND=1".to_owned(),
        ));
    }
    Ok(mode)
}

/// Reject incompatible flag combinations early so [`compute_mode`]
/// can stay focused on dispatching the chosen mode.
fn check_flag_conflicts(cli: &Cli) -> Result<(), ResolveError> {
    let task_set = cli.task_flags.task.is_some() || cli.task_flags.task_file.is_some();
    if cli.self_test.is_some() && (task_set || cli.task_flags.interactive) {
        return Err(ResolveError::Conflict(
            "--self-test cannot combine with --task/--task-file/--interactive".to_owned(),
        ));
    }
    if cli.task_flags.task.is_some() && cli.task_flags.task_file.is_some() {
        return Err(ResolveError::Conflict(
            "--task and --task-file are mutually exclusive".to_owned(),
        ));
    }
    if task_set && cli.task_flags.interactive {
        return Err(ResolveError::Conflict(
            "--task/--task-file and --interactive are mutually exclusive".to_owned(),
        ));
    }
    Ok(())
}

/// Pick the [`Mode`] given a [`Cli`] that has already passed
/// [`check_flag_conflicts`].
fn compute_mode(cli: &Cli) -> Result<Mode, ResolveError> {
    if cli.self_test == Some(SelfTestKind::Stub) {
        return Ok(Mode::SelfTestStub);
    }
    if cli.task_flags.task.is_some() {
        return Ok(Mode::Task(read_task_inline(cli)?));
    }
    if cli.task_flags.task_file.is_some() {
        return Ok(Mode::Task(read_task_file(cli)?));
    }
    Ok(Mode::Interactive)
}

/// Pull the inline task string out of `cli` (caller guarantees it's
/// `Some`).
fn read_task_inline(cli: &Cli) -> Result<String, ResolveError> {
    cli.task_flags
        .task
        .clone()
        .ok_or_else(|| ResolveError::Conflict("--task missing despite is_some()".to_owned()))
}

/// Read the task brief from `--task-file <path>`.
fn read_task_file(cli: &Cli) -> Result<String, ResolveError> {
    let path = cli
        .task_flags
        .task_file
        .as_ref()
        .ok_or_else(|| ResolveError::Conflict("--task-file missing".to_owned()))?;
    std::fs::read_to_string(path).map_err(ResolveError::Io)
}

/// Errors produced by [`resolve_mode`].
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// Two mutually exclusive modes were requested at once.
    #[error("flag conflict: {0}")]
    Conflict(String),
    /// A billed mode was requested without the spend gate.
    #[error("spend gate: {0}")]
    NotAllowed(String),
    /// `--task-file` could not be read.
    #[error("--task-file io: {0}")]
    Io(#[from] std::io::Error),
}
