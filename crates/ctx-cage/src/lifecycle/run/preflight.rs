//! Verify/context preflight injected into every `claude` launch: runs
//! `ctx-verify` and `ctx-context .` inside the cage before `exec
//! claude`, captures their combined output into `$PREFLIGHT`, and
//! prints it so the operator sees the same grounding the agent gets via
//! `--append-system-prompt-file`. A non-zero exit is noted inline, never
//! halts the run — serving context fails open (`.context/intent.md`).

use crate::bwrap::{CAGE_BIN, CAGE_RULES_PATH};

/// One probe: `label` names the tool, `cmd` is the absolute path to run.
fn probe(label: &str, cmd: &str) -> String {
    format!(
        "out=$({cmd} 2>&1); rc=$?; \
         printf '=== {label} ===\\n%s\\n' \"$out\"; \
         if [ \"$rc\" -ne 0 ]; then printf 'NOTE: {label} exited %s\\n' \"$rc\"; fi"
    )
}

/// Shell fragment that assembles `$PREFLIGHT` (`ctx-verify` output, then
/// `ctx-context .` output, in that order), echoes it to stdout, and
/// concatenates the cage rules doctrine with `$PREFLIGHT` into
/// `$SYSTEM_PROMPT_FILE`. `claude` rejects `--append-system-prompt` and
/// `--append-system-prompt-file` together, so both must fold into one file.
pub(super) fn snippet() -> String {
    let verify = probe("ctx-verify", &format!("{CAGE_BIN}/ctx-verify"));
    let context = probe("ctx-context .", &format!("{CAGE_BIN}/ctx-context ."));
    format!(
        "PREFLIGHT=$({verify}; printf '\\n'; {context})\n\
         printf '%s\\n' \"$PREFLIGHT\"\n\
         SYSTEM_PROMPT_FILE=$(mktemp)\n\
         cat {CAGE_RULES_PATH} > \"$SYSTEM_PROMPT_FILE\"\n\
         printf '\\n%s\\n' \"$PREFLIGHT\" >> \"$SYSTEM_PROMPT_FILE\""
    )
}

/// The `exec claude ...` invocation for headless (`Mode::Task`, `claude
/// -p`) vs. interactive (`Mode::Interactive`, PTY-driven) launches. Both
/// append the cage rules doctrine and the verify/context preflight, folded
/// together into `$SYSTEM_PROMPT_FILE` by [`snippet`], to the system prompt.
pub(super) fn claude_invocation(headless: bool) -> String {
    if headless {
        "exec claude -p --dangerously-skip-permissions \
             --append-system-prompt-file \"$SYSTEM_PROMPT_FILE\" \"$CTX_TASK_BRIEF\""
            .to_string()
    } else {
        // `setsid --ctty` gives claude a new session whose controlling
        // terminal is its stdin — the private PTY slave (see `pty.rs`).
        // Without it the caged process has no controlling TTY in its own
        // PID namespace and Node's readline busy-spins at 100% CPU.
        // `--wait` propagates claude's exit status back through setsid.
        "exec setsid --ctty --wait claude --dangerously-skip-permissions \
             --append-system-prompt-file \"$SYSTEM_PROMPT_FILE\""
            .to_string()
    }
}

#[cfg(test)]
mod tests;
