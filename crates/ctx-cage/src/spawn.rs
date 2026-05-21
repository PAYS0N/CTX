//! Process boundary for the broker.
//!
//! The [`Spawner`] trait is the seam the broker is pure over: tests
//! inject a canned spawner; the real runner shells out via
//! [`StdSpawner`]. Same shape as `ctx-verify::runner::Runner` and
//! `ctx-access::Env`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::error::CageError;

/// Run an allowlisted tool. The broker calls `emit(chunk)` zero or
/// more times with output bytes and returns the tool's exit code.
pub trait Spawner: Send + Sync {
    /// Run `tool args`, calling `emit` for each output chunk
    /// (stdout/stderr merged) and returning the exit code.
    ///
    /// # Errors
    ///
    /// Spawner-specific; the broker maps any error into an `Output`
    /// frame plus an `Exit(95)` frame so the client always sees a
    /// well-formed response.
    fn spawn(
        &self,
        tool: &str,
        args: &[String],
        emit: &mut dyn FnMut(&[u8]) -> Result<(), CageError>,
    ) -> Result<u32, CageError>;
}

/// Real runner: each request becomes one [`Command`] in `cwd`, with
/// stdout and stderr captured and merged into a single output buffer
/// (the in-cage agent sees the same combined stream the host would).
#[derive(Debug, Clone)]
pub struct StdSpawner {
    /// Working directory for the brokered tools (the target project).
    pub cwd: PathBuf,
    /// `tool name -> binary path`. Tools not in this map are rejected
    /// before the spawner ever sees them (broker allowlist check), but
    /// we double-check here for defense in depth.
    pub tool_paths: HashMap<String, PathBuf>,
}

impl Spawner for StdSpawner {
    fn spawn(
        &self,
        tool: &str,
        args: &[String],
        emit: &mut dyn FnMut(&[u8]) -> Result<(), CageError>,
    ) -> Result<u32, CageError> {
        let bin = self
            .tool_paths
            .get(tool)
            .ok_or_else(|| CageError::UnknownTool(tool.to_owned()))?;
        let out = Command::new(bin)
            .args(args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .output()?;
        if !out.stdout.is_empty() {
            emit(&out.stdout)?;
        }
        if !out.stderr.is_empty() {
            emit(&out.stderr)?;
        }
        Ok(exit_code(out.status))
    }
}

/// Map a [`std::process::ExitStatus`] to a `u32` exit code. A
/// signal-terminated child (no `.code()`) collapses to `1` — the same
/// convention `sh`'s `$?` uses when given nothing better.
fn exit_code(status: std::process::ExitStatus) -> u32 {
    status
        .code()
        .and_then(|c| u32::try_from(c).ok())
        .unwrap_or(1)
}
