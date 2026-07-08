//! Lifecycle phase 1: per-run setup and billed-mode gates.
//!
//! Billed modes require the API key (only `ctx-run` provides it) and a
//! clean committed tree — the cage mounts the workspace read-write, so
//! "recovery is plain git" only holds when the session starts from a
//! commit.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::mode_is_billed;
use crate::error::CageError;
use crate::runtime::{resolve_claude_runtime, ClaudeRuntime};
use crate::CAGE_RULES_MD;

use super::Resolved;

/// Resources prepared for one run (cleaned up by `teardown_run`).
#[derive(Debug)]
pub struct Prep {
    /// Tempdir holding the proxy socket, the rules file, the secret
    /// mask file, and (billed) the synthesized claude.json.
    pub rundir: PathBuf,
    /// Host path of the materialized cage-rules.md (bound into cage).
    pub rules_file: PathBuf,
    /// Host path of the empty regular file bound over secret paths.
    pub mask_file: PathBuf,
    /// `Some` for billed modes: the claude runtime binds.
    pub claude: Option<ClaudeRuntime>,
}

/// Gate billed modes, mint the run dir, write the rules file, and
/// resolve the claude runtime when needed.
///
/// # Errors
///
/// [`CageError::Protocol`] when a billed mode's tree is dirty or the
/// claude runtime / subscription credential is missing;
/// [`CageError::Io`] on any filesystem failure.
pub fn prepare_run(r: &Resolved) -> Result<Prep, CageError> {
    let billed = mode_is_billed(&r.mode);
    if billed {
        ensure_clean_tree(&r.target_root, r.allow_dirty)?;
    }
    let rundir = mint_rundir(&r.task_id)?;
    let rules_file = write_rules_file(&rundir)?;
    let mask_file = write_mask_file(&rundir)?;
    let claude = if billed {
        Some(resolve_claude_runtime(&rundir, &r.target_root)?)
    } else {
        None
    };
    Ok(Prep {
        rundir,
        rules_file,
        mask_file,
        claude,
    })
}

/// Create the empty regular file bound over secret paths (`/dev/null`
/// is unusable: bind mounts carry `nodev`).
fn write_mask_file(rundir: &Path) -> Result<PathBuf, CageError> {
    let path = rundir.join("empty-mask");
    std::fs::write(&path, b"")?;
    Ok(path)
}

/// Refuse a billed launch on a dirty tree unless overridden: the
/// session's undo story is `git` against the pre-run commit.
fn ensure_clean_tree(root: &Path, allow_dirty: bool) -> Result<(), CageError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .output()?;
    if !out.status.success() {
        return Err(CageError::Protocol(format!(
            "{} is not a git repository (billed runs need git as the recovery path)",
            root.display()
        )));
    }
    if !out.stdout.is_empty() && !allow_dirty {
        return Err(CageError::Protocol(
            "target tree is dirty; commit/stash first (or pass --allow-dirty)".to_owned(),
        ));
    }
    Ok(())
}

/// Create a fresh tempdir under `TMPDIR` named after the task and pid.
fn mint_rundir(task_id: &str) -> Result<PathBuf, CageError> {
    let parent = std::env::temp_dir();
    let dir = parent.join(format!("ctxcage-{task_id}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Write the embedded cage-rules markdown to `<rundir>/cage-rules.md`.
fn write_rules_file(rundir: &Path) -> Result<PathBuf, CageError> {
    let path = rundir.join("cage-rules.md");
    std::fs::write(&path, CAGE_RULES_MD)?;
    Ok(path)
}
