//! Orchestration over the [`Fs`] and [`Claude`] seams: resolve the backlog
//! item, gather a grounded dossier, then plan the brief.
//!
//! The gather stage is a cheap read-only `claude -p` pass; the plan stage
//! is either an interactive human interview (the session writes the brief
//! itself) or a headless adjudication (the runner writes the captured
//! stdout). Both run with their working directory set to the target repo
//! so its context hooks ground every read.

use std::path::Path;

use crate::claude::Claude;
use crate::error::BriefError;
use crate::fs::Fs;
use crate::status_item;

/// Read-only tools the gather pass may use: source inspection plus the
/// target repo's own context-chain probe. No edit or write tools.
const GATHER_TOOLS: [&str; 4] = ["Read", "Grep", "Glob", "Bash(target/debug/ctx-context *)"];

/// A fully resolved brief request, assembled by the CLI layer.
pub struct Config {
    /// The raw request text (joined argv), for slug and free-text fallback.
    pub request: String,
    /// Headless (adjudicate) vs interactive (interview) plan mode.
    pub headless: bool,
    /// Brief path relative to the target repo (handed to the interactive
    /// session, which writes it from that working directory).
    pub out_rel: String,
    /// Brief path relative to the process cwd (used to write/verify it).
    pub out_fs: String,
    /// Model for the gather pass.
    pub gather_model: String,
    /// Model for the plan pass, or `None` for the CLI default.
    pub plan_model: Option<String>,
    /// Directory holding the prompt files (cwd-relative).
    pub prompts_dir: String,
    /// `docs/STATUS.md` path relative to the process cwd.
    pub status_path: String,
}

/// Load one prompt file, mapping any read failure to [`BriefError::PromptMissing`].
fn load_prompt<F: Fs>(fs: &F, cfg: &Config, name: &str) -> Result<String, BriefError> {
    let path = format!("{}/{}", cfg.prompts_dir, name);
    fs.read(&path).map_err(|_| BriefError::PromptMissing(path))
}

/// Assemble the plan stage's user message from the item and dossier,
/// appending the output instruction when an interactive out path is given.
fn plan_user(item: &str, dossier: &str, out_rel: Option<&str>) -> String {
    let tail = out_rel.map_or_else(String::new, |path| {
        format!(
            "\n## OUTPUT\nWrite the finished brief to `{path}` \
             (create parent directories) and write nothing else to disk.\n"
        )
    });
    format!("## TASK ITEM\n{item}\n\n## DOSSIER\n{dossier}\n{tail}")
}

/// Run the gather pass and return the grounded dossier.
fn gather<F: Fs, C: Claude>(
    fs: &F,
    claude: &C,
    cfg: &Config,
    item: &str,
    target: &Path,
) -> Result<String, BriefError> {
    let prompt = load_prompt(fs, cfg, "briefer-gather.md")?;
    let tools: Vec<String> = GATHER_TOOLS.iter().map(|t| (*t).to_owned()).collect();
    claude.print(&prompt, item, Some(&cfg.gather_model), &tools, target)
}

/// Headless plan: adjudicate and capture the brief on stdout, then write it.
fn headless_plan<F: Fs, C: Claude>(
    fs: &F,
    claude: &C,
    cfg: &Config,
    item: &str,
    dossier: &str,
    target: &Path,
) -> Result<(), BriefError> {
    let prompt = load_prompt(fs, cfg, "briefer-plan-headless.md")?;
    let user = plan_user(item, dossier, None);
    let no_tools: [String; 0] = [];
    let brief = claude.print(&prompt, &user, cfg.plan_model.as_deref(), &no_tools, target)?;
    fs.write(&cfg.out_fs, &brief)
}

/// Interactive plan: interview the human; the session writes the brief.
fn interactive_plan<F: Fs, C: Claude>(
    fs: &F,
    claude: &C,
    cfg: &Config,
    item: &str,
    dossier: &str,
    target: &Path,
) -> Result<(), BriefError> {
    let prompt = load_prompt(fs, cfg, "briefer-plan.md")?;
    let user = plan_user(item, dossier, Some(&cfg.out_rel));
    claude.interactive(&prompt, &user, cfg.plan_model.as_deref(), target)?;
    if fs.exists(&cfg.out_fs) {
        Ok(())
    } else {
        Err(BriefError::BriefNotWritten(cfg.out_fs.clone()))
    }
}

/// Resolve the item, gather a dossier, plan the brief, and return the
/// (cwd-relative) path the brief was written to.
///
/// # Errors
///
/// Propagates ambiguity, prompt, claude, filesystem, and not-written
/// failures.
pub fn run<F: Fs, C: Claude>(
    fs: &F,
    claude: &C,
    cfg: &Config,
    target: &Path,
) -> Result<String, BriefError> {
    let status = fs.read(&cfg.status_path).unwrap_or_default();
    let item = status_item::resolve(&status, &cfg.request)?;
    let dossier = gather(fs, claude, cfg, &item, target)?;
    if cfg.headless {
        headless_plan(fs, claude, cfg, &item, &dossier, target)?;
    } else {
        interactive_plan(fs, claude, cfg, &item, &dossier, target)?;
    }
    Ok(cfg.out_fs.clone())
}
