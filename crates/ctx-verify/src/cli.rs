//! Argv parsing and report rendering — the thin `cli` layer.
//!
//! Two output modes over the same [`Report`]: the default terse render
//! (a single `pass` line, or one `FAIL:`/`ERROR:` block per failing
//! check with truncated diagnostics) and, behind `--json`, the full
//! machine contract via `serde_json` for ctx-run/CI.

use std::io::Write;

use clap::Parser;

use crate::checks;
use crate::error::CheckError;
use crate::model::{CheckReport, Diagnostic, Report, Status};
use crate::runner::Runner;

/// Terse diagnostic messages are clipped to this many chars.
const MAX_MSG: usize = 100;

/// The agent checkpoint: formats, builds, lints, and tests; emits one
/// capped report (`pass` when all-pass).
#[derive(Debug, Parser)]
#[command(
    name = "ctx-verify",
    about = "Format, build, lint, and test; one capped report"
)]
pub struct Cli {
    /// Optional crate name to scope the cargo checks (`-p`); empty =
    /// whole workspace. Module-level scoping is not a cargo concept.
    package: Option<String>,
    /// Maximum diagnostics retained per check (rest counted as truncated).
    #[arg(long, default_value_t = 20)]
    max_diagnostics: usize,
    /// Tight-loop override: restrict to these check names (comma-
    /// separated). Empty = the full default gate.
    #[arg(long, value_delimiter = ',')]
    checks: Vec<String>,
    /// Emit the full JSON report (the machine contract for ctx-run/CI)
    /// instead of the default terse render.
    #[arg(long)]
    json: bool,
}

/// Run the selected checks and write the report to `out`.
///
/// Returns `true` when the run passed (no non-skipped check failed).
///
/// # Errors
///
/// [`CheckError::Encode`] if the report cannot be serialized;
/// [`CheckError::Write`] if it cannot be written to `out`.
pub fn run<R: Runner, W: Write>(runner: &R, cli: &Cli, out: &mut W) -> Result<bool, CheckError> {
    let only = if cli.checks.is_empty() {
        None
    } else {
        Some(cli.checks.as_slice())
    };
    let report = checks::run_selected(runner, cli.max_diagnostics, only, cli.package.as_deref());
    if cli.json {
        serde_json::to_writer_pretty(&mut *out, &report)
            .map_err(|e| CheckError::Encode(e.to_string()))?;
        writeln!(out).map_err(|e| CheckError::Write(e.to_string()))?;
    } else {
        render_terse(&report, out)?;
    }
    Ok(report.status == Status::Pass)
}

/// Write the terse render: a single pass line, or one block per failing
/// check. Pass/skipped checks contribute nothing.
fn render_terse<W: Write>(report: &Report, out: &mut W) -> Result<(), CheckError> {
    if report.status == Status::Pass {
        return line(out, "pass");
    }
    for (name, check) in &report.checks {
        if matches!(check.status, Status::Pass | Status::Skipped) {
            continue;
        }
        render_check(name, check, out)?;
    }
    Ok(())
}

/// Write one `FAIL:`/`ERROR:` header plus its aligned, clipped
/// diagnostics and a `… +N more` tail when the list was capped.
fn render_check<W: Write>(name: &str, check: &CheckReport, out: &mut W) -> Result<(), CheckError> {
    let label = if check.status == Status::Errored {
        "ERROR"
    } else {
        "FAIL"
    };
    line(out, &format!("{label}: {name} ({})", check.count))?;
    let locs: Vec<String> = check.diagnostics.iter().map(locator).collect();
    let width = locs.iter().map(String::len).max().unwrap_or(0);
    for (loc, d) in locs.iter().zip(&check.diagnostics) {
        line(out, &diag_line(loc, width, d))?;
    }
    if check.truncated > 0 {
        line(out, &format!("  … +{} more", check.truncated))?;
    }
    Ok(())
}

/// Render one diagnostic body: indented, locator padded to `width`, then
/// `lint: message` (or bare message when the lint is empty).
fn diag_line(loc: &str, width: usize, d: &Diagnostic) -> String {
    let msg = clip(&d.message);
    if d.lint.is_empty() {
        format!("  {loc:<width$}  {msg}")
    } else {
        format!("  {loc:<width$}  {}: {msg}", d.lint)
    }
}

/// The `file:line` locator, degrading gracefully when either is absent.
fn locator(d: &Diagnostic) -> String {
    if d.file.is_empty() {
        "-".to_owned()
    } else if d.line > 0 {
        format!("{}:{}", d.file, d.line)
    } else {
        d.file.clone()
    }
}

/// Clip a message to [`MAX_MSG`] chars, appending an ellipsis when cut.
fn clip(msg: &str) -> String {
    if msg.chars().count() <= MAX_MSG {
        return msg.to_owned();
    }
    let mut s: String = msg.chars().take(MAX_MSG).collect();
    s.push('…');
    s
}

/// Write one line, mapping any I/O failure to [`CheckError::Write`].
fn line<W: Write>(out: &mut W, text: &str) -> Result<(), CheckError> {
    writeln!(out, "{text}").map_err(|e| CheckError::Write(e.to_string()))
}
