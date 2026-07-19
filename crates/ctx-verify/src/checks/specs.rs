//! The fixed catalog of agent-checkpoint checks: one [`Spec`] per check
//! (tool invocation + output parser), plus [`regen_command`] naming the
//! two checks (`contracts`, `architecture`) whose failure is mechanically
//! fixable from the source of truth already on disk.

use crate::model::Diagnostic;
use crate::parse;
use crate::runner::CommandOutcome;

/// A pure tool-output parser.
pub type Parser = fn(&CommandOutcome) -> Vec<Diagnostic>;

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
pub struct Spec {
    /// Stable key in the report map.
    pub name: &'static str,
    /// Executable to run.
    pub tool: &'static str,
    /// Arguments.
    pub args: &'static [&'static str],
    /// Extra environment.
    pub envs: &'static [(&'static str, &'static str)],
    /// Output parser.
    pub parser: Parser,
}

/// The default check set. `fmt` is first because it mutates (applies
/// formatting) before the read-only checks compile/inspect.
pub const SPECS: &[Spec] = &[
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

/// The mechanical regen command for a check whose failure is fixable purely
/// from the source of truth already on disk — `contracts` and
/// `architecture` only. `None` for every other check, which stays
/// report-only per ADR-022.
pub fn regen_command(name: &str) -> Option<(&'static str, &'static [&'static str])> {
    match name {
        "contracts" => Some(("bash", &["scripts/gen_tool_contracts.sh", "--write", "."])),
        "architecture" => Some((
            "bash",
            &["scripts/gen_readme_architecture.sh", "--write", "."],
        )),
        _ => None,
    }
}
