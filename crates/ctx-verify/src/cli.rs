//! Argv parsing and JSON rendering — the thin `cli` layer.

use std::io::Write;

use clap::Parser;

use crate::checks;
use crate::error::CheckError;
use crate::model::Status;
use crate::runner::Runner;

/// The agent checkpoint: formats, builds, lints, and tests; emits one
/// capped JSON report (`{"status":"pass"}` when all-pass).
#[derive(Debug, Parser)]
#[command(
    name = "ctx-verify",
    about = "Format, build, lint, and test; one capped JSON report"
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
}

/// Run the selected checks and write the JSON report to `out`.
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
    serde_json::to_writer_pretty(&mut *out, &report)
        .map_err(|e| CheckError::Encode(e.to_string()))?;
    writeln!(out).map_err(|e| CheckError::Write(e.to_string()))?;
    Ok(report.status == Status::Pass)
}
