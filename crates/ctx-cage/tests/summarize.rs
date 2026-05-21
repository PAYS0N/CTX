//! Tests for the pure pieces of `summarize`. The impure wrappers
//! (`list_tracked_*`, `run_summarize_*`) shell out to `git` /
//! `ctx-summarize` and are exercised by turn-8 end-to-end tests, not
//! here.

use std::path::PathBuf;

use ctx_cage::summarize::{compute_stale, expected_leaf_path};

#[test]
fn leaf_for_crate_source_is_under_dot_context() {
    let src = PathBuf::from("crates/x/src/foo.rs");
    assert_eq!(
        expected_leaf_path(&src),
        PathBuf::from(".context/crates/x/src/foo.rs.ctx")
    );
}

#[test]
fn leaf_for_nested_module_is_under_dot_context() {
    let src = PathBuf::from("crates/x/src/cli/handlers.rs");
    assert_eq!(
        expected_leaf_path(&src),
        PathBuf::from(".context/crates/x/src/cli/handlers.rs.ctx")
    );
}

#[test]
fn empty_inputs_yield_no_stale() {
    assert!(compute_stale(&[], &[]).is_empty());
}

#[test]
fn source_with_matching_leaf_is_not_stale() {
    let sources = vec![PathBuf::from("crates/x/src/lib.rs")];
    let leaves = vec![PathBuf::from(".context/crates/x/src/lib.rs.ctx")];
    assert!(compute_stale(&sources, &leaves).is_empty());
}

#[test]
fn source_without_matching_leaf_is_stale() {
    let sources = vec![
        PathBuf::from("crates/x/src/lib.rs"),
        PathBuf::from("crates/x/src/foo.rs"),
    ];
    let leaves = vec![PathBuf::from(".context/crates/x/src/lib.rs.ctx")];
    assert_eq!(
        compute_stale(&sources, &leaves),
        vec![PathBuf::from("crates/x/src/foo.rs")]
    );
}

#[test]
fn leaves_outside_dot_context_dont_satisfy_the_match() {
    // A `.ctx` file at the wrong path doesn't count as a hit — the
    // leaf MUST live under `.context/`.
    let sources = vec![PathBuf::from("crates/x/src/lib.rs")];
    let leaves = vec![PathBuf::from("crates/x/src/lib.rs.ctx")];
    assert_eq!(
        compute_stale(&sources, &leaves),
        vec![PathBuf::from("crates/x/src/lib.rs")]
    );
}
