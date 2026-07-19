//! The `claude` CLI boundary.
//!
//! The runner is pure over [`Claude`]. The real implementation shells the
//! `claude` binary with subscription billing (no API key), running it with
//! its working directory set to the target repo so that repo's own
//! `PostToolUse` context hooks fire on every read. Prompt content is never
//! embedded in code: `system` is a verbatim prompt file, `user` is the
//! assembled dynamic input.
//!
//! Two shapes are exposed. [`Claude::print`] is a headless, output-captured
//! run (`claude -p`) used by the gather and headless-plan stages; the user
//! message is fed on stdin and stdout is the completion. [`Claude::interactive`]
//! inherits the terminal so a human can drive the plan interview; the user
//! message seeds the first turn and the session writes the brief itself.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::BriefError;

/// The two `claude` invocation shapes the runner needs.
pub trait Claude {
    /// Run a headless, output-captured completion (`claude -p`).
    ///
    /// `system` is appended to the default system prompt, `user` is fed on
    /// stdin, `model` selects a model when `Some` (else the CLI default),
    /// and `allowed_tools` is the permission allowlist (empty = none).
    /// `cwd` is the directory the child runs in.
    ///
    /// # Errors
    ///
    /// [`BriefError::Claude`] if the child cannot be run, exits non-zero,
    /// or produces empty output.
    fn print(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
        allowed_tools: &[String],
        cwd: &Path,
    ) -> Result<String, BriefError>;

    /// Run an interactive session with inherited stdio, seeded by `user`.
    ///
    /// # Errors
    ///
    /// [`BriefError::Claude`] if the child cannot be run or exits non-zero.
    fn interactive(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
        cwd: &Path,
    ) -> Result<(), BriefError>;
}

/// [`Claude`] backed by the real `claude` CLI.
#[derive(Debug, Clone)]
pub struct ClaudeCli {
    /// The binary name or path invoked (default `claude`).
    command: String,
}

impl Default for ClaudeCli {
    fn default() -> Self {
        Self {
            command: "claude".to_owned(),
        }
    }
}

impl ClaudeCli {
    /// Build from an explicit binary name or path.
    #[must_use]
    pub const fn new(command: String) -> Self {
        Self { command }
    }
}

/// Append `--model <m>` to `cmd` when a model is named.
fn push_model(cmd: &mut Command, model: Option<&str>) {
    if let Some(m) = model {
        cmd.arg("--model").arg(m);
    }
}

impl Claude for ClaudeCli {
    fn print(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
        allowed_tools: &[String],
        cwd: &Path,
    ) -> Result<String, BriefError> {
        let mut cmd = Command::new(&self.command);
        cmd.arg("-p")
            .arg("--append-system-prompt")
            .arg(system)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        push_model(&mut cmd, model);
        if !allowed_tools.is_empty() {
            cmd.arg("--allowedTools").args(allowed_tools);
        }
        capture(cmd, user)
    }

    fn interactive(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
        cwd: &Path,
    ) -> Result<(), BriefError> {
        let mut cmd = Command::new(&self.command);
        cmd.arg("--append-system-prompt")
            .arg(system)
            .current_dir(cwd);
        push_model(&mut cmd, model);
        cmd.arg(user);
        let status = cmd
            .status()
            .map_err(|e| BriefError::Claude(e.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(BriefError::Claude(format!("exit {:?}", status.code())))
        }
    }
}

/// Spawn `cmd`, feed `user` on stdin, and return trimmed stdout.
fn capture(mut cmd: Command, user: &str) -> Result<String, BriefError> {
    let mut child = cmd.spawn().map_err(|e| BriefError::Claude(e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(user.as_bytes())
            .map_err(|e| BriefError::Claude(e.to_string()))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| BriefError::Claude(e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        return Err(BriefError::Claude(format!(
            "exit {:?}: {err}",
            out.status.code()
        )));
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if text.is_empty() {
        return Err(BriefError::Claude("empty completion".to_owned()));
    }
    Ok(text)
}
