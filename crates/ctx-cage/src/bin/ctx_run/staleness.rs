//! Staleness display for the pre-summarize prompt in `ctx_run.rs`.
//!
//! Mirrors `ctx_scan::render::render_staleness`'s one-line-per-item
//! format without importing it — that renderer is private to
//! `ctx-scan`.

use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;
use std::path::Path;

use ctx_scan::runner::check_run;
use ctx_scan::staleness::Staleness;

/// Format `s` for display before the summarize prompt, one labeled line
/// per item (`fresh` when nothing is stale).
pub fn format_staleness(s: &Staleness) -> String {
    if s.is_fresh() {
        return "fresh\n".to_owned();
    }
    let mut out = String::new();
    for d in &s.stale_dirs {
        let label = if d.is_empty() { "." } else { d };
        let result: std::fmt::Result = writeln!(out, "stale-dir: {label}");
        if result.is_err() {}
    }
    for f in &s.changed_files {
        let result: std::fmt::Result = writeln!(out, "stale-leaf: {f}");
        if result.is_err() {}
    }
    for l in &s.orphan_leaves {
        let result: std::fmt::Result = writeln!(out, "orphan-leaf: {l}");
        if result.is_err() {}
    }
    for m in &s.missing_artifacts {
        let result: std::fmt::Result = writeln!(out, "missing-artifact: {m}");
        if result.is_err() {}
    }
    for o in &s.orphan_artifacts {
        let result: std::fmt::Result = writeln!(out, "orphan-artifact: {o}");
        if result.is_err() {}
    }
    out
}

/// Show what `check_run(dir)` finds before the summarize prompt (all
/// five staleness categories, read-only, no model call), or a loud
/// warning if the check itself fails — the prompt must still proceed
/// either way, so only the write outcome is reported back.
pub fn show_staleness<W: IoWrite>(mut out: W, dir: &Path) -> bool {
    match check_run(dir) {
        Ok(staleness) => write!(out, "{}", format_staleness(&staleness)).is_ok(),
        Err(e) => writeln!(out, "⚠️ could not check staleness: {e}").is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::{check_run, format_staleness, show_staleness, Staleness};

    /// Build a `Staleness` from plain string slices, one per category,
    /// in the same field order `format_staleness` renders them.
    fn staleness_with(
        stale_dirs: &[&str],
        changed_files: &[&str],
        orphan_leaves: &[&str],
        missing_artifacts: &[&str],
        orphan_artifacts: &[&str],
    ) -> Staleness {
        let owned = |xs: &[&str]| xs.iter().map(|s| (*s).to_owned()).collect();
        Staleness {
            stale_dirs: owned(stale_dirs),
            changed_files: owned(changed_files),
            orphan_leaves: owned(orphan_leaves),
            missing_artifacts: owned(missing_artifacts),
            orphan_artifacts: owned(orphan_artifacts),
        }
    }

    #[test]
    fn fresh_tree_renders_fresh() {
        assert_eq!(format_staleness(&Staleness::default()), "fresh\n");
    }

    #[test]
    fn all_five_categories_render_one_line_each() {
        let s = staleness_with(
            &["src"],
            &["src/a.rs"],
            &["src/old.rs.ctx"],
            &["src/rollup.ctx"],
            &["src/gone.rs.ctx"],
        );
        assert_eq!(
            format_staleness(&s),
            "stale-dir: src\n\
             stale-leaf: src/a.rs\n\
             orphan-leaf: src/old.rs.ctx\n\
             missing-artifact: src/rollup.ctx\n\
             orphan-artifact: src/gone.rs.ctx\n"
        );
    }

    #[test]
    fn empty_stale_dir_label_renders_as_dot() {
        let s = staleness_with(&[""], &[], &[], &[], &[]);
        assert_eq!(format_staleness(&s), "stale-dir: .\n");
    }

    #[test]
    fn check_run_failure_shows_warning_but_still_writes() -> Result<(), std::string::FromUtf8Error>
    {
        let missing = std::env::temp_dir().join(format!(
            "ctx-run-confirm-summarize-missing-{}",
            std::process::id()
        ));
        assert!(check_run(&missing).is_err());
        let mut buf: Vec<u8> = Vec::new();
        let wrote = show_staleness(&mut buf, &missing);
        assert!(wrote);
        assert!(String::from_utf8(buf)?.contains("could not check staleness"));
        Ok(())
    }
}
