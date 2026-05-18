//! Check orchestration: run each wrapped tool through the [`Runner`],
//! parse, cap, and assemble one [`Report`].

use std::collections::BTreeMap;

use crate::error::CheckError;
use crate::model::{CheckReport, Diagnostic, Report};
use crate::parse;
use crate::runner::{CommandOutcome, Runner};

/// A pure tool-output parser.
type Parser = fn(&CommandOutcome) -> Vec<Diagnostic>;

/// Parse clippy/rustdoc JSON stdout.
fn compiler_parser(o: &CommandOutcome) -> Vec<Diagnostic> {
    parse::parse_compiler_json(&o.stdout)
}

/// Parse `cargo fmt --check` stdout.
fn fmt_parser(o: &CommandOutcome) -> Vec<Diagnostic> {
    parse::parse_fmt(&o.stdout)
}

/// Parse `FAIL:` lines from a script's combined output.
fn script_parser(o: &CommandOutcome) -> Vec<Diagnostic> {
    let mut combined = o.stdout.clone();
    combined.push('\n');
    combined.push_str(&o.stderr);
    parse::parse_fail_lines(&combined)
}

/// Static description of one check.
struct Spec {
    /// Stable key in the report map.
    name: &'static str,
    /// Executable to run.
    tool: &'static str,
    /// Arguments.
    args: &'static [&'static str],
    /// Extra environment.
    envs: &'static [(&'static str, &'static str)],
    /// Output parser.
    parser: Parser,
}

/// The default check set, in no particular order (the map sorts keys).
const SPECS: &[Spec] = &[
    Spec {
        name: "clippy",
        tool: "cargo",
        args: &[
            "clippy",
            "--quiet",
            "--all-targets",
            "--all-features",
            "--message-format=json",
            "--",
            "-D",
            "warnings",
        ],
        envs: &[("RUSTFLAGS", "-D warnings")],
        parser: compiler_parser,
    },
    Spec {
        name: "doc",
        tool: "cargo",
        args: &[
            "doc",
            "--no-deps",
            "--workspace",
            "--quiet",
            "--message-format=json",
        ],
        envs: &[("RUSTDOCFLAGS", "-D warnings")],
        parser: compiler_parser,
    },
    Spec {
        name: "fmt",
        tool: "cargo",
        args: &["fmt", "--all", "--", "--check"],
        envs: &[],
        parser: fmt_parser,
    },
    Spec {
        name: "rationale",
        tool: "python3",
        args: &["scripts/rationale_check.py", "."],
        envs: &[],
        parser: script_parser,
    },
    Spec {
        name: "workspace_lints",
        tool: "bash",
        args: &["scripts/workspace_lints_check.sh", "."],
        envs: &[],
        parser: script_parser,
    },
    Spec {
        name: "no_allow",
        tool: "bash",
        args: &["scripts/no_allow_check.sh", "."],
        envs: &[],
        parser: script_parser,
    },
];

/// Run one spec into a [`CheckReport`], capping diagnostics to `max`.
fn run_spec<R: Runner>(runner: &R, spec: &Spec, max: usize) -> CheckReport {
    match runner.run(spec.tool, spec.args, spec.envs) {
        Err(CheckError::ToolMissing(_)) => CheckReport::skipped(),
        Err(_) => CheckReport::failed_bare(),
        Ok(outcome) => {
            let diags = (spec.parser)(&outcome);
            CheckReport::build(outcome.ok(), diags, max)
        },
    }
}

/// Whether `spec` is wanted given an optional name filter.
fn wanted(spec: &Spec, only: Option<&[String]>) -> bool {
    only.is_none_or(|names| names.iter().any(|n| n == spec.name))
}

/// Run the selected checks (all when `only` is `None`) into a [`Report`].
#[must_use]
pub fn run_selected<R: Runner>(runner: &R, max: usize, only: Option<&[String]>) -> Report {
    let mut checks = BTreeMap::new();
    for spec in SPECS {
        if wanted(spec, only) {
            checks.insert(spec.name.to_owned(), run_spec(runner, spec, max));
        }
    }
    Report::new(checks)
}
