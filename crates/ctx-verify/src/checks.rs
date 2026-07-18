//! Check orchestration: run each wrapped tool through the [`Runner`],
//! parse, cap, and assemble one [`Report`].
//!
//! `fmt` runs first and *applies* `cargo fmt` (formatting is mechanical
//! and authoritative — never an agent judgement); everything after is
//! report-only. The default set is the full agent checkpoint, including
//! `test`. `--checks`/`max` are tight-loop overrides; a package name
//! scopes the cargo-based checks via `-p`.

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

/// Parse `FAIL:` lines from a script's combined output.
fn script_parser(o: &CommandOutcome) -> Vec<Diagnostic> {
    let mut combined = o.stdout.clone();
    combined.push('\n');
    combined.push_str(&o.stderr);
    parse::parse_fail_lines(&combined)
}

/// `fmt` applies formatting; nothing to report unless cargo itself errors
/// (caught by the command's exit status, not by parsed diagnostics).
const fn noop_parser(_o: &CommandOutcome) -> Vec<Diagnostic> {
    Vec::new()
}

/// Surface failing tests / compile errors from `cargo test` output.
fn test_parser(o: &CommandOutcome) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for line in o.stdout.lines().chain(o.stderr.lines()) {
        let t = line.trim();
        if t.ends_with("FAILED") || t.starts_with("error[") || t.starts_with("error:") {
            out.push(Diagnostic {
                file: "test".to_owned(),
                line: 0,
                col: 0,
                lint: "test".to_owned(),
                message: t.to_owned(),
            });
        }
    }
    out
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

/// The default check set. `fmt` is first because it mutates (applies
/// formatting) before the read-only checks compile/inspect.
const SPECS: &[Spec] = &[
    Spec {
        name: "fmt",
        tool: "cargo",
        args: &["fmt", "--all"],
        envs: &[],
        parser: noop_parser,
    },
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
        name: "test",
        tool: "cargo",
        args: &["test", "--quiet", "--workspace"],
        envs: &[],
        parser: test_parser,
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
    Spec {
        name: "machete",
        tool: "bash",
        args: &["scripts/machete_check.sh", "."],
        envs: &[],
        parser: script_parser,
    },
    Spec {
        name: "contracts",
        tool: "bash",
        args: &["scripts/gen_tool_contracts.sh", "--check", "."],
        envs: &[],
        parser: script_parser,
    },
    Spec {
        name: "architecture",
        tool: "bash",
        args: &["scripts/gen_readme_architecture.sh", "--check", "."],
        envs: &[],
        parser: script_parser,
    },
    Spec {
        name: "retired_terms",
        tool: "bash",
        args: &["scripts/retired_terms_check.sh", "."],
        envs: &[],
        parser: script_parser,
    },
];

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

/// Run one spec into a [`CheckReport`], capping diagnostics to `max`.
///
/// A check that cannot be executed is `errored` (distinct from `fail`,
/// carrying the spawn message) so a transient infra failure is never a
/// silent phantom code failure; a check that ran, failed, and produced
/// no parseable diagnostics still gets a synthesized hint, never an
/// empty `fail`.
fn run_spec<R: Runner>(runner: &R, spec: &Spec, max: usize, package: Option<&str>) -> CheckReport {
    let owned = effective_args(spec, package);
    let args: Vec<&str> = owned.iter().map(String::as_str).collect();
    match runner.run(spec.tool, &args, spec.envs) {
        Err(CheckError::ToolMissing(_)) => CheckReport::skipped(),
        Err(e) => CheckReport::errored(e.to_string()),
        Ok(outcome) => {
            let mut diags = (spec.parser)(&outcome);
            if !outcome.ok() && diags.is_empty() {
                diags.push(failure_hint(&outcome));
            }
            CheckReport::build(outcome.ok(), diags, max)
        },
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
