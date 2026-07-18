//! Chain serving: resolve a chain's nodes to their contents, with soft
//! absent markers.
//!
//! Context scaffolding is sparse by nature (`intent.md` is
//! owner-authored, not one-per-directory; a rollup or leaf may not exist
//! yet), so a missing node is served as an explicit one-line marker and
//! every chain level stays visible. Nothing here is a hard error except
//! a genuinely unreadable file.

use std::collections::BTreeSet;

use crate::chain::{self, ChainNode, NodeKind};
use crate::env::Env;
use crate::error::CtxError;
use crate::freshness::{self, Assessment};
use crate::repo_path::RepoPath;
use crate::session;

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
    /// How this node's content relates to its source (freshness sidecar).
    pub freshness: Assessment,
}

/// Parse a CLI target string; `.` denotes the repo root, and a trailing
/// `/` on a directory is tolerated.
fn parse_target(raw: &str) -> Result<RepoPath, CtxError> {
    if raw == "." {
        return Ok(RepoPath::root());
    }
    RepoPath::parse(raw.trim_end_matches('/'))
}

/// Serve one node: contents when present, an absent marker otherwise,
/// plus a freshness [`Assessment`] against its hash sidecar.
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
        freshness: freshness::assess(env, node, present),
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

/// Compute and serve the chain for `raw`, dropping any node already
/// injected for `session_id` this session.
///
/// `None` means nothing new-and-present remains (nothing worth showing —
/// absent markers alone carry no context). Shared by hook mode and CLI
/// path mode so a node shown by one is never re-shown by the other, or
/// by a repeat call of either.
///
/// # Errors
///
/// [`CtxError::PathEscape`] for an invalid path; [`CtxError::Io`] if a
/// present node or the session ledger cannot be read/written.
pub fn fresh_chain_for<E: Env>(
    env: &E,
    session_id: &str,
    raw: &str,
) -> Result<Option<Vec<ServedNode>>, CtxError> {
    let mut served: BTreeSet<String> = session::load(env, session_id);
    let nodes: Vec<ServedNode> = chain_for(env, raw)?
        .into_iter()
        .filter(|n| !served.contains(&n.id))
        .collect();
    // A present node is worth showing; so is a NeverGenerated one — an
    // absent artifact whose source exists is a signal, not silence. Plain
    // `(absent: …)` scaffolding alone still counts as nothing new.
    if !nodes.iter().any(worth_showing) {
        return Ok(None);
    }
    for n in &nodes {
        served.insert(n.id.clone());
    }
    session::save(env, session_id, &served)?;
    Ok(Some(nodes))
}

/// Whether a served node carries something worth injecting: real content,
/// or a "never generated" signal. A plain absent marker alone does not.
fn worth_showing(n: &ServedNode) -> bool {
    n.present || n.freshness == Assessment::NeverGenerated
}

/// The one-line marker a node's freshness earns, if any. Distinguishes
/// "content is untrustworthy" (stale / never generated) from the plain
/// `(absent: …)` marker, which means "no such node exists to have".
const fn freshness_marker(a: Assessment) -> Option<&'static str> {
    match a {
        Assessment::Stale => Some("[STALE — source changed since last regen]"),
        Assessment::NeverGenerated => {
            Some("[NEVER GENERATED — source exists but no summary on record]")
        },
        Assessment::Fresh | Assessment::Unknown => None,
    }
}

/// Render served nodes as labeled sections, each prefixed with a freshness
/// marker when its content is stale or was never generated.
#[must_use]
pub fn render(nodes: &[ServedNode]) -> String {
    let mut out = String::new();
    for n in nodes {
        out.push_str("=== ");
        out.push_str(&n.id);
        out.push_str(" [");
        out.push_str(n.kind.label());
        out.push_str("] ===\n");
        if let Some(marker) = freshness_marker(n.freshness) {
            out.push_str(marker);
            out.push('\n');
        }
        out.push_str(&n.body);
        if !n.body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}
