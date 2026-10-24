//! The Databases screen's operations: name, browsing and history, backup,
//! move, restore. A restored database must be the backup, row for row.

use std::path::{Path, PathBuf};

use super::files::{self, back_up, move_to, restore};
use super::*;

fn location(path: &str) -> NewLocation {
    NewLocation {
        path: path.to_owned(),
        path_key: platform::path_key(path),
        file_name: path.rsplit(['/', '\\']).next().unwrap().to_owned(),
        last_size: Some(1),
        last_mtime: None,
        volume_serial: None,
        volume_relative_key: None,
    }
}

fn sha(hex_char: char) -> Digest {
    Digest {
        algorithm: Algorithm::Sha256,
        hex: hex_char.to_string().repeat(64),
    }
}

fn fresh(dir: &Path, name: &str) -> Store {
    let mut store = Store::create(&dir.join(format!("{name}.fileseal")), name).unwrap();
    for (path, hex) in [
        (r"E:\Films\Alien.mkv", 'a'),
        (r"E:\Films\Brazil.mkv", 'b'),
        (r"E:\Séries\Dark S01E01.mkv", 'c'),
    ] {
        store
            .insert_reference(&location(path), 10, &[sha(hex)])
            .unwrap();
    }
    store
}

/// Every row of every table, in a stable order: two equal dumps are two
/// databases with the same content.
fn dump(store: &Store) -> String {
    let mut out = String::new();
    for table in [
        "meta",
        "item",
        "operation",
        "reference",
        "reference_hash",
        "location",
        "verification",
        "verification_hash",
    ] {
        let mut statement = store
            .conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
            .unwrap();
        let columns = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let values: Vec<String> = (0..columns)
                .map(|i| format!("{:?}", row.get_ref(i).unwrap()))
                .collect();
            out.push_str(&format!("{table}: {}\n", values.join(" | ")));
        }
    }
    out
}

#[test]
fn the_name_shown_can_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("films.fileseal");
    {
        let mut store = Store::create(&path, "Films").unwrap();
        store.rename("  Films 2026 ").unwrap();
        assert_eq!(store.name(), "Films 2026");
        assert_eq!(store.rename(" ").unwrap_err().kind, ProblemKind::Io);
    }
    assert_eq!(Store::open(&path).unwrap().name(), "Films 2026");
}

#[test]
fn references_are_found_by_name_path_or_hash() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = fresh(dir.path(), "Browse");
    let all = store.browse("", 100, Listed::Active).unwrap();
    assert_eq!(all.total, 3);
    let paths: Vec<&str> = all.rows.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            r"E:\Films\Alien.mkv",
            r"E:\Films\Brazil.mkv",
            r"E:\Séries\Dark S01E01.mkv"
        ]
    );
    assert_eq!(all.rows[0].algorithms, [Algorithm::Sha256]);
    assert_eq!(
        store.browse("brazil", 100, Listed::Active).unwrap().total,
        1
    );
    assert_eq!(
        store.browse("SÉRIES", 100, Listed::Active).unwrap().total,
        1,
        "case ignored"
    );
    assert_eq!(
        store.browse("cccccccc", 100, Listed::Active).unwrap().rows[0].path,
        paths[2]
    );
    assert_eq!(
        store.browse("dddddddd", 100, Listed::Active).unwrap().total,
        0
    );
    // `%` and `_` are characters to look for, not wildcards.
    assert_eq!(store.browse("%", 100, Listed::Active).unwrap().total, 0);
    let limited = store.browse("films", 1, Listed::Active).unwrap();
    assert_eq!((limited.total, limited.rows.len()), (2, 1));

    // The last verification shows in the list and in the history.
    let item = all.rows[0].item_id;
    let operation = store.begin_operation(OperationKind::Verify).unwrap();
    store
        .record_verification(&NewVerification {
            operation_id: operation,
            item_id: Some(item),
            reference_id: None,
            observed_path: r"E:\Films\Alien.mkv",
            observed_size: Some(10),
            observed_mtime: None,
            started_at: "2026-10-22T10:00:00.000Z",
            outcome: "match",
            detail: None,
            digests: &[sha('a')],
        })
        .unwrap();
    store
        .relocate(item, &location(r"F:\Films\Alien.mkv"))
        .unwrap();
    let row = &store.browse("alien", 10, Listed::Active).unwrap().rows[0];
    assert_eq!(row.last_outcome.as_deref(), Some("match"));
    assert_eq!(row.path, r"F:\Films\Alien.mkv");
    let history = store.item_history(item).unwrap();
    assert_eq!(history.versions.len(), 1);
    assert_eq!(history.verifications.len(), 1);
    assert_eq!(history.locations.len(), 2);
    assert!(history.locations[0].current);
    assert_eq!(history.locations[1].path, r"E:\Films\Alien.mkv");
    assert!(history.locations[1].missing_since.is_some());
    assert_eq!(
        store.item_history(999).unwrap_err().kind,
        ProblemKind::NotFound
    );
}

#[test]
fn a_backup_is_a_complete_copy_never_half_written() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh(dir.path(), "Films");
    let target = dir.path().join("backups").join("films copy.fileseal");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let report = back_up(&store, &target, false).unwrap();
    assert_eq!(report.counts, store.counts().unwrap());
    assert!(report.bytes > 0);
    let copy = Store::inspect(&target).unwrap();
    assert_eq!((copy.name.as_str(), copy.counts.references), ("Films", 3));
    // An existing file is replaced only when the user said so.
    assert_eq!(
        back_up(&store, &target, false).unwrap_err().kind,
        ProblemKind::AlreadyExists
    );
    back_up(&store, &target, true).unwrap();
    assert_eq!(
        back_up(&store, store.path(), true).unwrap_err().kind,
        ProblemKind::AlreadyExists,
        "never over itself"
    );
    let leftovers: Vec<PathBuf> = std::fs::read_dir(target.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".partial"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn moving_a_database_keeps_everything_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh(dir.path(), "Films");
    let before = dump(&store);
    let source = store.path().to_path_buf();
    let mut slot = Some(store);

    // Onto an existing file: refused, the database still open where it was.
    let existing = dir.path().join("taken.fileseal");
    std::fs::write(&existing, b"someone else's").unwrap();
    assert_eq!(
        move_to(&mut slot, &existing).unwrap_err().kind,
        ProblemKind::AlreadyExists
    );
    assert_eq!(slot.as_ref().unwrap().path(), source);
    assert_eq!(std::fs::read(&existing).unwrap(), b"someone else's");

    let target = dir.path().join("elsewhere").join("Films renamed.fileseal");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    move_to(&mut slot, &target).unwrap();
    let moved = slot.as_ref().unwrap();
    assert_eq!(moved.path(), target);
    assert_eq!(dump(moved), before);
    assert!(!source.exists());
}

#[test]
fn moving_to_another_disk_copies_checks_then_deletes() {
    // The path taken when a rename is impossible (another volume).
    let dir = tempfile::tempdir().unwrap();
    let store = fresh(dir.path(), "Films");
    let counts = store.counts().unwrap();
    let before = dump(&store);
    let source = store.path().to_path_buf();
    drop(store);
    let target = dir.path().join("Films on F.fileseal");
    let copy = files::copy_then_delete(&source, &target, counts).unwrap();
    assert_eq!(dump(&copy), before);
    assert!(!source.exists());
}

#[test]
fn a_restore_brings_the_backup_back_identical_after_a_safety_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = fresh(dir.path(), "Films");
    let backup = dir.path().join("films backup.fileseal");
    back_up(&store, &backup, false).unwrap();
    let at_backup = dump(&store);
    // Work goes on after the backup.
    store
        .insert_reference(&location(r"E:\Films\Cube.mkv"), 10, &[sha('d')])
        .unwrap();
    store.rename("Films (later)").unwrap();
    let later = store.counts().unwrap();
    let path = store.path().to_path_buf();
    let mut slot = Some(store);

    let safety = restore(&mut slot, &backup).unwrap();
    let restored = slot.as_ref().unwrap();
    assert_eq!(restored.path(), path, "same file, backup content");
    assert_eq!(dump(restored), at_backup, "identical to the backup");
    assert_eq!(restored.name(), "Films");
    // What it held before the restore is kept next to it.
    assert!(safety.path.contains(".before-restore-"), "{}", safety.path);
    assert_eq!(
        Store::inspect(Path::new(&safety.path)).unwrap().counts,
        later
    );
}

#[test]
fn a_restore_refuses_what_is_not_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh(dir.path(), "Films");
    let before = dump(&store);
    let path = store.path().to_path_buf();
    let mut slot = Some(store);
    let foreign = dir.path().join("other.sqlite");
    rusqlite::Connection::open(&foreign)
        .unwrap()
        .execute_batch("CREATE TABLE t (x);")
        .unwrap();
    assert_eq!(
        restore(&mut slot, &foreign).unwrap_err().kind,
        ProblemKind::NotAFileSealDatabase
    );
    assert_eq!(
        restore(&mut slot, &path).unwrap_err().kind,
        ProblemKind::AlreadyExists
    );
    assert_eq!(
        restore(&mut slot, &dir.path().join("missing.fileseal"))
            .unwrap_err()
            .kind,
        ProblemKind::NotFound
    );
    assert_eq!(dump(slot.as_ref().unwrap()), before, "left as it was");
}
