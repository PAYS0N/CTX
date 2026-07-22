//! Integration tests for `ctx_scan::walker::walk_dir`'s file-collection
//! and `.ctxignore`/`.gitignore` seeding behavior.

use std::path::PathBuf;

use ctx_scan::walker::walk_dir;

/// Unique tmpdir path for a given test label.
fn test_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ctx-scan-test-{label}"))
}

/// Create the walk-test fixture directory tree under `base`.
fn create_walk_fixture(base: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(base.join("src"))?;
    std::fs::create_dir_all(base.join(".context"))?;
    std::fs::write(base.join("src/main.rs"), "fn main() {}")?;
    std::fs::write(base.join("logo.png"), b"\x89PNG")?;
    std::fs::write(base.join(".context/rollup.ctx"), "old")?;
    Ok(())
}

#[test]
fn walk_collects_files_and_excludes_context_and_binaries() {
    let base = test_dir("walk");
    drop(std::fs::remove_dir_all(&base));
    create_walk_fixture(&base).expect("fixture setup");

    let files = walk_dir(&base).expect("walk");

    assert!(
        files.contains(&"src/main.rs".to_owned()),
        "rs file included"
    );
    assert!(
        !files.iter().any(|f| {
            std::path::Path::new(f)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        }),
        "png excluded"
    );
    assert!(
        !files.iter().any(|f| f.starts_with(".context")),
        ".context excluded"
    );
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn walk_seeds_ctxignore_from_gitignore_once_then_decouples() {
    let base = test_dir("seed");
    drop(std::fs::remove_dir_all(&base));
    std::fs::create_dir_all(base.join("gen")).expect("mkdir gen");
    std::fs::create_dir_all(base.join("src")).expect("mkdir src");
    std::fs::write(base.join("src/main.rs"), "fn main() {}").expect("write src");
    std::fs::write(base.join("gen/out.rs"), "generated").expect("write gen");
    std::fs::write(base.join(".gitignore"), "gen/\n").expect("write gitignore");

    let first = walk_dir(&base).expect("first walk");
    assert!(base.join(".ctxignore").is_file(), "seeded on first contact");
    assert!(
        !first.iter().any(|f| f.starts_with("gen/")),
        "seed inherited gen/"
    );

    // .gitignore is dead after the hand-off: new entries there change nothing.
    std::fs::write(base.join(".gitignore"), "gen/\nsrc/\n").expect("grow gitignore");
    let second = walk_dir(&base).expect("second walk");
    assert!(
        second.contains(&"src/main.rs".to_owned()),
        ".gitignore must not be consulted after seeding"
    );
    drop(std::fs::remove_dir_all(&base));
}

#[test]
fn walk_scope_honors_ctxignore_and_builtin_target_default() {
    let base = test_dir("scope");
    drop(std::fs::remove_dir_all(&base));
    std::fs::create_dir_all(base.join("src")).expect("mkdir src");
    std::fs::create_dir_all(base.join("target/debug")).expect("mkdir target");
    std::fs::create_dir_all(base.join("gen")).expect("mkdir gen");
    std::fs::write(base.join("src/main.rs"), "fn main() {}").expect("write src");
    std::fs::write(base.join("target/debug/junk.rs"), "junk").expect("write target");
    std::fs::write(base.join("gen/out.rs"), "generated").expect("write gen");
    std::fs::write(base.join(".ctxignore"), "gen/\n").expect("write ctxignore");

    let files = walk_dir(&base).expect("walk");

    assert!(files.contains(&"src/main.rs".to_owned()), "src included");
    assert!(
        !files.iter().any(|f| f.starts_with("target/")),
        "target/ excluded by built-in default"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("gen/")),
        "gen/ excluded by .ctxignore"
    );
    drop(std::fs::remove_dir_all(&base));
}
