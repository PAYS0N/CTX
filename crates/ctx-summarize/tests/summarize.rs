//! Hermetic runner tests over an in-memory `Fs` and a recording `Agent`,
//! plus real-subprocess `Agent` tests via `sh` (no network, deterministic).

use std::cell::RefCell;
use std::collections::BTreeMap;

use ctx_summarize::agent::{Agent, SubprocessAgent};
use ctx_summarize::error::SummError;
use ctx_summarize::fs::Fs;
use ctx_summarize::runner;

/// In-memory filesystem.
struct FakeFs {
    /// path -> contents.
    map: RefCell<BTreeMap<String, String>>,
}

impl FakeFs {
    fn seeded() -> Self {
        let mut m = BTreeMap::new();
        m.insert(
            "prompts/summarizer-leaf.md".to_owned(),
            "LEAF-PROMPT".to_owned(),
        );
        m.insert(
            "prompts/summarizer-rollup.md".to_owned(),
            "ROLLUP-PROMPT".to_owned(),
        );
        m.insert("crates/foo/bar.rs".to_owned(), "fn bar() {}".to_owned());
        m.insert(
            ".context/crates/foo/intent.md".to_owned(),
            "INTENT-FOO".to_owned(),
        );
        Self {
            map: RefCell::new(m),
        }
    }
}

impl Fs for FakeFs {
    fn read(&self, rel: &str) -> Result<String, SummError> {
        self.map
            .borrow()
            .get(rel)
            .cloned()
            .ok_or_else(|| SummError::Io {
                path: rel.to_owned(),
                detail: "missing".to_owned(),
            })
    }
    fn write(&self, rel: &str, contents: &str) -> Result<(), SummError> {
        self.map
            .borrow_mut()
            .insert(rel.to_owned(), contents.to_owned());
        Ok(())
    }
    fn exists(&self, rel: &str) -> bool {
        self.map.borrow().contains_key(rel)
    }
    fn list_dir(&self, rel: &str) -> Result<Vec<String>, SummError> {
        let prefix = format!("{rel}/");
        let mut names: Vec<String> = Vec::new();
        for key in self.map.borrow().keys() {
            if let Some(rest) = key.strip_prefix(&prefix) {
                if let Some(first) = rest.split('/').next() {
                    let name = first.to_owned();
                    if !name.is_empty() && !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }
        names.sort();
        Ok(names)
    }

    fn is_ignored(&self, _rel: &str) -> Result<bool, SummError> {
        Ok(false)
    }

    fn remove(&self, rel: &str) -> Result<(), SummError> {
        self.map.borrow_mut().remove(rel);
        Ok(())
    }
}

/// Records every `(system, user)` and returns a fixed completion.
struct RecordingAgent {
    /// Recorded calls.
    calls: RefCell<Vec<(String, String)>>,
}

impl Agent for RecordingAgent {
    fn complete(&self, system: &str, user: &str) -> Result<String, SummError> {
        self.calls
            .borrow_mut()
            .push((system.to_owned(), user.to_owned()));
        Ok(format!("SUMMARY[{system}]"))
    }
}

// rationale: one linear scenario (run -> assert leaves -> rollup order -> file contents -> agent call inputs); splitting would fragment the single behavior under test.
#[test]
fn leaf_then_rollups_leaf_up_and_intent_never_written() {
    let fs = FakeFs::seeded();
    let agent = RecordingAgent {
        calls: RefCell::new(Vec::new()),
    };
    let summary =
        runner::run(&fs, &agent, "prompts", &["crates/foo/bar.rs".to_owned()]).expect("run");

    assert_eq!(
        summary.leaves_written,
        vec![".context/crates/foo/bar.rs.ctx"]
    );
    assert_eq!(
        summary.rollups_written,
        vec![
            ".context/crates/foo/rollup.ctx",
            ".context/crates/rollup.ctx",
            ".context/rollup.ctx",
        ]
    );

    let map = fs.map.borrow();
    assert_eq!(
        map.get(".context/crates/foo/bar.rs.ctx").expect("leaf"),
        "SUMMARY[LEAF-PROMPT]"
    );
    assert_eq!(
        map.get(".context/rollup.ctx").expect("root rollup"),
        "SUMMARY[ROLLUP-PROMPT]"
    );
    // intent.md is never written by the runner.
    assert_eq!(
        map.get(".context/crates/foo/intent.md").expect("intent"),
        "INTENT-FOO"
    );

    let calls = agent.calls.borrow();
    let leaf_call = calls.first().expect("leaf call");
    assert_eq!(leaf_call.0, "LEAF-PROMPT");
    assert!(leaf_call.1.contains("SOURCE_PATH: crates/foo/bar.rs"));
    assert!(leaf_call.1.contains("fn bar() {}"));
    // The crates/foo rollup input carries the dir intent + the leaf .ctx.
    let foo_rollup = calls.get(1).expect("foo rollup call");
    assert!(foo_rollup.1.contains("INTENT-FOO"));
    assert!(foo_rollup.1.contains("SUMMARY[LEAF-PROMPT]"));
}

#[test]
fn missing_prompt_is_an_error() {
    let fs = FakeFs {
        map: RefCell::new(BTreeMap::new()),
    };
    let agent = RecordingAgent {
        calls: RefCell::new(Vec::new()),
    };
    let err = runner::run(&fs, &agent, "prompts", &["a.rs".to_owned()]).unwrap_err();
    assert!(matches!(err, SummError::MissingPrompt(_)));
}

#[test]
fn path_escape_is_rejected() {
    let fs = FakeFs::seeded();
    let agent = RecordingAgent {
        calls: RefCell::new(Vec::new()),
    };
    let err = runner::run(&fs, &agent, "prompts", &["../escape".to_owned()]).unwrap_err();
    assert!(matches!(err, SummError::PathEscape(_)));
}

#[test]
fn subprocess_agent_round_trips_stdin_to_stdout() {
    // `cat` echoes the JSON request to stdout: proves the real spawn +
    // stdin write + stdout capture path, with no network.
    let agent = SubprocessAgent::new("cat".to_owned());
    let out = agent.complete("SYS", "USERDATA").expect("cat completion");
    assert!(out.contains("USERDATA"));
    assert!(out.contains("SYS"));
}

#[test]
fn subprocess_agent_nonzero_exit_is_error() {
    let agent = SubprocessAgent::new("exit 3".to_owned());
    assert!(matches!(agent.complete("s", "u"), Err(SummError::Agent(_))));
}

#[test]
fn subprocess_agent_empty_output_is_error() {
    let agent = SubprocessAgent::new("true".to_owned());
    assert!(matches!(agent.complete("s", "u"), Err(SummError::Agent(_))));
}

#[test]
fn gate_refuses_a_secret_target() {
    let fs = FakeFs::seeded();
    let agent = RecordingAgent {
        calls: RefCell::new(Vec::new()),
    };
    let err = runner::run(&fs, &agent, "prompts", &["config/.env".to_owned()]).unwrap_err();
    assert!(matches!(err, SummError::AccessDenied { .. }));
}

#[test]
fn scope_check_gates_oversize_runs_unless_approved() {
    assert!(runner::scope_check(runner::MAX_TARGETS, false).is_ok());
    assert!(matches!(
        runner::scope_check(runner::MAX_TARGETS + 1, false),
        Err(SummError::ScopeTooLarge { .. })
    ));
    assert!(runner::scope_check(runner::MAX_TARGETS + 1, true).is_ok());
}
