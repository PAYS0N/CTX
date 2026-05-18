//! The pure enforcement core.
//!
//! Every operation is a free function over an injected [`Env`]; nothing
//! here touches argv, stdout, sockets, or the process boundary. This is
//! the body that moves behind `ctx-broker` later (see `docs/SANDBOX.md`).

use crate::cache::{cache_path, validate_task_id, TaskCache};
use crate::chain::{self, ChainNode, NodeKind};
use crate::env::Env;
use crate::error::CtxError;
use crate::repo_path::RepoPath;
use crate::report::report_path;
pub use crate::report::{Divergence, EndReport, NoopSummarizer, Summarizer};

/// Banner prepended to context nodes whose source was written this task.
const STALE_BANNER: &str = "STALE — modified in current task\n";

/// One context/source node as served to the agent.
#[derive(Debug, Clone)]
pub struct ServedNode {
    /// Repo-relative identifier of the backing file.
    pub id: String,
    /// The node's role in the chain.
    pub kind: NodeKind,
    /// File contents (UTF-8 lossy), stale banner prepended if applicable.
    pub body: String,
}

/// The ordered set of nodes a single `read` returns.
#[derive(Debug, Clone)]
pub struct ReadResponse {
    /// Nodes not previously served this task, in chain order.
    pub nodes: Vec<ServedNode>,
}

/// Leaf `<file>.ctx` and source ids for `target`, for stale eviction.
///
/// # Errors
///
/// [`CtxError::PathEscape`] if the chain cannot be built for `target`.
fn leaf_and_source_ids(target: &RepoPath) -> Result<Vec<String>, CtxError> {
    let mut ids = Vec::new();
    for node in chain::build(target)? {
        if matches!(node.kind, NodeKind::Leaf | NodeKind::Source) {
            ids.push(node.id.as_string());
        }
    }
    Ok(ids)
}

/// Create a fresh task cache.
///
/// # Errors
///
/// [`CtxError::InvalidTaskId`] for a bad id; [`CtxError::TaskExists`] if a
/// cache already exists and `force` is false; [`CtxError::Io`] on write.
pub fn init_task<E: Env>(env: &E, task_id: &str, force: bool) -> Result<(), CtxError> {
    validate_task_id(task_id)?;
    if env.exists(&cache_path(task_id)) && !force {
        return Err(CtxError::TaskExists(task_id.to_owned()));
    }
    let started_at = env.now_unix()?;
    TaskCache::new(task_id.to_owned(), started_at).save(env)
}

/// Serve one chain node, or `None` if already served this task.
///
/// # Errors
///
/// [`CtxError::MissingNode`] if the node's file is absent;
/// [`CtxError::Io`] on read failure.
fn serve_node<E: Env>(
    env: &E,
    cache: &mut TaskCache,
    node: &ChainNode,
    stale: bool,
) -> Result<Option<ServedNode>, CtxError> {
    let id = node.id.as_string();
    if cache.has_served(&id) {
        return Ok(None);
    }
    if !env.exists(&node.id) {
        return Err(CtxError::MissingNode(id));
    }
    let raw = env.read(&node.id)?;
    let decoded = String::from_utf8_lossy(&raw).into_owned();
    let banner_applies = stale && node.kind != NodeKind::Source;
    let body = if banner_applies {
        format!("{STALE_BANNER}{decoded}")
    } else {
        decoded
    };
    cache.mark_served(id.clone());
    Ok(Some(ServedNode {
        id,
        kind: node.kind,
        body,
    }))
}

/// Serve the unserved prefix of `target`'s chain (single call).
///
/// # Errors
///
/// [`CtxError::TaskMissing`] without prior `init-task`;
/// [`CtxError::PathEscape`] for a bad path; [`CtxError::MissingNode`] /
/// [`CtxError::Io`] on node access.
pub fn read<E: Env>(
    env: &E,
    task_id: &str,
    target: &str,
    shallow: bool,
) -> Result<ReadResponse, CtxError> {
    validate_task_id(task_id)?;
    let mut cache = TaskCache::load(env, task_id)?;
    let target_path = RepoPath::parse(target)?;
    let stale = cache.has_written(&target_path.as_string());
    let mut nodes = Vec::new();
    for node in chain::build(&target_path)? {
        if shallow && node.kind == NodeKind::Source {
            break;
        }
        if let Some(served) = serve_node(env, &mut cache, &node, stale)? {
            nodes.push(served);
        }
    }
    cache.save(env)?;
    Ok(ReadResponse { nodes })
}

/// Write source, requiring a prior non-shallow `read` of the same path.
///
/// # Errors
///
/// [`CtxError::TaskMissing`], [`CtxError::PathEscape`],
/// [`CtxError::WriteWithoutRead`] if source was not read this task, or
/// [`CtxError::Io`] on write.
pub fn write<E: Env>(env: &E, task_id: &str, target: &str, content: &[u8]) -> Result<(), CtxError> {
    validate_task_id(task_id)?;
    let mut cache = TaskCache::load(env, task_id)?;
    let target_path = RepoPath::parse(target)?;
    let source_id = target_path.as_string();
    if !cache.has_served(&source_id) {
        return Err(CtxError::WriteWithoutRead {
            path: source_id,
            task: task_id.to_owned(),
        });
    }
    env.write(&target_path, content)?;
    cache.mark_written(source_id);
    cache.evict(&leaf_and_source_ids(&target_path)?);
    cache.save(env)
}

/// List a directory, requiring its `rollup.ctx` to have been served.
///
/// # Errors
///
/// [`CtxError::TaskMissing`], [`CtxError::PathEscape`],
/// [`CtxError::ListWithoutRollup`], or [`CtxError::Io`].
pub fn list<E: Env>(env: &E, task_id: &str, dir: &str) -> Result<Vec<String>, CtxError> {
    validate_task_id(task_id)?;
    let cache = TaskCache::load(env, task_id)?;
    let dir_path = RepoPath::parse(dir)?;
    let rollup_id = chain::context_dir(&dir_path)
        .child("rollup.ctx")
        .as_string();
    if !cache.has_served(&rollup_id) {
        return Err(CtxError::ListWithoutRollup {
            dir: dir_path.as_string(),
            task: task_id.to_owned(),
        });
    }
    env.list_dir(&dir_path)
}

/// Finalize a task: run audit, write the report, delete the cache.
///
/// # Errors
///
/// [`CtxError::TaskMissing`] if no cache exists; propagates summarizer and
/// [`CtxError::Io`] failures.
pub fn end_task<E: Env, S: Summarizer>(
    env: &E,
    task_id: &str,
    summarizer: &S,
) -> Result<EndReport, CtxError> {
    validate_task_id(task_id)?;
    let cache = TaskCache::load(env, task_id)?;
    let divergences = summarizer.run(&cache.paths_written)?;
    let report = EndReport {
        task_id: task_id.to_owned(),
        completed_at: env.now_unix()?,
        divergences,
    };
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|_| CtxError::CorruptCache(task_id.to_owned()))?;
    env.write(&report_path(task_id), &bytes)?;
    env.remove(&cache_path(task_id))?;
    Ok(report)
}
