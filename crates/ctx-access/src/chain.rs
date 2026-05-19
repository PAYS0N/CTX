//! Chain computation: the ordered context nodes from repo root to a
//! target source path.
//!
//! Order per `docs/SPEC.md`: for each directory top-down, its
//! `rollup.ctx` then its `intent.md`; then the target's leaf
//! `<file>.ctx`; then source.

use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// What a chain node is, for stale-banner and shallow-stop decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A directory `rollup.ctx`.
    Rollup,
    /// A directory `intent.md`.
    Intent,
    /// The target file's leaf `<file>.ctx`.
    Leaf,
    /// The target source file itself.
    Source,
}

impl NodeKind {
    /// Short label, used in served-node headers and absent markers.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rollup => "rollup",
            Self::Intent => "intent",
            Self::Leaf => "leaf",
            Self::Source => "source",
        }
    }
}

/// One ordered step of a read chain.
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

/// Compute the full ordered chain for `target` (a source file path).
///
/// # Errors
///
/// [`CtxError::PathEscape`] if `target` has no file-name component.
pub fn build(target: &RepoPath) -> Result<Vec<ChainNode>, CtxError> {
    let filename = target
        .file_name()
        .ok_or_else(|| CtxError::PathEscape(target.as_string()))?
        .to_owned();
    let dirs = target.dir_chain();
    let mut nodes = Vec::new();
    for dir in &dirs {
        let cdir = context_dir(dir);
        nodes.push(node(cdir.child("rollup.ctx"), NodeKind::Rollup));
        nodes.push(node(cdir.child("intent.md"), NodeKind::Intent));
    }
    let parent = dirs.last().map_or_else(RepoPath::root, Clone::clone);
    let leaf = context_dir(&parent).child(&format!("{filename}.ctx"));
    nodes.push(node(leaf, NodeKind::Leaf));
    nodes.push(node(target.clone(), NodeKind::Source));
    Ok(nodes)
}
