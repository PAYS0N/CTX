//! The LLM boundary.
//!
//! The runner is pure over [`Agent`]. The real implementation shells a
//! deployment-configured command (`CTX_AGENT_CMD`) and speaks a tiny,
//! model-agnostic contract: a JSON object `{"system": ..., "user": ...}`
//! is written to the child's stdin; the child's stdout is the completion.
//! Prompt content is never embedded in code — `system` is the verbatim
//! prompt file, `user` is the assembled dynamic data.
//!
//! Configuration comes from the process environment, falling back to
//! the target's gitignored `.env` (the ADR-013 mechanism) so nothing
//! must live in the shell: `CTX_AGENT_CMD` selects the adapter, and an
//! `ANTHROPIC_API_KEY` found in `.env` is exported to the **agent
//! child process only** — never to this process's environment.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;

use crate::error::SummError;

/// Minimum spacing enforced between successive completion requests.
const MIN_REQUEST_SPACING: Duration = Duration::from_secs(1);

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
    /// Extra variables applied to the child's environment only (an
    /// API key sourced from `.env`).
    envs: Vec<(String, String)>,
}

/// The `.env` subset the agent resolution cares about.
#[derive(Debug, Default)]
struct DotEnv {
    /// `CTX_AGENT_CMD=` value, when present and non-empty.
    agent_cmd: Option<String>,
    /// `ANTHROPIC_API_KEY=` value, when present and non-empty.
    api_key: Option<String>,
}

/// Parse the `.env` subset: `KEY=VALUE` lines, `#` comments and blanks
/// skipped, optional surrounding quotes stripped.
fn parse_dotenv(text: &str) -> DotEnv {
    let mut out = DotEnv::default();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, val)) = trimmed.split_once('=') else {
            continue;
        };
        let clean = val.trim().trim_matches('"').trim_matches('\'').to_owned();
        let value = Some(clean).filter(|v| !v.is_empty());
        match key.trim() {
            "CTX_AGENT_CMD" => out.agent_cmd = value,
            "ANTHROPIC_API_KEY" => out.api_key = value,
            _ => {},
        }
    }
    out
}

/// Pure resolution: the process environment wins; `.env` fills gaps.
/// A key taken from `.env` goes only into the child-env additions.
fn resolve_config(
    env_cmd: Option<String>,
    env_key_set: bool,
    dotenv_text: &str,
) -> Result<(String, Vec<(String, String)>), SummError> {
    let dotenv = parse_dotenv(dotenv_text);
    let command = env_cmd
        .filter(|c| !c.trim().is_empty())
        .or(dotenv.agent_cmd)
        .ok_or(SummError::NoAgentCommand)?;
    let mut envs = Vec::new();
    if !env_key_set {
        if let Some(key) = dotenv.api_key {
            envs.push(("ANTHROPIC_API_KEY".to_owned(), key));
        }
    }
    Ok((command, envs))
}

impl SubprocessAgent {
    /// Build from an explicit command string.
    #[must_use]
    pub const fn new(command: String) -> Self {
        Self {
            command,
            envs: Vec::new(),
        }
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

    /// Build from `CTX_AGENT_CMD`, falling back to `<base>/.env` for
    /// both the command and (child-env only) `ANTHROPIC_API_KEY`, so
    /// neither must live in the shell.
    ///
    /// # Errors
    ///
    /// [`SummError::NoAgentCommand`] if neither source configures a
    /// command.
    pub fn from_env_or_dotenv(base: &Path) -> Result<Self, SummError> {
        let text = std::fs::read_to_string(base.join(".env")).unwrap_or_default();
        let (command, envs) = resolve_config(
            std::env::var("CTX_AGENT_CMD").ok(),
            std::env::var("ANTHROPIC_API_KEY").is_ok(),
            &text,
        )?;
        Ok(Self { command, envs })
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
            .envs(self.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
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
        std::thread::sleep(MIN_REQUEST_SPACING);
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_config, SummError};

    /// A `.env` covering both keys, with noise.
    const DOTENV: &str = "\
# comment\n\
ANTHROPIC_API_KEY=\"sk-from-dotenv\"\n\
CTX_AGENT_CMD='python3 adapter.py'\n\
UNRELATED=x\n";

    #[test]
    fn process_env_wins_and_key_is_not_duplicated() {
        let (cmd, envs) =
            resolve_config(Some("real-cmd".to_owned()), true, DOTENV).expect("resolve");
        assert_eq!(cmd, "real-cmd");
        assert!(envs.is_empty(), "process key set -> no child-env addition");
    }

    #[test]
    fn dotenv_fills_both_gaps() {
        let (cmd, envs) = resolve_config(None, false, DOTENV).expect("resolve");
        assert_eq!(cmd, "python3 adapter.py");
        assert_eq!(
            envs,
            vec![("ANTHROPIC_API_KEY".to_owned(), "sk-from-dotenv".to_owned())]
        );
    }

    #[test]
    fn empty_everything_is_no_agent_command() {
        let err = resolve_config(Some("  ".to_owned()), false, "").expect_err("must refuse");
        assert!(matches!(err, SummError::NoAgentCommand));
    }
}
