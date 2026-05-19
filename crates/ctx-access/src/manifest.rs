//! The deny-by-default manifest — a blinded agent's discovery entrypoint.
//!
//! Lists every readable source path (git-tracked ∩ accessible) with its
//! exact `ctx-access read` invocation. Materialized at
//! `.context/.manifest` on `init-task` so an agent that cannot see the
//! filesystem still has a file to start from.

use std::collections::BTreeSet;

use crate::access;
use crate::cache::validate_task_id;
use crate::env::Env;
use crate::error::CtxError;
use crate::repo_path::RepoPath;

/// The materialized manifest path (regenerated each `init-task`).
#[must_use]
pub fn path() -> RepoPath {
    RepoPath::root().child(".context").child(".manifest")
}

/// Render the manifest body: each readable path + its `read` invocation.
fn render(task_id: &str, accessible: &[String]) -> String {
    let mut s = String::from(
        "# ctx-access manifest — every readable source path + its read\n\
         # invocation. Deny-by-default: secrets, binaries, and gitignored\n\
         # paths are absent here and refused even if requested directly.\n",
    );
    for p in accessible {
        s.push_str(p);
        s.push_str("\tctx-access read ");
        s.push_str(p);
        s.push_str(" --task-id ");
        s.push_str(task_id);
        s.push('\n');
    }
    s
}

/// Compute, materialize at `.context/.manifest`, and return the manifest.
///
/// # Errors
///
/// [`CtxError::InvalidTaskId`]; [`CtxError::Io`] if the tracked set or the
/// manifest file is unavailable.
pub fn build<E: Env>(env: &E, task_id: &str) -> Result<String, CtxError> {
    validate_task_id(task_id)?;
    let tracked: BTreeSet<String> = env.tracked_files()?.into_iter().collect();
    let text = render(task_id, &access::accessible_set(&tracked));
    env.write(&path(), text.as_bytes())?;
    Ok(text)
}
