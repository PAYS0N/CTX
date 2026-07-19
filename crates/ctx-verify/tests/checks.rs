//! Behavioral tests: pure parsers against canned tool output, and
//! orchestration over a fake [`Runner`] (skipped tools, capping, overall
//! status). Hermetic — no real process is ever spawned. The
//! regen-then-recheck path for `contracts`/`architecture` is covered
//! separately in `checks_regen.rs`, which needs a `FakeRunner` that can
//! return different outcomes across successive calls to the same key.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ctx_verify::checks::run_selected;
use ctx_verify::error::CheckError;
use ctx_verify::model::{CheckReport, Diagnostic, Status};
use ctx_verify::parse::{parse_compiler_json, parse_fail_lines, parse_fmt};
use ctx_verify::runner::{CommandOutcome, Runner};

#[test]
fn compiler_json_extracts_primary_span_and_dedups() {
    let line = r#"{"reason":"compiler-message","message":{"level":"error","code":{"code":"clippy::unwrap_used"},"message":"used unwrap()","spans":[{"is_primary":true,"file_name":"crates/x/src/a.rs","line_start":10,"column_start":5}]}}"#;
    let noise = r#"{"reason":"build-finished","success":false}"#;
    let stdout = format!("{line}\n{noise}\n{line}\n");
    let diags = parse_compiler_json(&stdout);
    assert_eq!(
        diags.len(),
        1,
        "duplicate identical message should collapse"
    );
    let d = diags.first().expect("one diagnostic");
    assert_eq!(d.file, "crates/x/src/a.rs");
    assert_eq!(d.line, 10);
    assert_eq!(d.col, 5);
    assert_eq!(d.lint, "clippy::unwrap_used");
}

#[test]
fn fail_lines_split_path_and_line() {
    let text = "ok line\nFAIL: crates/x.rs:94 function spans 33 lines\nFAIL: bare message\n";
    let diags = parse_fail_lines(text);
    assert_eq!(diags.len(), 2);
    let first = diags.first().expect("first");
    assert_eq!(first.file, "crates/x.rs");
    assert_eq!(first.line, 94);
    let second = diags.get(1).expect("second");
    assert_eq!(second.file, "bare message");
    assert_eq!(second.line, 0);
}

#[test]
fn fmt_diff_lines_are_deduped_per_file() {
    let stdout = "Diff in /p/a.rs:3:\nDiff in /p/a.rs:9:\nDiff in /p/b.rs:1:\n";
    let diags = parse_fmt(stdout);
    assert_eq!(diags.len(), 2);
    assert_eq!(diags.first().expect("a").file, "/p/a.rs");
}

/// Canned responses keyed by the command's first argument. No test in
/// this file needs finer-grained disambiguation than that — the
/// `--check`/`--write` disambiguation for `contracts`/`architecture` only
/// matters in `checks_regen.rs`.
struct FakeRunner {
    /// `Some(outcome)` to return it; `None` to simulate a missing tool.
    by_first_arg: BTreeMap<String, Option<CommandOutcome>>,
}

impl Runner for FakeRunner {
    fn run(
        &self,
        _tool: &str,
        args: &[&str],
        _envs: &[(&str, &str)],
    ) -> Result<CommandOutcome, CheckError> {
        let key = (*args.first().unwrap_or(&"")).to_owned();
        match self.by_first_arg.get(&key) {
            Some(Some(outcome)) => Ok(outcome.clone()),
            Some(None) | None => Err(CheckError::ToolMissing(key)),
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

#[test]
fn missing_tool_is_skipped_not_failed() {
    let runner = FakeRunner {
        by_first_arg: BTreeMap::new(),
    };
    let report = run_selected(&runner, 20, Some(&["fmt".to_owned()]), None);
    let fmt = report.checks.get("fmt").expect("fmt present");
    assert_eq!(fmt.status, Status::Skipped);
    // A run of only skipped checks does not fail overall.
    assert_eq!(report.status, Status::Pass);
}

#[test]
fn diagnostics_are_capped_and_counted() {
    let mut lines = String::new();
    for i in 0..5 {
        writeln!(
            lines,
            r#"{{"reason":"compiler-message","message":{{"level":"error","code":{{"code":"clippy::x"}},"message":"m{i}","spans":[{{"is_primary":true,"file_name":"f{i}.rs","line_start":1,"column_start":1}}]}}}}"#
        )
        .expect("write to String is infallible");
    }
    let mut map = BTreeMap::new();
    map.insert("clippy".to_owned(), Some(outcome(101, &lines, "")));
    let runner = FakeRunner { by_first_arg: map };
    let report = run_selected(&runner, 2, Some(&["clippy".to_owned()]), None);
    let clippy = report.checks.get("clippy").expect("clippy present");
    assert_eq!(clippy.status, Status::Fail);
    assert_eq!(clippy.count, 5);
    assert_eq!(clippy.diagnostics.len(), 2);
    assert_eq!(clippy.truncated, 3);
    assert_eq!(report.status, Status::Fail);
}

#[test]
fn passing_check_serializes_to_status_only() {
    let pass = serde_json::to_string(&CheckReport::build(true, vec![], 20)).expect("ser");
    assert_eq!(pass, r#"{"status":"pass"}"#);
    let skipped = serde_json::to_string(&CheckReport::skipped()).expect("ser");
    assert_eq!(skipped, r#"{"status":"skipped"}"#);
    let d = Diagnostic {
        file: "f.rs".to_owned(),
        line: 1,
        col: 1,
        lint: "x".to_owned(),
        message: "m".to_owned(),
    };
    let fail = serde_json::to_string(&CheckReport::build(false, vec![d], 20)).expect("ser");
    assert!(fail.contains(r#""status":"fail""#));
    assert!(fail.contains(r#""count":1"#));
    assert!(fail.contains(r#""diagnostics""#));
}

#[test]
fn clean_command_passes() {
    let mut map = BTreeMap::new();
    map.insert("fmt".to_owned(), Some(outcome(0, "", "")));
    let runner = FakeRunner { by_first_arg: map };
    let report = run_selected(&runner, 20, Some(&["fmt".to_owned()]), None);
    assert_eq!(report.checks.get("fmt").expect("fmt").status, Status::Pass);
    assert_eq!(report.status, Status::Pass);
}

#[test]
fn machete_unused_dependency_fails_with_parsed_diagnostic() {
    // The `machete` script emits one `FAIL: <path> unused dependency '<dep>'`
    // line per finding; script_parser must turn each into a diagnostic and
    // the check must report Fail.
    let mut map = BTreeMap::new();
    map.insert(
        "scripts/machete_check.sh".to_owned(),
        Some(outcome(
            1,
            "",
            "FAIL: ./crates/x/Cargo.toml unused dependency 'serde'",
        )),
    );
    let runner = FakeRunner { by_first_arg: map };
    let report = run_selected(&runner, 20, Some(&["machete".to_owned()]), None);
    let machete = report.checks.get("machete").expect("machete present");
    assert_eq!(machete.status, Status::Fail);
    assert_eq!(machete.count, 1);
    assert!(machete
        .diagnostics
        .first()
        .expect("diagnostic")
        .message
        .contains("unused dependency"));
}

#[test]
fn machete_clean_run_passes() {
    let mut map = BTreeMap::new();
    map.insert(
        "scripts/machete_check.sh".to_owned(),
        Some(outcome(0, "", "")),
    );
    let runner = FakeRunner { by_first_arg: map };
    let report = run_selected(&runner, 20, Some(&["machete".to_owned()]), None);
    assert_eq!(
        report.checks.get("machete").expect("machete").status,
        Status::Pass
    );
    assert_eq!(report.status, Status::Pass);
}

#[test]
fn command_failure_without_diagnostics_is_fail_but_never_silent() {
    // A compile abort exits non-zero with no parseable compiler-message
    // JSON; it must report Fail AND a synthesized hint (never empty).
    let mut map = BTreeMap::new();
    map.insert(
        "clippy".to_owned(),
        Some(outcome(101, "", "error: aborting due to previous error")),
    );
    let runner = FakeRunner { by_first_arg: map };
    let report = run_selected(&runner, 20, Some(&["clippy".to_owned()]), None);
    let clippy = report.checks.get("clippy").expect("clippy present");
    assert_eq!(clippy.status, Status::Fail);
    assert_eq!(clippy.count, 1);
    assert!(clippy
        .diagnostics
        .first()
        .expect("hint")
        .message
        .contains("aborting"));
    assert_eq!(report.status, Status::Fail);
}

#[test]
fn unrunnable_check_is_errored_not_fail() {
    use ctx_verify::model::CheckReport;
    let e = CheckReport::errored("spawn: permission denied".to_owned());
    assert_eq!(e.status, Status::Errored);
    assert_eq!(
        e.diagnostics.first().expect("d").message,
        "spawn: permission denied"
    );
    let ser = serde_json::to_string(&e).expect("ser");
    assert!(ser.contains(r#""status":"errored""#));
    assert!(ser.contains("permission denied"));
    // Precedence: errored outranks fail in the overall status.
    let mut m = BTreeMap::new();
    m.insert("a".to_owned(), CheckReport::build(false, vec![], 20));
    m.insert("b".to_owned(), CheckReport::errored("x".to_owned()));
    assert_eq!(ctx_verify::model::Report::new(m).status, Status::Errored);
}

#[test]
fn raw_rustc_compile_error_is_parsed() {
    // A hard compile error (not a clippy lint): `code` is null, so the
    // lint field is empty but file/line/message are still extracted.
    let line = r#"{"reason":"compiler-message","message":{"level":"error","code":null,"message":"cannot find value `x` in this scope","spans":[{"is_primary":true,"file_name":"crates/m/src/lib.rs","line_start":7,"column_start":13}]}}"#;
    let diags = parse_compiler_json(line);
    assert_eq!(diags.len(), 1);
    let d = diags.first().expect("diagnostic");
    assert_eq!(d.file, "crates/m/src/lib.rs");
    assert_eq!(d.line, 7);
    assert_eq!(d.lint, "");
    assert!(d.message.contains("cannot find value"));
}
