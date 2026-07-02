//! Chain computation: the ordered context nodes from repo root to a
//! target path.
//!
//! Order: for each directory top-down, its `rollup.ctx` then its
//! `intent.md`; a file target additionally ends with its leaf
//! `<file>.ctx`. No source bytes are ever part of a chain — the agent
//! reads source natively; this tool serves only the summary scaffolding
//! above it.

use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// What a chain node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A directory `rollup.ctx`.
    Rollup,
    /// A directory `intent.md`.
    Intent,
    /// The target file's leaf `<file>.ctx`.
    Leaf,
}

impl NodeKind {
    /// Short label, used in served-node headers and absent markers.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rollup => "rollup",
            Self::Intent => "intent",
            Self::Leaf => "leaf",
        }
    }
}

/// One ordered step of a context chain.
#[derive(Debug, Clone)]
pub struct ChainNode {
    /// Repo-relative location of this node's backing file.
    pub id: RepoPath,
    /// The node's role in the chain.
    pub kind: NodeKind,
}

/// The `.context`-mirrored directory for a repo directory.
#[must_use]
pub fn context_dir(dir: &RepoPath) -> RepoPath {
    dir.under(".context")
}

/// Construct one chain node.
const fn node(id: RepoPath, kind: NodeKind) -> ChainNode {
    ChainNode { id, kind }
}

/// Rollup + intent nodes for each of `dirs`, in the given (top-down) order.
fn dir_nodes(dirs: &[RepoPath]) -> Vec<ChainNode> {
    let mut nodes = Vec::new();
    for dir in dirs {
        let cdir = context_dir(dir);
        nodes.push(node(cdir.child("rollup.ctx"), NodeKind::Rollup));
        nodes.push(node(cdir.child("intent.md"), NodeKind::Intent));
    }
    nodes
}

/// Compute the chain for a **file** target: each ancestor directory's
/// rollup + intent, then the target's leaf `<file>.ctx`.
///
/// # Errors
///
/// [`CtxError::PathEscape`] if `target` has no file-name component.
pub fn for_file(target: &RepoPath) -> Result<Vec<ChainNode>, CtxError> {
    let filename = target
        .file_name()
        .ok_or_else(|| CtxError::PathEscape(target.as_string()))?
        .to_owned();
    let dirs = target.dir_chain();
    let mut nodes = dir_nodes(&dirs);
    let parent = dirs.last().map_or_else(RepoPath::root, Clone::clone);
    let leaf = context_dir(&parent).child(&format!("{filename}.ctx"));
    nodes.push(node(leaf, NodeKind::Leaf));
    Ok(nodes)
}

/// Compute the chain for a **directory** target: rollup + intent for
/// every level from the repo root down to (and including) the directory
/// itself — this is how directory summaries are served on demand.
#[must_use]
pub fn for_dir(target: &RepoPath) -> Vec<ChainNode> {
    let mut dirs = target.dir_chain();
    if !target.is_root() {
        dirs.push(target.clone());
    }
    dir_nodes(&dirs)
}
