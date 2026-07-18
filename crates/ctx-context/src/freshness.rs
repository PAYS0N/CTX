//! Serve-time freshness: does a context node still match its source?
//!
//! Each served node is checked against its own directory's
//! `.context/<dir>/hashes.json` sidecar — the same `DirNode` schema and
//! `hex_hash` the generator (`ctx-scan`) writes, shared through `ctx-core`
//! so the two cannot disagree. This is a *local* check (one sidecar per
//! node), not a whole-tree recompute: the read hook fires on every file
//! access, so re-hashing the repo per serve would be too costly. The
//! trade-off is that an un-regenerated deep edit surfaces on the changed
//! file's leaf and on its owning directory's rollup, but not on ancestor
//! rollups (whose recorded child hashes have not moved). Everything here
//! is fail-open: no sidecar record ⇒ [`Assessment::Unknown`] ⇒ no marker.

use ctx_core::hashtree::{hex_hash, DirNode};

use crate::chain::{ChainNode, NodeKind};
use crate::env::Env;
use crate::repo_path::RepoPath;

/// How a served node's content relates to the current source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assessment {
    /// No basis to judge — owner-authored intent, no sidecar record, or a
    /// node not backed by hashable source. Serve without a marker.
    Unknown,
    /// The sidecar records this source and the hash still matches.
    Fresh,
    /// The sidecar records this source but it has changed since regen.
    Stale,
    /// Source exists but no summary/record was ever generated for it.
    NeverGenerated,
}

/// Assess a chain node against its hash sidecar. `present` is whether the
/// node's own backing `.ctx`/`rollup.ctx` file exists (from serving).
#[must_use]
pub fn assess<E: Env>(env: &E, node: &ChainNode, present: bool) -> Assessment {
    let id = node.id.as_string();
    let Some(inner) = id.strip_prefix(".context/") else {
        return Assessment::Unknown;
    };
    match node.kind {
        NodeKind::Intent => Assessment::Unknown,
        NodeKind::Rollup => rollup(env, &dir_of_rollup(inner), present),
        NodeKind::Leaf => leaf(env, inner, present),
    }
}

/// Directory (repo-relative, "" = root) owning a rollup whose
/// `.context`-stripped path is `inner` (e.g. `crates/foo/rollup.ctx`).
fn dir_of_rollup(inner: &str) -> String {
    inner
        .strip_suffix("rollup.ctx")
        .unwrap_or(inner)
        .trim_end_matches('/')
        .to_owned()
}

/// Freshness of a leaf whose `.context`-stripped path is `inner`
/// (e.g. `crates/foo/bar.rs.ctx`; source is `crates/foo/bar.rs`).
fn leaf<E: Env>(env: &E, inner: &str, present: bool) -> Assessment {
    let Some(source_rel) = inner.strip_suffix(".ctx") else {
        return Assessment::Unknown;
    };
    if present {
        let (dir_rel, name) = split_parent(source_rel);
        recorded_leaf(env, &dir_rel, name, source_rel)
    } else if source_exists(env, source_rel) {
        Assessment::NeverGenerated
    } else {
        Assessment::Unknown
    }
}

/// A present leaf: compare the recorded `f:<hex>` entry to the source.
fn recorded_leaf<E: Env>(env: &E, dir_rel: &str, name: &str, source_rel: &str) -> Assessment {
    let Some(node) = read_sidecar(env, dir_rel) else {
        return Assessment::Unknown;
    };
    let Some(entry) = node.children.get(name) else {
        return Assessment::Unknown;
    };
    match source_hash(env, source_rel) {
        Some(h) if *entry == format!("f:{h}") => Assessment::Fresh,
        _ => Assessment::Stale,
    }
}

/// Freshness of a rollup for directory `dir_rel` ("" = root).
fn rollup<E: Env>(env: &E, dir_rel: &str, present: bool) -> Assessment {
    if present {
        read_sidecar(env, dir_rel).map_or(Assessment::Unknown, |node| {
            recorded_rollup(env, dir_rel, &node)
        })
    } else if dir_has_source(env, dir_rel) {
        Assessment::NeverGenerated
    } else {
        Assessment::Unknown
    }
}

/// A present rollup: stale if any recorded child's current entry differs
/// from what the sidecar stored.
fn recorded_rollup<E: Env>(env: &E, dir_rel: &str, node: &DirNode) -> Assessment {
    for (name, entry) in &node.children {
        if current_entry(env, dir_rel, name, entry).as_ref() != Some(entry) {
            return Assessment::Stale;
        }
    }
    Assessment::Fresh
}

/// Recompute a child's `f:`/`d:` entry from the current tree, or `None`
/// if the backing source file or subdirectory sidecar is gone.
fn current_entry<E: Env>(env: &E, dir_rel: &str, name: &str, entry: &str) -> Option<String> {
    if entry.starts_with("f:") {
        source_hash(env, &join(dir_rel, name)).map(|h| format!("f:{h}"))
    } else if entry.starts_with("d:") {
        read_sidecar(env, &join(dir_rel, name)).map(|n| format!("d:{}", n.hash))
    } else {
        None
    }
}

/// Read and parse a directory's `hashes.json` sidecar, if present/valid.
fn read_sidecar<E: Env>(env: &E, dir_rel: &str) -> Option<DirNode> {
    let path = if dir_rel.is_empty() {
        ".context/hashes.json".to_owned()
    } else {
        format!(".context/{dir_rel}/hashes.json")
    };
    let rp = RepoPath::parse(&path).ok()?;
    let text = String::from_utf8(env.read(&rp).ok()?).ok()?;
    serde_json::from_str::<DirNode>(&text).ok()
}

/// Hash of a repo-relative source file, or `None` if it is absent.
fn source_hash<E: Env>(env: &E, source_rel: &str) -> Option<String> {
    let rp = RepoPath::parse(source_rel).ok()?;
    if !env.exists(&rp) {
        return None;
    }
    env.read(&rp).ok().map(|b| hex_hash(&b))
}

/// Whether a repo-relative source path exists.
fn source_exists<E: Env>(env: &E, source_rel: &str) -> bool {
    RepoPath::parse(source_rel).is_ok_and(|rp| env.exists(&rp))
}

/// Whether the directory holding a missing rollup exists as a source
/// directory (the root always does; otherwise `is_dir` decides). A source
/// directory is expected to carry a rollup, so its absence is "never
/// generated" rather than legitimately sparse.
fn dir_has_source<E: Env>(env: &E, dir_rel: &str) -> bool {
    dir_rel.is_empty() || RepoPath::parse(dir_rel).is_ok_and(|rp| env.is_dir(&rp))
}

/// Split a repo-relative path into (parent-dir, basename); parent is ""
/// for a top-level path.
fn split_parent(path: &str) -> (String, &str) {
    path.rsplit_once('/').map_or_else(
        || (String::new(), path),
        |(dir, name)| (dir.to_owned(), name),
    )
}

/// Join a possibly-empty directory with a child name.
fn join(dir_rel: &str, name: &str) -> String {
    if dir_rel.is_empty() {
        name.to_owned()
    } else {
        format!("{dir_rel}/{name}")
    }
}
