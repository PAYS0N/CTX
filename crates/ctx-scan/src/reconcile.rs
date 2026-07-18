//! Mirror reconciliation — the inverse of `record_missing_artifacts`.
//!
//! [`find_orphan_artifacts`] enumerates what actually exists under the
//! `.context/` mirror and flags every derived artifact (leaf `.ctx`,
//! `rollup.ctx`, `hashes.json` sidecar) whose source file or directory
//! no longer exists or is no longer in scope; [`prune`] deletes the
//! flagged files and sweeps mirror directories left empty. Scope is not
//! reimplemented here: the walker's target list is the single source of
//! truth, so the live artifact set is exactly what a scan of those
//! targets would (re)generate. Owner-authored `intent.md` is never
//! flagged, and the runtime dirs `.context/.cache/` and
//! `.context/.reports/` are never entered.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ctx_summarize::cpath;
use ctx_summarize::fs::Fs;

use crate::error::ScanError;
use crate::hash::{depth, join};

/// Mirror artifacts with no live source, plus the mirror subdirectories
/// visited while looking (the empty-dir sweep candidates).
#[derive(Debug, Default)]
pub struct OrphanScan {
    /// `.context/...` file paths whose source is gone or out of scope.
    pub artifacts: Vec<String>,
    /// Source-relative mirror subdirectories visited, any depth.
    visited_dirs: Vec<String>,
}

impl OrphanScan {
    /// The flagged artifacts minus paths already reported elsewhere
    /// (e.g. the hash diff's `orphan_leaves`).
    #[must_use]
    pub fn artifacts_excluding(&self, known: &[String]) -> Vec<String> {
        self.artifacts
            .iter()
            .filter(|a| !known.contains(a))
            .cloned()
            .collect()
    }
}

/// The live source universe derived from the walker's target list:
/// every in-scope file, and every directory the scan writes a rollup
/// and sidecar for (the targets' ancestor dirs, root always included).
struct Live {
    /// In-scope source files, repo-relative.
    files: BTreeSet<String>,
    /// Directories owning a live rollup/sidecar (`""` = root).
    dirs: BTreeSet<String>,
}

impl Live {
    /// Derive the live set from `targets`. The root is always live so
    /// an accidentally emptied scope cannot flag the root artifacts.
    fn from_targets(targets: &[String]) -> Self {
        let mut dirs = BTreeSet::new();
        dirs.insert(String::new());
        for f in targets {
            for d in cpath::ancestor_dirs(f) {
                dirs.insert(d);
            }
        }
        Self {
            files: targets.iter().cloned().collect(),
            dirs,
        }
    }
}

/// Runtime state directories under `.context/` that are never entered.
fn is_runtime(dir: &str, name: &str) -> bool {
    dir.is_empty() && matches!(name, ".cache" | ".reports")
}

/// Classify one mirror entry in source dir `dir`: flag orphaned derived
/// artifacts, recurse into subdirectories, leave everything else alone.
fn classify<F: Fs>(
    fs: &F,
    live: &Live,
    dir: &str,
    name: &str,
    out: &mut OrphanScan,
) -> Result<(), ScanError> {
    let mirror = cpath::context_dir_of(dir);
    if name == "intent.md" {
        return Ok(()); // owner-authored: never flagged
    }
    if name == "rollup.ctx" || name == "hashes.json" {
        if !live.dirs.contains(dir) {
            out.artifacts.push(format!("{mirror}/{name}"));
        }
        return Ok(());
    }
    if let Some(stem) = name.strip_suffix(".ctx") {
        if !live.files.contains(&join(dir, stem)) {
            out.artifacts.push(format!("{mirror}/{name}"));
        }
        return Ok(());
    }
    let child = join(dir, name);
    out.visited_dirs.push(child.clone());
    scan_mirror_dir(fs, live, &child, out)
}

/// Enumerate the mirror of source dir `dir` and classify each entry.
/// A plain file that is not a derived artifact lists as an empty
/// directory and is a no-op.
fn scan_mirror_dir<F: Fs>(
    fs: &F,
    live: &Live,
    dir: &str,
    out: &mut OrphanScan,
) -> Result<(), ScanError> {
    for name in fs.list_dir(&cpath::context_dir_of(dir))? {
        if !is_runtime(dir, &name) {
            classify(fs, live, dir, &name, out)?;
        }
    }
    Ok(())
}

/// Find every derived mirror artifact whose source is absent from the
/// walker's `targets` (deleted, moved, or scoped out). Read-only.
///
/// # Errors
///
/// Propagates mirror listing failures.
pub fn find_orphan_artifacts<F: Fs>(fs: &F, targets: &[String]) -> Result<OrphanScan, ScanError> {
    let live = Live::from_targets(targets);
    let mut out = OrphanScan::default();
    scan_mirror_dir(fs, &live, "", &mut out)?;
    out.artifacts.sort();
    Ok(out)
}

/// Remove `base`-relative file `rel`; a missing file is not an error.
fn remove_file(base: &Path, rel: &str) -> Result<(), ScanError> {
    match fs::remove_file(base.join(rel)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ScanError::Io {
            path: rel.to_owned(),
            detail: e.to_string(),
        }),
    }
}

/// Remove directory `p` iff it exists and is empty.
fn remove_dir_if_empty(p: &Path) -> Result<(), ScanError> {
    let Ok(mut entries) = fs::read_dir(p) else {
        return Ok(()); // absent, or a plain file: nothing to sweep
    };
    if entries.next().is_some() {
        return Ok(());
    }
    match fs::remove_dir(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(ScanError::Io {
            path: p.to_string_lossy().into_owned(),
            detail: e.to_string(),
        }),
    }
}

/// Delete every flagged artifact, then sweep the visited mirror
/// directories deepest-first so emptied subtrees collapse. Pure
/// filesystem work — never calls a model.
///
/// # Errors
///
/// [`ScanError::Io`] if a removal fails for a reason other than absence.
pub fn prune(base: &Path, scan: &OrphanScan) -> Result<(), ScanError> {
    for rel in &scan.artifacts {
        remove_file(base, rel)?;
    }
    let mut dirs = scan.visited_dirs.clone();
    dirs.sort_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));
    for d in &dirs {
        remove_dir_if_empty(&base.join(cpath::context_dir_of(d)))?;
    }
    Ok(())
}
