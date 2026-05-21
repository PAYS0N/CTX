//! Cargo-workspace shape discovery for the cage's tmpfs overlays.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::error::CageError;

use super::config::DiscoveredCrates;

/// Walk `<target_root>/crates/*` and collect each crate's `src` and
/// `tests` directories. Used so the cage's tmpfs overlays don't have
/// to hardcode any project's layout.
///
/// # Errors
///
/// [`CageError::Io`] if `crates/` cannot be read;
/// [`CageError::Protocol`] if `crates/` is missing.
pub fn discover_crate_dirs(target_root: &Path) -> Result<DiscoveredCrates, CageError> {
    let crates_root = target_root.join("crates");
    if !crates_root.is_dir() {
        return Err(CageError::Protocol(format!(
            "no 'crates' directory under {}",
            target_root.display()
        )));
    }
    let mut found = DiscoveredCrates::default();
    for entry in std::fs::read_dir(&crates_root)? {
        let entry = entry?;
        let crate_dir = entry.path();
        if !crate_dir.is_dir() {
            continue;
        }
        if let Some(name) = crate_dir.file_name().and_then(OsStr::to_str) {
            collect_crate(&crate_dir, name, &mut found);
        }
    }
    found.srcs.sort();
    found.tests.sort();
    Ok(found)
}

/// Add a single crate's `src` / `tests` directories to `found` when
/// they exist on disk.
fn collect_crate(crate_dir: &Path, name: &str, found: &mut DiscoveredCrates) {
    let base = PathBuf::from("crates").join(name);
    if crate_dir.join("src").is_dir() {
        found.srcs.push(base.join("src"));
    }
    if crate_dir.join("tests").is_dir() {
        found.tests.push(base.join("tests"));
    }
}
