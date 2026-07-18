//! Argv parsing and output rendering — the thin `cli` layer.
//!
//! Knows nothing about chain logic; it maps parsed arguments onto
//! [`crate::serve`] / [`crate::hook`] calls and renders results to an
//! injected writer (never `println!`, so `clippy::print_stdout` need not
//! be excepted). Path mode additionally folds what it served into
//! [`crate::session`]'s ledger when run inside a Claude Code session, so
//! a manual call and the hook share one dedup record.

use std::io::Write;
use std::path::Path;

use clap::Parser;

use crate::env::Env;
use crate::error::CtxError;
use crate::{hook, serve, session};

/// One-paragraph, agent-facing contract for this binary.
///
/// Single source of truth: the generated tool-contract block in
/// `CLAUDE.md`/`README.md` is assembled from `--contract` output, and the
/// `contracts` battery check fails if that block drifts from this string.
pub const CONTRACT: &str = "ctx-context <path> prints the context chain \
an agent must read before touching <path>: the ancestor rollup.ctx + \
intent.md at each directory level, plus the file's own leaf .ctx for a \
file target (`.` targets the repo root). Read-only and fail-open — a \
missing node renders as an explicit `(absent: …)` marker, never an error; \
a served summary whose source changed since the last regen is prefixed \
`[STALE …]`, and one whose source exists but was never summarized \
`[NEVER GENERATED …]`. `--hook` reads a Claude Code PostToolUse event \
from stdin and emits deduplicated additional-context for the session.";

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

/// Shown in path mode when every node for the requested path was already
/// injected this session — the ledger has nothing left to add.
const NOTHING_NEW: &str =
    "(no new context: everything for this path was already shown this session)\n";

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
    let text = env.env_var(session::ENV_SESSION_ID).map_or_else(
        || serve::chain_for(env, &raw).map(|nodes| serve::render(&nodes)),
        |session_id| {
            serve::fresh_chain_for(env, &session_id, &raw).map(|fresh| {
                fresh.map_or_else(|| NOTHING_NEW.to_owned(), |nodes| serve::render(&nodes))
            })
        },
    )?;
    write!(out, "{text}").map_err(|e| out_err(&e))
}
