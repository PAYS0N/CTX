//! The LLM boundary.
//!
//! The runner is pure over [`Agent`]. The real implementation shells a
//! deployment-configured command (`CTX_AGENT_CMD`) and speaks a tiny,
//! model-agnostic contract: a JSON object `{"system": ..., "user": ...}`
//! is written to the child's stdin; the child's stdout is the completion.
//! Prompt content is never embedded in code — `system` is the verbatim
//! prompt file, `user` is the assembled dynamic data.

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::error::SummError;

/// One completion request/response.
pub trait Agent {
    /// Run one completion. `system` is the prompt file verbatim; `user`
    /// is the assembled dynamic input. Returns the model's text output.
    ///
    /// # Errors
    ///
    /// [`SummError::Agent`] if the agent cannot be run, exits non-zero,
    /// or produces empty output.
    fn complete(&self, system: &str, user: &str) -> Result<String, SummError>;
}

/// The JSON payload written to the agent command's stdin.
#[derive(Serialize)]
struct Payload<'a> {
    /// Verbatim prompt-file contents.
    system: &'a str,
    /// Assembled dynamic input data.
    user: &'a str,
}

/// Agent backed by a deployment-configured shell command.
#[derive(Debug, Clone)]
pub struct SubprocessAgent {
    /// The command run via `sh -c`; reads the JSON request on stdin.
    command: String,
}

impl SubprocessAgent {
    /// Build from an explicit command string.
    #[must_use]
    pub const fn new(command: String) -> Self {
        Self { command }
    }

    /// Build from `CTX_AGENT_CMD`.
    ///
    /// # Errors
    ///
    /// [`SummError::NoAgentCommand`] if the variable is unset or empty.
    pub fn from_env() -> Result<Self, SummError> {
        match std::env::var("CTX_AGENT_CMD") {
            Ok(cmd) if !cmd.trim().is_empty() => Ok(Self::new(cmd)),
            _ => Err(SummError::NoAgentCommand),
        }
    }
}

impl Agent for SubprocessAgent {
    // rationale: one linear request/response sequence (encode -> spawn -> feed stdin -> collect -> validate); splitting it would scatter the single I/O transaction.
    fn complete(&self, system: &str, user: &str) -> Result<String, SummError> {
        let payload = serde_json::to_vec(&Payload { system, user })
            .map_err(|e| SummError::Agent(e.to_string()))?;
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&self.command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SummError::Agent(e.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&payload)
                .map_err(|e| SummError::Agent(e.to_string()))?;
        }
        let out = child
            .wait_with_output()
            .map_err(|e| SummError::Agent(e.to_string()))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).into_owned();
            return Err(SummError::Agent(format!(
                "exit {:?}: {err}",
                out.status.code()
            )));
        }
        let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if text.is_empty() {
            return Err(SummError::Agent("empty completion".to_owned()));
        }
        Ok(text)
    }
}
