//! Check orchestration: run each wrapped tool through the [`Runner`],
//! parse, cap, and assemble one [`Report`].
//!
//! `fmt` runs first and *applies* `cargo fmt` (formatting is mechanical
//! and authoritative — never an agent judgement); everything after is
//! report-only, with one exception: a failing `contracts` or
//! `architecture` check auto-applies the generator's `--write` mode once
//! and re-runs the check, reporting the recheck's outcome rather than the
//! pre-regen one — the same mechanical-mutation precedent as `fmt`
//! (ADR-055). The default set is the full agent checkpoint, including
//! `test`. `--checks`/`max` are tight-loop overrides; a package name
//! scopes the cargo-based checks via `-p`.

use std::collections::BTreeMap;

use crate::error::CheckError;
use crate::model::{CheckReport, Diagnostic, Report};
use crate::runner::{CommandOutcome, Runner};

mod specs;

use specs::{regen_command, Spec, SPECS};

/// Effective args: insert `-p <package>` after the cargo subcommand when
/// a package scopes a cargo-based check. Non-cargo (repo-wide script)
/// checks ignore the package.
fn effective_args(spec: &Spec, package: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = spec.args.iter().map(|s| (*s).to_owned()).collect();
    if let (Some(pkg), "cargo") = (package, spec.tool) {
        if !v.is_empty() {
            v.splice(1..1, ["-p".to_owned(), pkg.to_owned()]);
        }
    }
    v
}

/// Last non-empty line of stderr (else stdout), for a failing check that
/// produced no parseable diagnostics — so the result is never silent.
fn failure_hint(o: &CommandOutcome) -> Diagnostic {
    let tail = o
        .stderr
        .lines()
        .chain(o.stdout.lines())
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("non-zero exit with no output");
    Diagnostic {
        file: "<exec>".to_owned(),
        line: 0,
        col: 0,
        lint: "exit".to_owned(),
        message: tail.to_owned(),
    }
}

/// Assemble a [`CheckReport`] from one command's outcome using `spec`'s
/// parser, synthesizing a failure hint when a failing run parsed no
/// diagnostics.
fn report_from_outcome(spec: &Spec, outcome: &CommandOutcome, max: usize) -> CheckReport {
    let mut diags = (spec.parser)(outcome);
    if !outcome.ok() && diags.is_empty() {
        diags.push(failure_hint(outcome));
    }
    CheckReport::build(outcome.ok(), diags, max)
}

/// Run one spec into a [`CheckReport`], capping diagnostics to `max`.
///
/// A check that cannot be executed is `errored` (distinct from `fail`,
/// carrying the spawn message) so a transient infra failure is never a
/// silent phantom code failure; a check that ran, failed, and produced
/// no parseable diagnostics still gets a synthesized hint, never an
/// empty `fail`.
///
/// When the first run of `contracts`/`architecture` fails,
/// [`regen_command`] is run once and the original check re-run once (no
/// retry loop); the returned report always reflects the recheck's
/// [`CommandOutcome`] — what's on disk after the regen attempt — except
/// when the regen command itself could not be executed, in which case
/// there is nothing fresher to report and the original outcome stands.
fn run_spec<R: Runner>(runner: &R, spec: &Spec, max: usize, package: Option<&str>) -> CheckReport {
    let owned = effective_args(spec, package);
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    let outcome = match runner.run(spec.tool, &args, spec.envs) {
        Err(CheckError::ToolMissing(tool)) => return CheckReport::skipped(&tool),
        Err(e) => return CheckReport::errored(e.to_string()),
        Ok(outcome) => outcome,
    };
    if outcome.ok() {
        return report_from_outcome(spec, &outcome, max);
    }
    let Some((regen_tool, regen_args)) = regen_command(spec.name) else {
        return report_from_outcome(spec, &outcome, max);
    };
    if runner.run(regen_tool, regen_args, &[]).is_err() {
        return report_from_outcome(spec, &outcome, max);
    }
    match runner.run(spec.tool, &args, spec.envs) {
        Err(CheckError::ToolMissing(tool)) => CheckReport::skipped(&tool),
        Err(e) => CheckReport::errored(e.to_string()),
        Ok(recheck) => report_from_outcome(spec, &recheck, max),
    }
}

/// Whether `spec` is wanted given an optional name filter.
fn wanted(spec: &Spec, only: Option<&[String]>) -> bool {
    only.is_none_or(|names| names.iter().any(|n| n == spec.name))
}

/// Run the selected checks (all when `only` is `None`) into a [`Report`],
/// scoping cargo checks to `package` when given.
#[must_use]
pub fn run_selected<R: Runner>(
    runner: &R,
    max: usize,
    only: Option<&[String]>,
    package: Option<&str>,
) -> Report {
    let mut checks = BTreeMap::new();
    for spec in SPECS {
        if wanted(spec, only) {
            checks.insert(spec.name.to_owned(), run_spec(runner, spec, max, package));
        }
    }
    Report::new(checks)
}
