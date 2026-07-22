//! Tests for [`super::probe`], [`super::snippet`], and
//! [`super::claude_invocation`] — the verify/context preflight injected
//! into every `claude` launch. `probe` is exercised as real shell (not
//! string matching) since its whole job is shell behavior: capturing
//! combined stdout+stderr and failing open on a non-zero exit. `snippet`
//! and `claude_invocation` hardcode the `/cage/bin/...` absolute paths
//! only present inside a real cage, so those are checked structurally.

use std::process::Command;

/// Run `script` via a real `sh -c` and return (stdout, exit code).
fn run_sh(script: &str) -> (String, i32) {
    let out = Command::new("sh")
        .arg("-c")
        .arg(script)
        .output()
        .expect("spawn sh");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn probe_reports_label_and_captured_output_on_success() {
    let script = super::probe("demo", "printf 'hello world'");
    let (stdout, code) = run_sh(&script);
    assert_eq!(code, 0);
    assert!(stdout.contains("=== demo ==="), "got: {stdout}");
    assert!(stdout.contains("hello world"), "got: {stdout}");
    assert!(
        !stdout.contains("NOTE:"),
        "a clean exit must not add a NOTE: {stdout}"
    );
}

#[test]
fn probe_notes_a_nonzero_exit_but_stays_fail_open() {
    let script = super::probe("demo", "sh -c 'echo boom 1>&2; exit 7'");
    let (stdout, code) = run_sh(&script);
    assert_eq!(
        code, 0,
        "a failing probe must not fail the enclosing script (fail-open)"
    );
    assert!(stdout.contains("=== demo ==="), "got: {stdout}");
    assert!(stdout.contains("boom"), "stderr must be captured: {stdout}");
    assert!(stdout.contains("NOTE: demo exited 7"), "got: {stdout}");
}

#[test]
fn snippet_orders_verify_before_context_and_exports_preflight() {
    let s = super::snippet();
    let verify_pos = s.find("ctx-verify").expect("mentions ctx-verify");
    let context_pos = s.find("ctx-context .").expect("mentions ctx-context .");
    assert!(
        verify_pos < context_pos,
        "verify must be captured before context: {s}"
    );
    assert!(
        s.contains("PREFLIGHT=$("),
        "combined output must be assigned to $PREFLIGHT: {s}"
    );
    assert!(
        s.contains("printf '%s\\n' \"$PREFLIGHT\""),
        "must echo $PREFLIGHT to stdout too, for the operator: {s}"
    );
    assert!(
        s.trim_end()
            .ends_with("printf '\\n%s\\n' \"$PREFLIGHT\" >> \"$SYSTEM_PROMPT_FILE\""),
        "must fold $PREFLIGHT into $SYSTEM_PROMPT_FILE: {s}"
    );
}

#[test]
fn claude_invocation_headless_carries_preflight_and_task_brief() {
    let inv = super::claude_invocation(true);
    assert!(
        inv.contains("--append-system-prompt-file \"$SYSTEM_PROMPT_FILE\""),
        "got: {inv}"
    );
    assert!(
        !inv.contains("--append-system-prompt \""),
        "claude rejects --append-system-prompt and --append-system-prompt-file together: {inv}"
    );
    assert!(inv.contains("\"$CTX_TASK_BRIEF\""), "got: {inv}");
}

#[test]
fn claude_invocation_interactive_carries_preflight_not_task_brief() {
    let inv = super::claude_invocation(false);
    assert!(inv.contains("setsid --ctty --wait"), "got: {inv}");
    assert!(
        inv.contains("--append-system-prompt-file \"$SYSTEM_PROMPT_FILE\""),
        "got: {inv}"
    );
    assert!(
        !inv.contains("--append-system-prompt \""),
        "claude rejects --append-system-prompt and --append-system-prompt-file together: {inv}"
    );
    assert!(!inv.contains("CTX_TASK_BRIEF"), "got: {inv}");
}
