//! Binds the shared `ctx_core` access gate to `CtxError`.
//!
//! The gate logic (secret/binary/gitignored deny) is single-sourced in
//! `ctx_core::access` so it cannot drift from `ctx-summarize`'s copy
//! (a divergent secret denylist would be a silent exfil bug — ADR-023).
//! This module only maps the neutral deny reason into `CtxError`.

use std::collections::BTreeSet;

use crate::error::CtxError;

/// Check `path` against the gate. `ignored` is `Env::is_ignored(path)`.
///
/// # Errors
///
/// [`CtxError::AccessDenied`] if `path` is secret, binary, or gitignored.
pub fn check(path: &str, ignored: bool) -> Result<(), CtxError> {
    ctx_core::access::deny_reason(path, ignored).map_or(Ok(()), |reason| {
        Err(CtxError::AccessDenied {
            path: path.to_owned(),
            reason: reason.to_owned(),
        })
    })
}

/// The accessible subset of `tracked`, sorted — the manifest's body.
#[must_use]
pub fn accessible_set(tracked: &BTreeSet<String>) -> Vec<String> {
    ctx_core::access::accessible_set(tracked)
}
