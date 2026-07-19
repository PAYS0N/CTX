//! Shared fakes and builders for the ctx-brief integration tests.
//!
//! The fake filesystem and fake CLI share one `World` so an interactive
//! session can "write" a brief the runner then observes.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use ctx_brief::claude::Claude;
use ctx_brief::error::BriefError;
use ctx_brief::fs::Fs;
use ctx_brief::runner::Config;

/// A two-row backlog whose task column the tests match against.
pub const STATUS: &str = "\
# Status\n\
\n\
| task | description | impact | difficulty |\n\
|---|---|---|---|\n\
| wire the Stop-hook staleness report | not wired anywhere | high | easy |\n\
| unrelated other item | noise | low | hard |\n";

/// Shared mutable world backing both fakes.
#[derive(Default)]
pub struct World {
    /// path -> contents.
    pub files: BTreeMap<String, String>,
}

/// A shared handle to the world.
pub type Shared = Rc<RefCell<World>>;

/// In-memory filesystem over the shared world.
pub struct FakeFs {
    /// The shared world.
    pub world: Shared,
}

impl Fs for FakeFs {
    fn read(&self, rel: &str) -> Result<String, BriefError> {
        self.world
            .borrow()
            .files
            .get(rel)
            .cloned()
            .ok_or_else(|| BriefError::Io {
                path: rel.to_owned(),
                detail: "missing".to_owned(),
            })
    }
    fn write(&self, rel: &str, contents: &str) -> Result<(), BriefError> {
        self.world
            .borrow_mut()
            .files
            .insert(rel.to_owned(), contents.to_owned());
        Ok(())
    }
    fn exists(&self, rel: &str) -> bool {
        self.world.borrow().files.contains_key(rel)
    }
}

/// One recorded `print` call.
#[derive(Clone)]
pub struct PrintCall {
    /// Appended system prompt (the prompt file contents).
    pub system: String,
    /// The user message.
    pub user: String,
    /// Selected model, if any.
    pub model: Option<String>,
    /// The permission allowlist.
    pub tools: Vec<String>,
    /// Working directory the child would run in.
    pub cwd: String,
}

/// One recorded `interactive` call.
#[derive(Clone)]
pub struct InteractiveCall {
    /// Appended system prompt.
    pub system: String,
    /// The seed user message.
    pub user: String,
}

/// Recording fake `claude`: returns queued print outputs and optionally
/// creates a file on `interactive` to simulate the human session.
pub struct FakeClaude {
    /// Shared world (so `interactive` can create the brief).
    world: Shared,
    /// Print outputs, consumed front-to-back.
    print_outputs: RefCell<Vec<String>>,
    /// Recorded print calls.
    print_calls: RefCell<Vec<PrintCall>>,
    /// Recorded interactive calls.
    interactive_calls: RefCell<Vec<InteractiveCall>>,
    /// (path, contents) the interactive session "writes", if any.
    interactive_creates: Option<(String, String)>,
}

impl FakeClaude {
    /// The `i`th recorded print call (cloned), or `None` if absent.
    #[must_use]
    pub fn nth_print(&self, i: usize) -> Option<PrintCall> {
        self.print_calls.borrow().get(i).cloned()
    }
    /// The `i`th recorded interactive call (cloned), or `None` if absent.
    #[must_use]
    pub fn nth_interactive(&self, i: usize) -> Option<InteractiveCall> {
        self.interactive_calls.borrow().get(i).cloned()
    }
}

impl Claude for FakeClaude {
    fn print(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
        allowed_tools: &[String],
        cwd: &Path,
    ) -> Result<String, BriefError> {
        self.print_calls.borrow_mut().push(PrintCall {
            system: system.to_owned(),
            user: user.to_owned(),
            model: model.map(str::to_owned),
            tools: allowed_tools.to_vec(),
            cwd: cwd.display().to_string(),
        });
        let mut outs = self.print_outputs.borrow_mut();
        if outs.is_empty() {
            return Err(BriefError::Claude("no queued print output".to_owned()));
        }
        Ok(outs.remove(0))
    }

    fn interactive(
        &self,
        system: &str,
        user: &str,
        _model: Option<&str>,
        _cwd: &Path,
    ) -> Result<(), BriefError> {
        self.interactive_calls.borrow_mut().push(InteractiveCall {
            system: system.to_owned(),
            user: user.to_owned(),
        });
        if let Some((path, contents)) = &self.interactive_creates {
            self.world
                .borrow_mut()
                .files
                .insert(path.clone(), contents.clone());
        }
        Ok(())
    }
}

/// Seed a world with the three prompt files and (optionally) STATUS.md.
#[must_use]
pub fn seeded(with_status: bool) -> Shared {
    let mut files = BTreeMap::new();
    files.insert(
        "prompts/briefer-gather.md".to_owned(),
        "GATHER-PROMPT".to_owned(),
    );
    files.insert(
        "prompts/briefer-plan.md".to_owned(),
        "PLAN-PROMPT".to_owned(),
    );
    files.insert(
        "prompts/briefer-plan-headless.md".to_owned(),
        "HEADLESS-PROMPT".to_owned(),
    );
    if with_status {
        files.insert("docs/STATUS.md".to_owned(), STATUS.to_owned());
    }
    Rc::new(RefCell::new(World { files }))
}

/// A fake filesystem sharing `world`.
#[must_use]
pub fn fake_fs(world: &Shared) -> FakeFs {
    FakeFs {
        world: Rc::clone(world),
    }
}

/// A fake claude with queued print `outputs` and an optional interactive write.
#[must_use]
pub fn fake_claude(world: &Shared, outputs: &[&str], creates: Option<(&str, &str)>) -> FakeClaude {
    FakeClaude {
        world: Rc::clone(world),
        print_outputs: RefCell::new(outputs.iter().map(|s| (*s).to_owned()).collect()),
        print_calls: RefCell::new(Vec::new()),
        interactive_calls: RefCell::new(Vec::new()),
        interactive_creates: creates.map(|(p, c)| (p.to_owned(), c.to_owned())),
    }
}

/// A Config for the given mode and request, writing to a fixed brief path.
#[must_use]
pub fn cfg(headless: bool, request: &str) -> Config {
    Config {
        request: request.to_owned(),
        headless,
        out_rel: ".context/.reports/briefs/item.md".to_owned(),
        out_fs: ".context/.reports/briefs/item.md".to_owned(),
        gather_model: "haiku".to_owned(),
        plan_model: None,
        prompts_dir: "prompts".to_owned(),
        status_path: "docs/STATUS.md".to_owned(),
    }
}
