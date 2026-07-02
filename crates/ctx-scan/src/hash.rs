//! CACT-style content-hash tree for change detection.
//!
//! Freshness of the summary tree is decided by content hashes carried in
//! the tree itself, never by git state (which would couple summary
//! freshness to commit state and miss gitignore-invisible edits). A leaf
//! entry is the SHA-256 of its source file; a directory node's hash is
//! the SHA-256 of its sorted children entries, so any change propagates
//! to the root. Each mirrored directory stores its node in
//! `.context/<dir>/hashes.json`; diffing stored against recomputed state
//! yields exactly the stale rollup directories and stale leaf summaries.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use ctx_summarize::cpath;
use ctx_summarize::fs::Fs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ScanError;

/// One directory's hash node as stored in its sidecar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirNode {
    /// Aggregate hash over the sorted `children` entries.
    pub hash: String,
    /// Child name -> `f:<hex>` (file) or `d:<hex>` (subdirectory).
    pub children: BTreeMap<String, String>,
}

/// Hash nodes for every directory in scope, keyed by repo-relative
/// directory (`""` = root).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeState {
    /// Directory -> node.
    pub dirs: BTreeMap<String, DirNode>,
}

/// What separates a fresh tree from a stale one.
#[derive(Debug, Default)]
pub struct Staleness {
    /// Directories whose rollup is stale, deepest-first.
    pub stale_dirs: Vec<String>,
    /// Source files whose leaf summary is stale (new or changed).
    pub changed_files: Vec<String>,
    /// Leaf `.ctx` paths whose source file no longer exists.
    pub orphan_leaves: Vec<String>,
}

impl Staleness {
    /// True when nothing needs regeneration.
    #[must_use]
    pub const fn is_fresh(&self) -> bool {
        self.stale_dirs.is_empty() && self.changed_files.is_empty() && self.orphan_leaves.is_empty()
    }
}

/// Hex SHA-256 of `bytes`.
fn hex_hash(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        let r = write!(s, "{b:02x}");
        if r.is_err() {}
    }
    s
}

/// Split a repo-relative path into (parent dir, basename).
fn split_parent(path: &str) -> (String, String) {
    path.rsplit_once('/').map_or_else(
        || (String::new(), path.to_owned()),
        |(d, n)| (d.to_owned(), n.to_owned()),
    )
}

/// `dir/name`, with the root directory handled.
fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// Directory depth (`""` = 0).
fn depth(dir: &str) -> usize {
    if dir.is_empty() {
        0
    } else {
        dir.split('/').count()
    }
}

/// Fill each directory's aggregate hash deepest-first, propagating a
/// `d:<hex>` entry into its parent's children.
fn aggregate(dirs: &mut BTreeMap<String, DirNode>, all: &BTreeSet<String>) {
    let mut ordered: Vec<String> = all.iter().cloned().collect();
    ordered.sort_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));
    for d in ordered {
        let node_hash = {
            let node = dirs.entry(d.clone()).or_default();
            let mut acc = String::new();
            for (name, h) in &node.children {
                acc.push_str(name);
                acc.push('=');
                acc.push_str(h);
                acc.push('\n');
            }
            hex_hash(acc.as_bytes())
        };
        if let Some(node) = dirs.get_mut(&d) {
            node.hash.clone_from(&node_hash);
        }
        if !d.is_empty() {
            let (parent, name) = split_parent(&d);
            dirs.entry(parent)
                .or_default()
                .children
                .insert(name, format!("d:{node_hash}"));
        }
    }
}

/// Recompute the current hash tree for `files` (repo-relative) under
/// `base`.
///
/// # Errors
///
/// [`ScanError::Io`] if a source file cannot be read.
pub fn compute(base: &Path, files: &[String]) -> Result<TreeState, ScanError> {
    let mut dirs: BTreeMap<String, DirNode> = BTreeMap::new();
    let mut all_dirs: BTreeSet<String> = BTreeSet::new();
    for f in files {
        for d in cpath::ancestor_dirs(f) {
            all_dirs.insert(d);
        }
        let bytes = fs::read(base.join(f)).map_err(|e| ScanError::Io {
            path: f.clone(),
            detail: e.to_string(),
        })?;
        let (dir, name) = split_parent(f);
        dirs.entry(dir)
            .or_default()
            .children
            .insert(name, format!("f:{}", hex_hash(&bytes)));
    }
    aggregate(&mut dirs, &all_dirs);
    Ok(TreeState { dirs })
}

/// Sidecar path for `dir`.
fn sidecar(dir: &str) -> String {
    format!("{}/hashes.json", cpath::context_dir_of(dir))
}

/// Load the stored nodes for every directory `current` covers; a missing
/// or corrupt sidecar simply reads as absent (⇒ that directory is stale).
pub fn load_stored<F: Fs>(fs: &F, current: &TreeState) -> TreeState {
    let mut dirs = BTreeMap::new();
    for d in current.dirs.keys() {
        let p = sidecar(d);
        if !fs.exists(&p) {
            continue;
        }
        if let Ok(text) = fs.read(&p) {
            if let Ok(node) = serde_json::from_str::<DirNode>(&text) {
                dirs.insert(d.clone(), node);
            }
        }
    }
    TreeState { dirs }
}

/// Persist every node of `state` to its sidecar.
///
/// # Errors
///
/// [`ScanError::Io`] on encoding failure; propagates write failures.
pub fn store<F: Fs>(fs: &F, state: &TreeState) -> Result<(), ScanError> {
    for (d, node) in &state.dirs {
        let text = serde_json::to_string_pretty(node).map_err(|e| ScanError::Io {
            path: sidecar(d),
            detail: e.to_string(),
        })?;
        fs.write(&sidecar(d), &text)?;
    }
    Ok(())
}

/// Record per-directory file-level differences into `out`.
fn diff_children(dir: &str, node: &DirNode, old: Option<&DirNode>, out: &mut Staleness) {
    for (name, h) in &node.children {
        if !h.starts_with("f:") {
            continue;
        }
        let same = old.is_some_and(|o| o.children.get(name) == Some(h));
        if !same {
            out.changed_files.push(join(dir, name));
        }
    }
    let Some(o) = old else { return };
    for (name, h) in &o.children {
        if h.starts_with("f:") && !node.children.contains_key(name) {
            out.orphan_leaves
                .push(format!("{}/{name}.ctx", cpath::context_dir_of(dir)));
        }
    }
}

/// Compare `current` against `stored`, yielding what must regenerate.
#[must_use]
pub fn diff(current: &TreeState, stored: &TreeState) -> Staleness {
    let mut out = Staleness::default();
    for (d, node) in &current.dirs {
        let old = stored.dirs.get(d);
        if old.is_none_or(|o| o.hash != node.hash) {
            out.stale_dirs.push(d.clone());
        }
        diff_children(d, node, old, &mut out);
    }
    out.stale_dirs
        .sort_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));
    out
}
