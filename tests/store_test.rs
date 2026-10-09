use std::fs;
use tempfile::tempdir;
use zfg::core::store::{AddFocusResult, List, Snapshot, Store};

#[test]
fn initialization_creates_missing_files_and_preserves_existing_bytes() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let root = dir.path().join("store");
    Store::new(root.clone(), 2)?;
    assert_eq!(fs::read(root.join("focus.txt"))?, b"");
    assert_eq!(fs::read(root.join("later.txt"))?, b"");
    let existing = "\n café \n\n";
    fs::write(root.join("focus.txt"), existing)?;
    fs::remove_file(root.join("later.txt"))?;
    Store::new(root.clone(), 2)?;
    assert_eq!(fs::read_to_string(root.join("focus.txt"))?, existing);
    assert_eq!(fs::read(root.join("later.txt"))?, b"");
    fs::write(root.join("later.txt"), "existing later\n")?;
    Store::new(root.clone(), 2)?;
    assert_eq!(
        fs::read_to_string(root.join("later.txt"))?,
        "existing later\n"
    );
    let invalid_root = dir.path().join("invalid");
    assert!(Store::new(invalid_root.clone(), 0).is_err());
    assert!(!invalid_root.exists());
    Ok(())
}

#[test]
fn replacement_retains_slot_and_appends_displaced_duplicate() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 3)?;
    for task in ["same", "same", "last"] {
        store.add_focus(task)?;
    }
    store.add_later("waiting")?;
    let before = store.snapshot()?;
    assert!(store.replace_focus(3, "new", &before).is_err());
    assert!(store.replace_focus(1, " \n ", &before).is_err());
    assert_eq!(store.snapshot()?, before);
    store.replace_focus(1, " new\n task ", &before)?;
    assert_eq!(store.snapshot()?.focus, vec!["same", "new task", "last"]);
    assert_eq!(store.snapshot()?.later, vec!["waiting", "same"]);
    assert!(!dir.path().join("state/txn.json").exists());
    assert!(store.replace_focus(1, "again", &before).is_err());
    Ok(())
}

#[test]
fn replace_from_later_is_atomic_and_preserves_every_task() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 1)?;
    store.add_focus("focus item")?;
    store.add_later("incoming")?;
    store.add_later("keep later")?;
    let snapshot = store.snapshot()?;
    store.replace_focus_from_later(0, 0, &snapshot)?;
    let state = store.snapshot()?;
    assert_eq!(state.focus, vec!["incoming"]);
    assert_eq!(state.later, vec!["keep later", "focus item"]);
    assert!(store.replace_focus_from_later(0, 0, &snapshot).is_err());
    Ok(())
}

#[test]
fn replacement_journal_recovers_after_only_focus_was_written() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 2)?;
    store.add_focus("old")?;
    store.add_focus("keep")?;
    store.add_later("waiting")?;
    let target = Snapshot {
        focus: vec!["new".into(), "keep".into()],
        later: vec!["waiting".into(), "old".into()],
    };
    fs::write(
        dir.path().join("state/txn.json"),
        serde_json::to_vec(&target)?,
    )?;
    fs::write(dir.path().join("focus.txt"), "new\nkeep\n")?;
    let reopened = Store::new(dir.path().to_owned(), 2)?;
    assert_eq!(reopened.snapshot()?, target);
    assert!(!dir.path().join("state/txn.json").exists());
    Ok(())
}

#[test]
fn cap_duplicates_and_status() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 2)?;
    assert_eq!(store.status()?.focus_first, "");
    assert_eq!(store.add_focus("same")?, AddFocusResult::Added);
    store.add_focus("same")?;
    assert_eq!(
        store.add_focus("third")?,
        AddFocusResult::Full {
            current: vec!["same".into(), "same".into()]
        }
    );
    let status = store.status()?;
    assert_eq!(status.focus_count, 2);
    assert_eq!(status.focus_limit, 2);
    assert_eq!(status.focus_first, "same");
    assert_eq!(status.later_count, 0);
    assert!(status.focus_full);
    let old = store.snapshot()?;
    store.complete(List::Focus, 0, &old)?;
    assert!(store.complete(List::Focus, 0, &old).is_err());
    assert_eq!(store.snapshot()?.focus, vec!["same"]);
    Ok(())
}

#[test]
fn normalization_unicode_and_utf8_files() -> anyhow::Result<()> {
    let dir = tempdir()?;
    fs::write(dir.path().join("later.txt"), "\n café \n\n日本語\n")?;
    let store = Store::new(dir.path().to_owned(), 3)?;
    assert_eq!(store.snapshot()?.later, vec!["café", "日本語"]);
    store.add_focus("  Привет\r\n мир  ")?;
    assert_eq!(store.snapshot()?.focus, vec!["Привет мир"]);
    assert!(store.add_focus(" \n\t ").is_err());
    store.add_later(&"界".repeat(500))?;
    assert!(store.add_later(&"界".repeat(501)).is_err());
    assert_eq!(store.snapshot()?.later[2].chars().count(), 500);
    Ok(())
}

#[test]
fn mutations_check_both_lists_and_indices() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 3)?;
    store.add_focus("a")?;
    store.add_focus("b")?;
    let old = store.snapshot()?;
    store.add_later("other")?;
    assert!(store.replace_focus(0, "x", &old).is_err());
    assert!(store.edit(List::Later, 0, "x", &old).is_err());
    assert!(store.reorder_focus(0, 1, &old).is_err());
    assert!(store.move_task(List::Focus, 0, &old).is_err());
    assert!(store.complete(List::Focus, 0, &old).is_err());
    let current = store.snapshot()?;
    assert!(store.complete(List::Focus, 2, &current).is_err());
    assert!(store.reorder_focus(0, 2, &current).is_err());
    store.reorder_focus(0, 1, &current)?;
    store.replace_focus(0, "new", &store.snapshot()?)?;
    store.edit(List::Later, 0, "edited", &store.snapshot()?)?;
    assert_eq!(store.snapshot()?.focus, vec!["new", "a"]);
    assert_eq!(store.snapshot()?.later, vec!["edited", "b"]);
    Ok(())
}

#[test]
fn transfers_respect_cap_and_leave_no_journal() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 1)?;
    store.add_focus("a")?;
    store.add_later("b")?;
    let snapshot = store.snapshot()?;
    assert_eq!(
        store.move_task(List::Later, 0, &snapshot)?,
        AddFocusResult::Full {
            current: vec!["a".into()]
        }
    );
    assert_eq!(store.snapshot()?, snapshot);
    assert_eq!(
        store.move_task(List::Focus, 0, &snapshot)?,
        AddFocusResult::Added
    );
    assert_eq!(store.snapshot()?.later, vec!["b", "a"]);
    store.move_task(List::Later, 1, &store.snapshot()?)?;
    assert_eq!(store.snapshot()?.focus, vec!["a"]);
    assert_eq!(store.snapshot()?.later, vec!["b"]);
    assert!(!dir.path().join("state/txn.json").exists());
    Ok(())
}

#[test]
fn journal_replays_partial_transfer_on_every_lock() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 2)?;
    store.add_focus("a")?;
    store.add_later("b")?;
    let target = Snapshot {
        focus: vec![],
        later: vec!["b".into(), "a".into()],
    };
    fs::write(
        dir.path().join("state/txn.json"),
        serde_json::to_vec(&target)?,
    )?;
    fs::write(dir.path().join("focus.txt"), "")?;
    assert_eq!(store.snapshot()?, target);
    assert!(!dir.path().join("state/txn.json").exists());
    // Replaying the same journal again must not duplicate any task.
    fs::write(
        dir.path().join("state/txn.json"),
        serde_json::to_vec(&target)?,
    )?;
    store.add_later("c")?;
    assert_eq!(store.snapshot()?.later, vec!["b", "a", "c"]);
    fs::write(dir.path().join("state/txn.json"), "broken")?;
    assert!(store.snapshot().is_err());
    assert!(dir.path().join("state/txn.json").exists());
    Ok(())
}

#[test]
fn independent_stores_reread_and_serialize_writers() -> anyhow::Result<()> {
    let dir = tempdir()?;
    let store = Store::new(dir.path().to_owned(), 10)?;
    let mut threads = Vec::new();
    for _ in 0..4 {
        let other = Store::new(dir.path().to_owned(), 10)?;
        threads.push(std::thread::spawn(move || -> anyhow::Result<()> {
            for _ in 0..20 {
                other.add_later("duplicate")?;
            }
            Ok(())
        }));
    }
    for thread in threads {
        thread.join().unwrap()?;
    }
    assert_eq!(store.snapshot()?.later.len(), 80);
    assert!(Store::new(dir.path().to_owned(), 0).is_err());
    Ok(())
}
