//! Behavioral tests for the `contracts`/`architecture` auto-regen-then-
//! recheck path in `run_spec` (ADR-055): a failing first check run
//! triggers the generator's `--write` mode once, then the original check
//! is re-run once. Separated from `checks.rs` because this is the one
//! path where the same argument list must yield *different* canned
//! outcomes across successive calls (the initial failing check vs. its
//! recheck), which needs a call-ordered `FakeRunner`, not the plain
//! one-response-per-key fake `checks.rs` uses. Hermetic — no real process
//! is ever spawned.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

use ctx_verify::checks::run_selected;
use ctx_verify::error::CheckError;
use ctx_verify::model::Status;
use ctx_verify::runner::{CommandOutcome, Runner};

/// Canned responses keyed by the exact argument list a call is made with,
/// each key holding an ordered queue of outcomes consumed one per call —
/// so the same arguments can yield different outcomes across successive
/// calls (an initial failing check vs. its regen-triggered recheck). A
/// `None` entry, or an absent/exhausted key, simulates a missing tool.
struct FakeRunner {
    responses: RefCell<BTreeMap<Vec<String>, VecDeque<Option<CommandOutcome>>>>,
}

impl FakeRunner {
    const fn new() -> Self {
        Self {
            responses: RefCell::new(BTreeMap::new()),
        }
    }

    /// Queue `outcome` (`None` for a missing tool) as the next response to
    /// a call made with exactly `args`.
    #[must_use]
    fn queue(self, args: &[&str], outcome: Option<CommandOutcome>) -> Self {
        let key: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        self.responses
            .borrow_mut()
            .entry(key)
            .or_default()
            .push_back(outcome);
        self
    }
}

impl Runner for FakeRunner {
    fn run(
        &self,
        _tool: &str,
        args: &[&str],
        _envs: &[(&str, &str)],
    ) -> Result<CommandOutcome, CheckError> {
        let key: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let popped = self
            .responses
            .borrow_mut()
            .get_mut(&key)
            .and_then(VecDeque::pop_front);
        match popped {
            Some(Some(outcome)) => Ok(outcome),
            Some(None) | None => Err(CheckError::ToolMissing(key.join(" "))),
        }
    }
}

fn outcome(code: i32, stdout: &str, stderr: &str) -> CommandOutcome {
    CommandOutcome {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

/// Exact `SPECS`/`regen_command` argument lists these tests key
/// `FakeRunner` calls on; keep in sync with `checks/specs.rs`.
const CONTRACTS_CHECK_ARGS: [&str; 3] = ["scripts/gen_tool_contracts.sh", "--check", "."];
const CONTRACTS_WRITE_ARGS: [&str; 3] = ["scripts/gen_tool_contracts.sh", "--write", "."];
const ARCHITECTURE_CHECK_ARGS: [&str; 3] = ["scripts/gen_readme_architecture.sh", "--check", "."];
const ARCHITECTURE_WRITE_ARGS: [&str; 3] = ["scripts/gen_readme_architecture.sh", "--write", "."];

#[test]
fn contracts_failure_triggers_regen_then_recheck_pass() {
    // First `--check` call fails; `--write` runs (mechanical regen); the
    // recheck `--check` call (same args, second turn in the queue) passes.
    // Overall must be Pass with no diagnostics — the regen fixed it.
    let runner = FakeRunner::new()
        .queue(
            &CONTRACTS_CHECK_ARGS,
            Some(outcome(1, "", "FAIL: contract block is stale")),
        )
        .queue(&CONTRACTS_WRITE_ARGS, Some(outcome(0, "", "")))
        .queue(&CONTRACTS_CHECK_ARGS, Some(outcome(0, "", "")));
    let report = run_selected(&runner, 20, Some(&["contracts".to_owned()]), None);
    let contracts = report.checks.get("contracts").expect("contracts present");
    assert_eq!(contracts.status, Status::Pass);
    assert_eq!(contracts.count, 0);
    assert!(contracts.diagnostics.is_empty());
    assert_eq!(report.status, Status::Pass);
}

#[test]
fn architecture_failure_after_regen_still_fails_reports_recheck_diagnostics() {
    // Regen runs, but the recheck still fails with a *different* diagnostic
    // than the pre-regen call. Per D1, the report must reflect the
    // recheck's output, not the original.
    let runner = FakeRunner::new()
        .queue(
            &ARCHITECTURE_CHECK_ARGS,
            Some(outcome(1, "", "FAIL: pre-regen stale block")),
        )
        .queue(&ARCHITECTURE_WRITE_ARGS, Some(outcome(0, "", "")))
        .queue(
            &ARCHITECTURE_CHECK_ARGS,
            Some(outcome(1, "", "FAIL: still stale after regen")),
        );
    let report = run_selected(&runner, 20, Some(&["architecture".to_owned()]), None);
    let architecture = report
        .checks
        .get("architecture")
        .expect("architecture present");
    assert_eq!(architecture.status, Status::Fail);
    assert_eq!(architecture.count, 1);
    assert_recheck_diagnostic(architecture.diagnostics.first().expect("diagnostic"));
    assert_eq!(report.status, Status::Fail);
}

/// The recheck diagnostic must be the recheck's, never the pre-regen one.
fn assert_recheck_diagnostic(d: &ctx_verify::model::Diagnostic) {
    assert!(
        d.message.contains("still stale after regen"),
        "expected recheck diagnostic, got: {}",
        d.message
    );
    assert!(
        !d.message.contains("pre-regen"),
        "must not report the pre-regen diagnostic per D1, got: {}",
        d.message
    );
}

#[test]
fn regen_command_itself_missing_falls_back_to_original_outcome() {
    // The `--write` regen call has no canned response at all (simulating
    // `CheckError::ToolMissing` for the script), so there is nothing
    // fresher to report and the original failing outcome's diagnostic
    // must be what's reported — no recheck call is ever made.
    let runner = FakeRunner::new().queue(
        &CONTRACTS_CHECK_ARGS,
        Some(outcome(1, "", "FAIL: contract block is stale")),
    );
    let report = run_selected(&runner, 20, Some(&["contracts".to_owned()]), None);
    let contracts = report.checks.get("contracts").expect("contracts present");
    assert_eq!(contracts.status, Status::Fail);
    assert_eq!(contracts.count, 1);
    assert!(contracts
        .diagnostics
        .first()
        .expect("diagnostic")
        .message
        .contains("contract block is stale"));
}
