//! Lifecycle phase 1: per-run setup. Mints a sockdir, materializes
//! the cage-rules file, runs host-side `ctx-access init-task`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bwrap::ClaudeBinds;
use crate::error::CageError;
use crate::runtime::resolve_claude_binds;
use crate::CAGE_RULES_MD;

use super::Resolved;

/// Resources prepared for one run (cleaned up by `teardown_run`).
#[derive(Debug)]
pub struct Prep {
    /// Tempdir holding the socket, the rules file, and (for `--claude`)
    /// the synthesized nsswitch + claude.json.
    pub sockdir: PathBuf,
    /// Basename of the socket file inside `sockdir`.
    pub sockname: String,
    /// Host path of the materialized cage-rules.md (bound into cage).
    pub rules_file: PathBuf,
    /// `Some` when `Resolved::claude_runtime` is `true`: the host paths
    /// the bwrap builder binds for the `--claude` mounts.
    pub claude_binds: Option<ClaudeBinds>,
}

/// Mint a sockdir, write the rules file, optionally resolve the
/// claude runtime, and run `init-task`.
///
/// # Errors
///
/// [`CageError::Io`] on any filesystem failure;
/// [`CageError::Protocol`] if `init-task` exits non-zero or the
/// `--claude` runtime cannot be resolved.
pub fn prepare_run(r: &Resolved) -> Result<Prep, CageError> {
    let sockdir = mint_sockdir(&r.task_id)?;
    let rules_file = write_rules_file(&sockdir)?;
    let claude_binds = if r.claude_runtime {
        Some(resolve_claude_binds(&sockdir)?)
    } else {
        None
    };
    run_init_task(r)?;
    Ok(Prep {
        sockdir,
        sockname: "ctx.sock".to_owned(),
        rules_file,
        claude_binds,
    })
}

/// Create a fresh tempdir under `TMPDIR` named after the task and pid.
fn mint_sockdir(task_id: &str) -> Result<PathBuf, CageError> {
    let parent = std::env::temp_dir();
    let dir = parent.join(format!("ctxcage-{task_id}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Write the embedded cage-rules markdown to `<sockdir>/cage-rules.md`.
fn write_rules_file(sockdir: &Path) -> Result<PathBuf, CageError> {
    let path = sockdir.join("cage-rules.md");
    std::fs::write(&path, CAGE_RULES_MD)?;
    Ok(path)
}

/// `ctx-access init-task --task-id <id> --force` in the target root.
fn run_init_task(r: &Resolved) -> Result<(), CageError> {
    let status = Command::new(&r.ctx_access_bin)
        .args(["init-task", "--task-id", &r.task_id, "--force"])
        .current_dir(&r.target_root)
        .status()?;
    if !status.success() {
        return Err(CageError::Protocol(format!(
            "ctx-access init-task failed: {status}"
        )));
    }
    Ok(())
}
