//! End-to-end tests for `cli::run`'s output modes: the default terse
//! render (a single `pass` line, or `FAIL:` blocks) and the `--json`
//! machine contract, driven over a fake [`Runner`] — no real subprocess
//! is spawned.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;

use clap::Parser as _;
use ctx_verify::cli::{run, Cli};
use ctx_verify::error::CheckError;
use ctx_verify::runner::{CommandOutcome, Runner};

/// Canned responses keyed by the command's first argument.
struct FakeRunner {
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

fn outcome(code: i32, stdout: &str) -> CommandOutcome {
    CommandOutcome {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// Parse argv, drive `cli::run` over the fake runner, return
/// `(passed, stdout)`. Returns `Result` so it may live outside a test
/// body without tripping the unwrap/expect ban.
fn render(
    args: &[&str],
    map: BTreeMap<String, Option<CommandOutcome>>,
) -> Result<(bool, String), Box<dyn Error>> {
    let cli = Cli::try_parse_from(args)?;
    let runner = FakeRunner { by_first_arg: map };
    let mut out: Vec<u8> = Vec::new();
    let passed = run(&runner, &cli, &mut out)?;
    Ok((passed, String::from_utf8(out)?))
}

#[test]
fn terse_pass_is_a_single_status_line() {
    let mut map = BTreeMap::new();
    map.insert("fmt".to_owned(), Some(outcome(0, "")));
    let (passed, text) = render(&["ctx-verify", "--checks", "fmt"], map).expect("render");
    assert!(passed);
    assert_eq!(text, "pass\n");
}

#[test]
fn terse_fail_emits_header_and_truncated_diagnostics() {
    let mut lines = String::new();
    for i in 0..3 {
        writeln!(
            lines,
            r#"{{"reason":"compiler-message","message":{{"level":"error","code":{{"code":"clippy::x"}},"message":"m{i}","spans":[{{"is_primary":true,"file_name":"f{i}.rs","line_start":1,"column_start":1}}]}}}}"#
        )
        .expect("write to String is infallible");
    }
    let mut map = BTreeMap::new();
    map.insert("clippy".to_owned(), Some(outcome(101, &lines)));
    let args = ["ctx-verify", "--checks", "clippy", "--max-diagnostics", "2"];
    let (passed, text) = render(&args, map).expect("render");
    assert!(!passed);
    assert!(text.contains("FAIL: clippy (3)"), "header: {text}");
    assert!(text.contains("f0.rs:1  clippy::x: m0"), "diag line: {text}");
    assert!(text.contains("… +1 more"), "truncation tail: {text}");
    // No JSON body leaks into the terse render.
    assert!(!text.contains("\"status\""), "no json body on fail: {text}");
}

#[test]
fn json_flag_still_emits_the_machine_contract() {
    let mut map = BTreeMap::new();
    map.insert("fmt".to_owned(), Some(outcome(0, "")));
    let (passed, text) = render(&["ctx-verify", "--checks", "fmt", "--json"], map).expect("render");
    assert!(passed);
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    assert_eq!(value.get("status").and_then(|v| v.as_str()), Some("pass"));
}
