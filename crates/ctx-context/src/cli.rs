//! Argv parsing and output rendering — the thin `cli` layer.
//!
//! Knows nothing about chain logic; it maps parsed arguments onto
//! [`crate::serve`] / [`crate::hook`] calls and renders results to an
//! injected writer (never `println!`, so `clippy::print_stdout` need not
//! be excepted).

use std::io::Write;
use std::path::Path;

use clap::Parser;

use crate::env::Env;
use crate::error::CtxError;
use crate::{hook, serve};

/// The context-chain server.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-context",
    about = "Print the context chain (rollups + intent) for a path"
)]
pub struct Cli {
    /// Repo-relative file or directory (`.` for the repo root).
    path: Option<String>,
    /// Claude Code `PostToolUse` hook mode: read the hook event JSON from
    /// stdin and emit an additional-context payload (fail-open).
    #[arg(long)]
    hook: bool,
}

impl Cli {
    /// Whether stdin must be consumed before dispatch.
    #[must_use]
    pub const fn hook_mode(&self) -> bool {
        self.hook
    }
}

/// Wrap a writer error as [`CtxError::Io`].
fn out_err(e: &std::io::Error) -> CtxError {
    CtxError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    }
}

/// Execute the parsed command against `env`, rendering to `out`. `base`
/// is the absolute repo root (used by hook mode to relativize paths);
/// `stdin` is consumed only in hook mode.
///
/// # Errors
///
/// [`CtxError::Usage`] when neither a path nor `--hook` was given;
/// propagates chain and writer failures in path mode. Hook mode never
/// fails except on a broken writer (fail-open by design).
pub fn dispatch<E: Env, W: Write>(
    env: &E,
    base: &Path,
    cli: Cli,
    stdin: &str,
    out: &mut W,
) -> Result<(), CtxError> {
    if cli.hook {
        let json = hook::run(env, base, stdin);
        if json.is_empty() {
            return Ok(());
        }
        return writeln!(out, "{json}").map_err(|e| out_err(&e));
    }
    let Some(raw) = cli.path else {
        return Err(CtxError::Usage("a path (or --hook) is required".to_owned()));
    };
    let nodes = serve::chain_for(env, &raw)?;
    write!(out, "{}", serve::render(&nodes)).map_err(|e| out_err(&e))
}
