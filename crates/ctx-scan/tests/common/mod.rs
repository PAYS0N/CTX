//! Shared in-memory test fixtures for ctx-scan's hermetic integration tests.

use std::cell::RefCell;
use std::collections::BTreeMap;

use ctx_summarize::agent::Agent;
use ctx_summarize::error::SummError;
use ctx_summarize::fs::Fs;

/// In-memory filesystem for hermetic tests.
pub struct FakeFs {
    /// path → contents.
    pub map: RefCell<BTreeMap<String, String>>,
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

/// Seed `m` with the embedded leaf/rollup summarizer prompt contents.
pub fn seed_prompts(m: &mut BTreeMap<String, String>) {
    let rollup = "prompts/summarizer-rollup.md".to_owned();
    m.insert("prompts/summarizer-leaf.md".to_owned(), "LEAF".to_owned());
    m.insert(rollup, "ROLLUP".to_owned());
}

/// Records every `(system, user)` completion call; always succeeds.
pub struct RecordingAgent {
    /// Recorded `(system, user)` pairs in call order.
    pub calls: RefCell<Vec<(String, String)>>,
}

impl Agent for RecordingAgent {
    fn complete(&self, system: &str, user: &str, _model: &str) -> Result<String, SummError> {
        self.calls
            .borrow_mut()
            .push((system.to_owned(), user.to_owned()));
        Ok(format!("SUMMARY[{system}]"))
    }
}

/// Build a fresh `RecordingAgent`.
pub const fn recording() -> RecordingAgent {
    RecordingAgent {
        calls: RefCell::new(Vec::new()),
    }
}

/// Absolute path to the workspace prompt files, independent of the test
/// process cwd (prompts are loaded from cwd in production, but tests pass
/// an absolute path so `StdFs` resolves it regardless of where cargo runs).
pub fn prompts_path() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../prompts")
        .to_string_lossy()
        .into_owned()
}
