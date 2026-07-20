//! Hermetic runner tests over the shared in-memory `Fs` + recording
//! `Claude` fakes in [`common`], exercising the gather → plan sequence, the
//! exact user messages, both plan modes, and the failure paths — no
//! subprocess.

mod common;

use std::path::Path;

use common::{cfg, fake_claude, fake_fs, seeded};
use ctx_brief::error::BriefError;
use ctx_brief::runner;

#[test]
fn headless_writes_brief_and_threads_the_dossier() {
    let world = seeded(true);
    let fs = fake_fs(&world);
    let claude = fake_claude(&world, &["DOSSIER-XYZ", "FINAL-BRIEF"], None);
    let out = runner::run(
        &fs,
        &claude,
        &cfg(true, "stop-hook"),
        Path::new("/tmp/target"),
    )
    .expect("run should succeed");
    assert_eq!(out, ".context/.reports/briefs/item.md");
    assert_eq!(
        world.borrow().files.get(&out).map(String::as_str),
        Some("FINAL-BRIEF")
    );
    assert_headless_calls(&claude).expect("both print calls recorded");
}

/// Assert the two recorded `print` calls of a headless run: a grounded
/// gather pass on the default model with the context probe, then a
/// tool-less plan pass that receives the gather stage's dossier. `None`
/// signals a missing call so the `#[test]` caller can fail on it.
fn assert_headless_calls(claude: &common::FakeClaude) -> Option<()> {
    let gather = claude.nth_print(0)?;
    assert_eq!(gather.system, "GATHER-PROMPT");
    assert!(gather
        .user
        .starts_with("TASK: wire the Stop-hook staleness report"));
    assert_eq!(gather.model.as_deref(), Some("haiku"));
    assert!(gather
        .tools
        .iter()
        .any(|t| t == "Bash(target/debug/ctx-context *)"));
    assert_eq!(gather.cwd, "/tmp/target");
    let plan = claude.nth_print(1)?;
    assert_eq!(plan.system, "HEADLESS-PROMPT");
    assert!(plan.user.contains("DOSSIER-XYZ"));
    assert!(plan.tools.is_empty(), "headless plan runs with no tools");
    Some(())
}

#[test]
fn interactive_succeeds_when_the_session_writes_the_brief() {
    let world = seeded(true);
    let fs = fake_fs(&world);
    let brief = (".context/.reports/briefs/item.md", "HUMAN-BRIEF");
    let claude = fake_claude(&world, &["DOSSIER"], Some(brief));
    let out = runner::run(&fs, &claude, &cfg(false, "stop-hook"), Path::new("/repo"))
        .expect("interactive run should succeed");
    assert_eq!(
        world.borrow().files.get(&out).map(String::as_str),
        Some("HUMAN-BRIEF")
    );
    let seed = claude
        .nth_interactive(0)
        .expect("interactive call recorded");
    assert_eq!(seed.system, "PLAN-PROMPT");
    assert!(seed.user.contains("DOSSIER"));
    assert!(seed.user.contains(".context/.reports/briefs/item.md"));
}

#[test]
fn interactive_errors_when_no_brief_is_written() {
    let world = seeded(true);
    let fs = fake_fs(&world);
    let claude = fake_claude(&world, &["DOSSIER"], None);
    let err = runner::run(&fs, &claude, &cfg(false, "stop-hook"), Path::new("/repo"))
        .expect_err("must fail without a written brief");
    assert!(matches!(err, BriefError::BriefNotWritten(_)));
}

#[test]
fn missing_gather_prompt_errors() {
    let world = seeded(true);
    world.borrow_mut().files.remove("prompts/briefer-gather.md");
    let fs = fake_fs(&world);
    let claude = fake_claude(&world, &[], None);
    let err = runner::run(&fs, &claude, &cfg(true, "stop-hook"), Path::new("/repo"))
        .expect_err("must fail on missing prompt");
    assert!(matches!(err, BriefError::PromptMissing(p) if p == "prompts/briefer-gather.md"));
}

#[test]
fn free_text_request_is_the_item_when_status_is_absent() {
    let world = seeded(false);
    let fs = fake_fs(&world);
    let claude = fake_claude(&world, &["DOSSIER", "BRIEF"], None);
    runner::run(
        &fs,
        &claude,
        &cfg(true, "invent a widget"),
        Path::new("/repo"),
    )
    .expect("run should succeed");
    let gather = claude.nth_print(0).expect("gather call recorded");
    assert_eq!(gather.user, "invent a widget");
}

#[test]
fn no_match_against_a_populated_status_falls_back_to_raw_request() {
    let world = seeded(true);
    let fs = fake_fs(&world);
    let claude = fake_claude(&world, &["DOSSIER", "BRIEF"], None);
    runner::run(
        &fs,
        &claude,
        &cfg(true, "invent a teleporter"),
        Path::new("/repo"),
    )
    .expect("run should succeed even when no backlog row matches");
    let gather = claude.nth_print(0).expect("gather call recorded");
    assert_eq!(gather.user, "invent a teleporter");
}
