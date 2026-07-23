//! CLI dispatch tests: argv parsing, id-based add/delete/list output.

use std::cell::RefCell;
use std::collections::BTreeMap;

use clap::Parser;

use super::{dispatch, Cli};
use crate::error::StatusError;
use crate::fs::Fs;

#[derive(Default)]
struct FakeFs {
    files: RefCell<BTreeMap<String, String>>,
}

impl Fs for FakeFs {
    fn read(&self, rel: &str) -> Result<String, StatusError> {
        self.files
            .borrow()
            .get(rel)
            .cloned()
            .ok_or_else(|| StatusError::Io {
                path: rel.to_owned(),
                detail: "missing".to_owned(),
            })
    }
    fn write(&self, rel: &str, contents: &str) -> Result<(), StatusError> {
        self.files
            .borrow_mut()
            .insert(rel.to_owned(), contents.to_owned());
        Ok(())
    }
    fn exists(&self, rel: &str) -> bool {
        self.files.borrow().contains_key(rel)
    }
}

#[test]
fn add_task_requires_an_explicit_task_title() {
    let result = Cli::try_parse_from([
        "ctx-status",
        "add-task",
        "a fresh idea",
        "--impact",
        "medium",
        "--difficulty",
        "easy",
    ]);
    assert!(result.is_err(), "must refuse a missing --task");
}

#[test]
fn add_task_uses_the_given_title() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    fs.write("docs/status.json", "[]")?;
    let cli = Cli::parse_from([
        "ctx-status",
        "add-task",
        "a fresh idea",
        "--task",
        "fresh idea",
        "--impact",
        "medium",
        "--difficulty",
        "easy",
    ]);
    let mut out = Vec::new();
    dispatch(&fs, &cli, &mut out)?;
    assert_eq!(
        String::from_utf8(out).expect("utf8"),
        "added: [1] fresh idea\n"
    );
    assert!(fs.read("docs/status.json")?.contains("fresh idea"));
    Ok(())
}

#[test]
fn delete_task_removes_the_row_with_the_given_id() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    fs.write(
        "docs/status.json",
        r#"[{"id":1,"task":"t","description":"d","impact":"high","difficulty":"easy"}]"#,
    )?;
    let cli = Cli::parse_from(["ctx-status", "delete-task", "1"]);
    let mut out = Vec::new();
    dispatch(&fs, &cli, &mut out)?;
    assert_eq!(String::from_utf8(out).expect("utf8"), "deleted: [1]\n");
    assert!(!fs.read("docs/status.json")?.contains("\"t\""));
    Ok(())
}

#[test]
fn delete_task_rejects_an_unknown_id() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    fs.write(
        "docs/status.json",
        r#"[{"id":1,"task":"t","description":"d","impact":"high","difficulty":"easy"}]"#,
    )?;
    let cli = Cli::parse_from(["ctx-status", "delete-task", "99"]);
    let mut out = Vec::new();
    let err = dispatch(&fs, &cli, &mut out).expect_err("must refuse an unknown id");
    assert!(matches!(err, StatusError::TaskIdNotFound(99)));
    Ok(())
}

#[test]
fn list_prints_one_table_line_per_row_prefixed_with_its_id() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    fs.write(
        "docs/status.json",
        r#"[{"id":1,"task":"t","description":"d","impact":"high","difficulty":"easy"}]"#,
    )?;
    let cli = Cli::parse_from(["ctx-status", "list"]);
    let mut out = Vec::new();
    dispatch(&fs, &cli, &mut out)?;
    assert_eq!(
        String::from_utf8(out).expect("utf8"),
        "| 1 | t | d | high | easy |\n"
    );
    Ok(())
}
