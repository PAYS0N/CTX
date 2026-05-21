//! Crate auto-discovery tests. Hermetic: each test creates its own
//! tempdir scaffold and cleans up. No `common` helpers needed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ctx_cage::bwrap::discover_crate_dirs;
use ctx_cage::error::CageError;

/// Counter for unique tempdir names across tests.
static SEQ: AtomicU32 = AtomicU32::new(0);

/// A fresh tempdir; the test must clean it up.
fn fresh_tempdir(label: &str) -> Result<PathBuf, CageError> {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!(
        "ctx-cage-disco-{}-{}-{}",
        label,
        std::process::id(),
        n
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir(&p)?;
    Ok(p)
}

/// Scaffold one Cargo crate under `root/crates/<name>`.
fn scaffold_crate(root: &Path, name: &str, with_tests: bool) -> std::io::Result<()> {
    fs::create_dir_all(root.join("crates").join(name).join("src"))?;
    if with_tests {
        fs::create_dir_all(root.join("crates").join(name).join("tests"))?;
    }
    Ok(())
}

#[test]
fn finds_src_and_tests_under_crates() -> Result<(), CageError> {
    let root = fresh_tempdir("found")?;
    scaffold_crate(&root, "alpha", true).expect("alpha");
    scaffold_crate(&root, "beta", false).expect("beta");
    let found = discover_crate_dirs(&root)?;
    assert_eq!(
        found.srcs,
        vec![
            PathBuf::from("crates/alpha/src"),
            PathBuf::from("crates/beta/src"),
        ]
    );
    assert_eq!(found.tests, vec![PathBuf::from("crates/alpha/tests")]);
    fs::remove_dir_all(&root).expect("rm");
    Ok(())
}

#[test]
fn fails_clearly_when_crates_dir_is_missing() -> Result<(), CageError> {
    let root = fresh_tempdir("none")?;
    let err = discover_crate_dirs(&root).expect_err("must fail");
    let msg = err.to_string();
    assert!(msg.contains("no 'crates' directory"), "got: {msg}");
    fs::remove_dir_all(&root).expect("rm");
    Ok(())
}
