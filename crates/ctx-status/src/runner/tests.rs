//! Runner tests: migration seeding, additive appends, priority-order
//! rendering, and re-syncing the view from a hand-edited store.

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::{add_task, list, migrate, render, Paths};
use crate::error::StatusError;
use crate::fs::Fs;

/// In-memory `Fs` fake: a shared map of path -> contents.
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

const SOURCE_MD: &str = "\
# Status

Old header text.

| task | description | impact | difficulty |
|---|---|---|---|
| low task | d1 | low | hard |
| high task | d2 | high | easy |
";

#[test]
fn migrate_seeds_the_store_and_renders_the_view() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    let count = migrate(&fs, &paths, "docs/STATUS.md")?;
    assert_eq!(count, 2);
    let rows = list(&fs, &paths)?;
    // priority order: high/easy before low/hard.
    assert_eq!(rows.first().map(|r| r.task.as_str()), Some("high task"));
    let rendered = fs.read(&paths.status_md)?;
    assert!(rendered.contains("docs/status.json"));
    Ok(())
}

#[test]
fn migrate_refuses_to_run_against_a_non_empty_store() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    migrate(&fs, &paths, "docs/STATUS.md")?;
    let err = migrate(&fs, &paths, "docs/STATUS.md").expect_err("must refuse re-migration");
    assert!(matches!(err, StatusError::StoreNotEmpty(_)));
    Ok(())
}

#[test]
fn add_task_appends_without_reordering_the_store() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    migrate(&fs, &paths, "docs/STATUS.md")?;
    add_task(&fs, &paths, "new task", "new desc", "medium", "easy")?;
    let stored: super::Store =
        serde_json::from_str(&fs.read(&paths.store)?).map_err(|e| StatusError::StoreCorrupt {
            path: paths.store.clone(),
            detail: e.to_string(),
        })?;
    // store (insertion) order: migration order preserved, new row appended last.
    assert_eq!(
        stored.iter().map(|r| r.task.as_str()).collect::<Vec<_>>(),
        vec!["low task", "high task", "new task"]
    );
    Ok(())
}

#[test]
fn render_resyncs_the_view_after_a_hand_edit_to_the_store_without_touching_it(
) -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    migrate(&fs, &paths, "docs/STATUS.md")?;
    // Simulate an operator hand-edit to the JSON store (D3 authority).
    fs.write(
        &paths.store,
        r#"[{"task":"renamed","description":"d1","impact":"low","difficulty":"hard"}]"#,
    )?;
    render(&fs, &paths)?;
    let rendered = fs.read(&paths.status_md)?;
    assert!(rendered.contains("| renamed | d1 | low | hard |"));
    let stored: super::Store =
        serde_json::from_str(&fs.read(&paths.store)?).map_err(|e| StatusError::StoreCorrupt {
            path: paths.store.clone(),
            detail: e.to_string(),
        })?;
    // render must not mutate the store it read.
    assert_eq!(stored.len(), 1);
    assert_eq!(stored.first().map(|r| r.task.as_str()), Some("renamed"));
    Ok(())
}

#[test]
fn add_task_rejects_an_unknown_impact() {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD).expect("seed write");
    migrate(&fs, &paths, "docs/STATUS.md").expect("migrate");
    let err = add_task(&fs, &paths, "t", "d", "urgent", "easy").expect_err("bad impact");
    assert!(matches!(err, StatusError::BadImpact(_)));
}
