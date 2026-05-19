//! Process-execution boundary.
//!
//! [`checks`](crate::checks) is pure over the [`Runner`] trait: the real
//! runner shells `std::process::Command`; tests inject canned output.
//! Same seam idea as `ctx-access`'s `Env`.

use std::io::ErrorKind;
use std::process::Command;

use crate::error::CheckError;

/// The captured result of one external command.
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    /// Process exit code, if the process exited normally.
    pub code: Option<i32>,
    /// Captured standard output (UTF-8 lossy).
    pub stdout: String,
    /// Captured standard error (UTF-8 lossy).
    pub stderr: String,
}

impl CommandOutcome {
    /// Whether the process exited with code zero.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

/// Runs external tools. A trait so checks are unit-testable offline.
pub trait Runner {
    /// Run `tool` with `args` and extra environment `envs`.
    ///
    /// # Errors
    ///
    /// [`CheckError::ToolMissing`] if the binary is absent;
    /// [`CheckError::Spawn`] for any other spawn/wait failure.
    fn run(
        &self,
        tool: &str,
        args: &[&str],
        envs: &[(&str, &str)],
    ) -> Result<CommandOutcome, CheckError>;
}

/// Real runner over `std::process::Command`.
#[derive(Debug, Clone, Copy)]
pub struct ProcRunner;

impl Runner for ProcRunner {
    fn run(
        &self,
        tool: &str,
        args: &[&str],
        envs: &[(&str, &str)],
    ) -> Result<CommandOutcome, CheckError> {
        let mut cmd = Command::new(tool);
        cmd.args(args);
        for (key, value) in envs {
            cmd.env(key, value);
        }
        let out = cmd.output().map_err(|e| {
            if e.kind() == ErrorKind::NotFound {
                CheckError::ToolMissing(tool.to_owned())
            } else {
                CheckError::Spawn {
                    tool: tool.to_owned(),
                    detail: e.to_string(),
                }
            }
        })?;
        Ok(CommandOutcome {
            code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}
