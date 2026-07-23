//! Argv parsing and the thin dispatch into [`runner`].

use std::io::Write;
use std::path::Path;

use clap::Parser;

use crate::claude::Claude;
use crate::error::BriefError;
use crate::fs::Fs;
use crate::runner::{self, Config};

/// Turn a `docs/STATUS.md` backlog item into an executor brief.
#[derive(Debug, Parser)]
#[command(
    name = "ctx-brief",
    about = "Turn a STATUS.md backlog item into a ctx-cage task brief"
)]
pub struct Cli {
    /// Headless mode: the plan pass adjudicates tactical decisions itself
    /// and escalates doctrinal ones, instead of interviewing a human.
    #[arg(long)]
    headless: bool,
    /// Override the output path (target-relative). Defaults to
    /// `.context/.reports/briefs/<slug>.md`.
    #[arg(long)]
    out: Option<String>,
    /// Model for the cheap read-only gather pass.
    #[arg(long, default_value = "haiku")]
    gather_model: String,
    /// Model for the plan pass (default: the CLI's own default model).
    #[arg(long)]
    plan_model: Option<String>,
    /// Target repository to brief against (its hooks ground the gather).
    #[arg(long, default_value = ".")]
    target: String,
    /// Directory holding the prompt files (cwd-relative).
    #[arg(long, default_value = "prompts")]
    prompts: String,
    /// Look up this stable backlog id directly in `docs/status.json`,
    /// instead of matching `<request>` against `docs/STATUS.md`. Mutually
    /// exclusive with `<request>`.
    #[arg(long, conflicts_with = "request")]
    id: Option<u64>,
    /// The backlog item to brief: matched against the task column, or used
    /// as free text when nothing matches. Required unless `--id` is given.
    #[arg(num_args = 0..)]
    request: Vec<String>,
}

/// Join `rel` under a (cwd-relative) `target` prefix; `.` means the cwd.
fn under(target: &str, rel: &str) -> String {
    let base = target.trim_end_matches('/');
    if base.is_empty() || base == "." {
        rel.to_owned()
    } else {
        format!("{base}/{rel}")
    }
}

/// Derive a filesystem-safe slug from free text (alphanumerics, dashes).
fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-');
    if trimmed.is_empty() {
        "brief".to_owned()
    } else {
        trimmed.to_owned()
    }
}

impl Cli {
    /// The target repository argument (cwd-relative), for the caller to
    /// resolve to an absolute working directory.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Whether `--id` or a non-empty `<request>` was given.
    const fn has_selector(&self) -> bool {
        self.id.is_some() || !self.request.is_empty()
    }

    /// Assemble the resolved [`Config`] from parsed flags.
    fn to_config(&self) -> Config {
        let request = self.request.join(" ");
        let out_rel = self.out.clone().unwrap_or_else(|| {
            let basis = self
                .id
                .map_or_else(|| slug(&request), |id| format!("item-{id}"));
            format!(".context/.reports/briefs/{basis}.md")
        });
        Config {
            out_fs: under(&self.target, &out_rel),
            status_path: under(&self.target, "docs/STATUS.md"),
            status_json_path: under(&self.target, "docs/status.json"),
            request,
            id: self.id,
            headless: self.headless,
            out_rel,
            gather_model: self.gather_model.clone(),
            plan_model: self.plan_model.clone(),
            prompts_dir: self.prompts.clone(),
        }
    }
}

/// Build the [`Config`], run the pipeline, and print the brief path.
///
/// # Errors
///
/// [`BriefError::NoSelector`] if neither `<request>` nor `--id` was given.
/// Otherwise propagates every runner failure; a stdout write failure is
/// reported as [`BriefError::Io`].
pub fn dispatch<F: Fs, C: Claude, W: Write>(
    fs: &F,
    claude: &C,
    cli: &Cli,
    target_abs: &Path,
    out: &mut W,
) -> Result<(), BriefError> {
    if !cli.has_selector() {
        return Err(BriefError::NoSelector);
    }
    let cfg = cli.to_config();
    let path = runner::run(fs, claude, &cfg, target_abs)?;
    writeln!(out, "{path}").map_err(|e| BriefError::Io {
        path: "<stdout>".to_owned(),
        detail: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{slug, under, Cli};

    #[test]
    fn slug_is_kebab_and_trimmed() {
        assert_eq!(
            slug("Wire the Stop-hook staleness report!"),
            "wire-the-stop-hook-staleness-report"
        );
    }

    #[test]
    fn slug_falls_back_when_empty() {
        assert_eq!(slug("!!!"), "brief");
    }

    #[test]
    fn under_dot_is_identity() {
        assert_eq!(under(".", "docs/STATUS.md"), "docs/STATUS.md");
    }

    #[test]
    fn under_prefixes_a_real_target() {
        assert_eq!(
            under("../other/", "docs/STATUS.md"),
            "../other/docs/STATUS.md"
        );
    }

    #[test]
    fn to_config_slugs_the_id_when_out_is_not_given() {
        let cli = Cli::parse_from(["ctx-brief", "--id", "5"]);
        let cfg = cli.to_config();
        assert_eq!(cfg.id, Some(5));
        assert_eq!(cfg.out_rel, ".context/.reports/briefs/item-5.md");
    }

    #[test]
    fn to_config_still_slugs_the_request_text_without_id() {
        let cli = Cli::parse_from(["ctx-brief", "wire", "the", "thing"]);
        let cfg = cli.to_config();
        assert_eq!(cfg.id, None);
        assert_eq!(cfg.out_rel, ".context/.reports/briefs/wire-the-thing.md");
    }

    #[test]
    fn has_selector_is_true_for_id_alone() {
        assert!(Cli::parse_from(["ctx-brief", "--id", "5"]).has_selector());
    }

    #[test]
    fn has_selector_is_true_for_request_alone() {
        assert!(Cli::parse_from(["ctx-brief", "wire", "the", "thing"]).has_selector());
    }

    #[test]
    fn has_selector_is_false_for_neither() {
        assert!(!Cli::parse_from(["ctx-brief"]).has_selector());
    }
}
