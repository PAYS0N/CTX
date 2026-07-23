//! Runner tests: migration seeding, additive appends, priority-order
//! rendering, id-based deletion, and re-syncing the view from a
//! hand-edited store.

use std::cell::RefCell;
use std::collections::BTreeMap;

use super::{add_task, delete_task, list, migrate, render, Paths};
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

/// Deserialize the raw JSON store, backfilling nothing (tests inspect
/// exactly what was persisted).
fn stored(fs: &FakeFs, paths: &Paths) -> Result<super::Store, StatusError> {
    serde_json::from_str(&fs.read(&paths.store)?).map_err(|e| StatusError::StoreCorrupt {
        path: paths.store.clone(),
        detail: e.to_string(),
    })
}

#[test]
fn migrate_seeds_the_store_and_renders_the_view() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    let count = migrate(&fs, &paths, "docs/STATUS.md")?;
    assert_eq!(count, 2);
    let tasks = list(&fs, &paths)?;
    // priority order: high/easy before low/hard.
    assert_eq!(
        tasks.first().map(|t| t.row.task.as_str()),
        Some("high task")
    );
    let rendered = fs.read(&paths.status_md)?;
    assert!(rendered.contains("docs/status.json"));
    Ok(())
}

#[test]
fn migrate_assigns_sequential_ids_in_source_order() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    migrate(&fs, &paths, "docs/STATUS.md")?;
    let stored = stored(&fs, &paths)?;
    assert_eq!(
        stored
            .iter()
            .map(|t| (t.id, t.row.task.as_str()))
            .collect::<Vec<_>>(),
        vec![(1, "low task"), (2, "high task")]
    );
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
    let stored = stored(&fs, &paths)?;
    // store (insertion) order: migration order preserved, new row appended last.
    assert_eq!(
        stored
            .iter()
            .map(|t| t.row.task.as_str())
            .collect::<Vec<_>>(),
        vec!["low task", "high task", "new task"]
    );
    Ok(())
}

#[test]
fn add_task_returns_and_assigns_a_fresh_id_past_the_current_max() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    migrate(&fs, &paths, "docs/STATUS.md")?;
    let id = add_task(&fs, &paths, "new task", "new desc", "medium", "easy")?;
    assert_eq!(id, 3);
    let stored = stored(&fs, &paths)?;
    assert_eq!(stored.last().map(|t| t.id), Some(3));
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
        r#"[{"id":1,"task":"renamed","description":"d1","impact":"low","difficulty":"hard"}]"#,
    )?;
    render(&fs, &paths)?;
    let rendered = fs.read(&paths.status_md)?;
    assert!(rendered.contains("| renamed | d1 | low | hard |"));
    let stored = stored(&fs, &paths)?;
    // render must not mutate the store it read.
    assert_eq!(stored.len(), 1);
    assert_eq!(stored.first().map(|t| t.row.task.as_str()), Some("renamed"));
    Ok(())
}

#[test]
fn load_backfills_ids_for_rows_written_before_the_id_field_existed() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write(
        &paths.store,
        r#"[{"task":"legacy a","description":"d","impact":"low","difficulty":"hard"},
            {"task":"legacy b","description":"d","impact":"high","difficulty":"easy"}]"#,
    )?;
    let tasks = list(&fs, &paths)?;
    // priority-sorted: "legacy b" (high/easy) first, but ids follow store
    // (insertion) order regardless of display order.
    let by_task: BTreeMap<&str, u64> = tasks.iter().map(|t| (t.row.task.as_str(), t.id)).collect();
    assert_eq!(by_task.get("legacy a"), Some(&1));
    assert_eq!(by_task.get("legacy b"), Some(&2));
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

#[test]
fn delete_task_removes_the_matching_row_and_keeps_the_rest_in_order() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    migrate(&fs, &paths, "docs/STATUS.md")?;
    // "low task" is id 1 (first migrated row).
    delete_task(&fs, &paths, 1)?;
    let stored = stored(&fs, &paths)?;
    assert_eq!(
        stored
            .iter()
            .map(|t| t.row.task.as_str())
            .collect::<Vec<_>>(),
        vec!["high task"]
    );
    let rendered = fs.read(&paths.status_md)?;
    assert!(!rendered.contains("low task"));
    Ok(())
}

#[test]
fn delete_task_rejects_an_unknown_id() -> Result<(), StatusError> {
    let fs = FakeFs::default();
    let paths = Paths::default();
    fs.write("docs/STATUS.md", SOURCE_MD)?;
    migrate(&fs, &paths, "docs/STATUS.md")?;
    let err = delete_task(&fs, &paths, 999).expect_err("must refuse");
    assert!(matches!(err, StatusError::TaskIdNotFound(999)));
    Ok(())
}
