//! Chain serving: resolve a chain's nodes to their contents, with soft
//! absent markers.
//!
//! Context scaffolding is sparse by nature (`intent.md` is
//! owner-authored, not one-per-directory; a rollup or leaf may not exist
//! yet), so a missing node is served as an explicit one-line marker and
//! every chain level stays visible. Nothing here is a hard error except
//! a genuinely unreadable file.

use crate::chain::{self, ChainNode, NodeKind};
use crate::env::Env;
use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// One context node as served.
#[derive(Debug, Clone)]
pub struct ServedNode {
    /// Repo-relative identifier of the backing file.
    pub id: String,
    /// The node's role in the chain.
    pub kind: NodeKind,
    /// File contents (UTF-8 lossy), or an `(absent: …)` marker.
    pub body: String,
    /// Whether the backing file exists (`false` ⇒ marker body).
    pub present: bool,
}

/// Parse a CLI target string; `.` denotes the repo root, and a trailing
/// `/` on a directory is tolerated.
fn parse_target(raw: &str) -> Result<RepoPath, CtxError> {
    if raw == "." {
        return Ok(RepoPath::root());
    }
    RepoPath::parse(raw.trim_end_matches('/'))
}

/// Serve one node: contents when present, an absent marker otherwise.
fn serve_one<E: Env>(env: &E, node: &ChainNode) -> Result<ServedNode, CtxError> {
    let present = env.exists(&node.id);
    let body = if present {
        String::from_utf8_lossy(&env.read(&node.id)?).into_owned()
    } else {
        format!("(absent: no {} at this level)", node.kind.label())
    };
    Ok(ServedNode {
        id: node.id.as_string(),
        kind: node.kind,
        body,
        present,
    })
}

/// Compute and serve the full chain for `raw` (a file or directory).
///
/// A directory target (or `.`) yields rollup + intent down to and
/// including the directory itself; a file target additionally ends with
/// its leaf `<file>.ctx`.
///
/// # Errors
///
/// [`CtxError::PathEscape`] for an invalid path; [`CtxError::Io`] if a
/// present node cannot be read.
pub fn chain_for<E: Env>(env: &E, raw: &str) -> Result<Vec<ServedNode>, CtxError> {
    let target = parse_target(raw)?;
    let nodes = if target.is_root() || env.is_dir(&target) {
        chain::for_dir(&target)
    } else {
        chain::for_file(&target)?
    };
    nodes.iter().map(|n| serve_one(env, n)).collect()
}

/// Render served nodes as labeled sections.
#[must_use]
pub fn render(nodes: &[ServedNode]) -> String {
    let mut out = String::new();
    for n in nodes {
        out.push_str("=== ");
        out.push_str(&n.id);
        out.push_str(" [");
        out.push_str(n.kind.label());
        out.push_str("] ===\n");
        out.push_str(&n.body);
        if !n.body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
