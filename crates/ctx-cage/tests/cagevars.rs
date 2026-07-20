//! CLI-level test for `ctx_cage::lifecycle::load_cagevars`: process
//! env wins over a conflicting `.cagevars` entry, and the file value
//! fills the gap when the process env is unset.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use ctx_cage::lifecycle::load_cagevars;

/// Counter for unique tempdir names across runs of this binary.
static SEQ: AtomicU32 = AtomicU32::new(0);

/// A fresh tempdir containing a `.cagevars` with `contents`.
fn tempdir_with_cagevars(contents: &str) -> std::io::Result<PathBuf> {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("ctx-cage-cagevars-{}-{}", std::process::id(), n));
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(".cagevars"), contents)?;
    Ok(dir)
}

#[test]
fn process_env_wins_and_file_fills_the_gap() {
    let dir = tempdir_with_cagevars("CTX_CAGE_EXTRA_PATH=/from/file\n").expect("tempdir");

    std::env::set_var("CTX_CAGE_EXTRA_PATH", "/from/process");
    load_cagevars(&dir);
    assert_eq!(
        std::env::var("CTX_CAGE_EXTRA_PATH").as_deref(),
        Ok("/from/process"),
        "process env must win over a conflicting .cagevars entry"
    );

    std::env::remove_var("CTX_CAGE_EXTRA_PATH");
    load_cagevars(&dir);
    assert_eq!(
        std::env::var("CTX_CAGE_EXTRA_PATH").as_deref(),
        Ok("/from/file"),
        ".cagevars must fill the gap when the process env is unset"
    );

    std::env::remove_var("CTX_CAGE_EXTRA_PATH");
    let _ = fs::remove_dir_all(&dir);
}
