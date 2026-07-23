//! Orchestration over the [`Fs`] and [`Claude`] seams: resolve the backlog
//! item, gather a grounded dossier, then plan the brief.
//!
//! The gather stage is a cheap read-only `claude -p` pass; the plan stage
//! is either an interactive human interview (the session writes the brief
//! itself) or a headless adjudication (the runner writes the captured
//! stdout). Both run with their working directory set to the target repo
//! so its context hooks ground every read.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use crate::claude::Claude;
use crate::error::BriefError;
use crate::fs::Fs;
use crate::status_item;

/// Read-only tools the gather pass may use: source inspection plus the
/// target repo's own context-chain probe. No edit or write tools.
const GATHER_TOOLS: [&str; 4] = ["Read", "Grep", "Glob", "Bash(target/debug/ctx-context *)"];

/// Append a `<stage>: starting` progress line to stderr, ignoring a
/// broken write channel. Not threaded through a seam: nothing in the
/// test suite inspects stderr, and this stays independent of the
/// `Fs`/`Claude` fakes.
fn stage_start(stage: &str) {
    let result: Result<(), std::io::Error> = writeln!(std::io::stderr(), "{stage}: starting");
    if result.is_err() {}
}

/// Append a `<stage>: done (N.Ns)` progress line to stderr, timed since
/// `started`, ignoring a broken write channel.
fn stage_done(stage: &str, started: Instant) {
    let secs = started.elapsed().as_secs_f64();
    let result: Result<(), std::io::Error> =
        writeln!(std::io::stderr(), "{stage}: done ({secs:.1}s)");
    if result.is_err() {}
}

/// Confirm on `w` that `request` didn't match a STATUS.md row and is being
/// treated as a custom (free-text) item. Generic over the writer purely for
/// unit-testability — the same pattern `cli::dispatch` uses for its own
/// output — and ignores a broken write channel like `stage_start`/`stage_done`.
fn log_custom_item<W: Write>(mut w: W, request: &str) {
    let result: Result<(), std::io::Error> = writeln!(
        w,
        "resolve: no match for '{request}'; treating as custom item"
    );
    if result.is_err() {}
}

/// Confirm on `w` that `status_path` doesn't exist and the request is being
/// treated as free text with no backlog to match against. Same generic-writer
/// pattern as [`log_custom_item`], for the same reason.
fn log_status_not_found<W: Write>(mut w: W, status_path: &str) {
    let result: Result<(), std::io::Error> = writeln!(
        w,
        "resolve: STATUS.md not found at '{status_path}'; treating request as free text"
    );
    if result.is_err() {}
}

/// A fully resolved brief request, assembled by the CLI layer.
pub struct Config {
    /// The raw request text (joined argv), for slug and free-text fallback.
    /// Empty when `id` is `Some` (the two selectors are mutually exclusive).
    pub request: String,
    /// A stable `docs/status.json` id to look up directly, instead of
    /// matching `request` against `docs/STATUS.md`.
    pub id: Option<u64>,
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
    /// `docs/status.json` path relative to the process cwd, read only when
    /// `id` is `Some`.
    pub status_json_path: String,
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
    stage_start("gather");
    let started = Instant::now();
    let dossier = claude.print(&prompt, item, Some(&cfg.gather_model), &tools, target);
    stage_done("gather", started);
    dossier
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
    stage_start("plan");
    let started = Instant::now();
    let brief = claude.print(&prompt, &user, cfg.plan_model.as_deref(), &no_tools, target);
    stage_done("plan", started);
    fs.write(&cfg.out_fs, &brief?)
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
    stage_start("plan");
    claude.interactive(&prompt, &user, cfg.plan_model.as_deref(), target)?;
    if fs.exists(&cfg.out_fs) {
        Ok(())
    } else {
        Err(BriefError::BriefNotWritten(cfg.out_fs.clone()))
    }
}

/// Resolve the TASK ITEM: by `cfg.id` against `docs/status.json` when
/// given, otherwise by matching `cfg.request` against `docs/STATUS.md`
/// (logging to stderr whether it matched or fell back to free text).
fn resolve_item<F: Fs>(fs: &F, cfg: &Config) -> Result<String, BriefError> {
    if let Some(id) = cfg.id {
        let status_json = fs
            .read(&cfg.status_json_path)
            .map_err(|_| BriefError::TaskIdNotFound(id))?;
        return status_item::resolve_id(&status_json, id);
    }
    if !fs.exists(&cfg.status_path) {
        log_status_not_found(std::io::stderr(), &cfg.status_path);
    }
    let status = fs.read(&cfg.status_path).unwrap_or_default();
    let item = status_item::resolve(&status, &cfg.request)?;
    if !status_item::matched(&status, &cfg.request) {
        log_custom_item(std::io::stderr(), &cfg.request);
    }
    Ok(item)
}

/// Resolve the item, gather a dossier, plan the brief, and return the
/// (cwd-relative) path the brief was written to.
///
/// # Errors
///
/// Propagates task-id-lookup, ambiguity, prompt, claude, filesystem, and
/// not-written failures.
pub fn run<F: Fs, C: Claude>(
    fs: &F,
    claude: &C,
    cfg: &Config,
    target: &Path,
) -> Result<String, BriefError> {
    let item = resolve_item(fs, cfg)?;
    let dossier = gather(fs, claude, cfg, &item, target)?;
    if cfg.headless {
        headless_plan(fs, claude, cfg, &item, &dossier, target)?;
    } else {
        interactive_plan(fs, claude, cfg, &item, &dossier, target)?;
    }
    Ok(cfg.out_fs.clone())
}

#[cfg(test)]
mod tests {
    use super::{log_custom_item, log_status_not_found};

    #[test]
    fn log_custom_item_confirms_the_fallback_with_the_request_text() {
        let mut buf: Vec<u8> = Vec::new();
        log_custom_item(&mut buf, "invent a teleporter");
        assert_eq!(
            String::from_utf8(buf).expect("stderr line is valid utf-8"),
            "resolve: no match for 'invent a teleporter'; treating as custom item\n"
        );
    }

    #[test]
    fn log_status_not_found_confirms_the_missing_path() {
        let mut buf: Vec<u8> = Vec::new();
        log_status_not_found(&mut buf, "docs/STATUS.md");
        assert_eq!(
            String::from_utf8(buf).expect("stderr line is valid utf-8"),
            "resolve: STATUS.md not found at 'docs/STATUS.md'; treating request as free text\n"
        );
    }
}
