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
    let all = store
        .browse("", 100, Listed::Active, BrowseOrder::default())
        .unwrap();
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
        store
            .browse("brazil", 100, Listed::Active, BrowseOrder::default())
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        store
            .browse("SÉRIES", 100, Listed::Active, BrowseOrder::default())
            .unwrap()
            .total,
        1,
        "case ignored"
    );
    assert_eq!(
        store
            .browse("cccccccc", 100, Listed::Active, BrowseOrder::default())
            .unwrap()
            .rows[0]
            .path,
        paths[2]
    );
    assert_eq!(
        store
            .browse("dddddddd", 100, Listed::Active, BrowseOrder::default())
            .unwrap()
            .total,
        0
    );
    // `%` and `_` are characters to look for, not wildcards.
    assert_eq!(
        store
            .browse("%", 100, Listed::Active, BrowseOrder::default())
            .unwrap()
            .total,
        0
    );
    let limited = store
        .browse("films", 1, Listed::Active, BrowseOrder::default())
        .unwrap();
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
    let row = &store
        .browse("alien", 10, Listed::Active, BrowseOrder::default())
        .unwrap()
        .rows[0];
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

/// The file names of a listing, in its order.
fn names(store: &Store, column: BrowseColumn, descending: bool, listed: Listed) -> Vec<String> {
    let order = BrowseOrder { column, descending };
    let page = store.browse("", 100, listed, order).unwrap();
    page.rows
        .iter()
        .map(|row| row.path.rsplit('\\').next().unwrap().to_owned())
        .collect()
}

#[test]
fn references_are_listed_in_the_order_asked() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::create(&dir.path().join("order.fileseal"), "Order").unwrap();
    // Recorded on the 1st, 2nd, 3rd; Beta never verified, zeta before alpha.
    let rows = [
        (r"E:\A\alpha.mkv", 10, '1', Some("2026-02-05")),
        (r"E:\C\Beta.mkv", 20, '2', None),
        (r"E:\B\zeta.mkv", 30, '3', Some("2026-02-01")),
    ];
    let operation = store.begin_operation(OperationKind::Verify).unwrap();
    for (path, size, day, checked) in rows {
        let item = store
            .insert_reference(&location(path), size, &[sha(day)])
            .unwrap()
            .item_id;
        store
            .conn
            .execute(
                "UPDATE reference SET created_at = ?2 WHERE item_id = ?1",
                params![item, format!("2026-01-0{day}T10:00:00.000Z")],
            )
            .unwrap();
        if let Some(date) = checked {
            store
                .record_verification(&NewVerification {
                    operation_id: operation,
                    item_id: Some(item),
                    reference_id: None,
                    observed_path: path,
                    observed_size: Some(size),
                    observed_mtime: None,
                    started_at: "2026-02-01T10:00:00.000Z",
                    outcome: "match",
                    detail: None,
                    digests: &[sha(day)],
                })
                .unwrap();
            store
                .conn
                .execute(
                    "UPDATE verification SET finished_at = ?2 WHERE item_id = ?1",
                    params![item, format!("{date}T10:00:00.000Z")],
                )
                .unwrap();
        }
    }
    let active = Listed::Active;
    use BrowseColumn::*;
    // By default, by path: by folder, then by name.
    assert_eq!(
        names(&store, Folder, false, active),
        ["alpha.mkv", "zeta.mkv", "Beta.mkv"]
    );
    assert_eq!(
        names(&store, Folder, true, active),
        ["Beta.mkv", "zeta.mkv", "alpha.mkv"]
    );
    assert_eq!(
        names(&store, Name, false, active),
        ["alpha.mkv", "Beta.mkv", "zeta.mkv"],
        "case ignored"
    );
    assert_eq!(
        names(&store, Size, true, active),
        ["zeta.mkv", "Beta.mkv", "alpha.mkv"]
    );
    assert_eq!(
        names(&store, Recorded, false, active),
        ["alpha.mkv", "Beta.mkv", "zeta.mkv"]
    );
    assert_eq!(
        names(&store, LastCheck, false, active),
        ["Beta.mkv", "zeta.mkv", "alpha.mkv"],
        "never verified first"
    );
    assert_eq!(
        names(&store, LastCheck, true, active),
        ["alpha.mkv", "zeta.mkv", "Beta.mkv"]
    );
    // The first of the whole database, not of the rows a page would hold.
    let largest = BrowseOrder {
        column: Size,
        descending: true,
    };
    let page = store.browse("", 1, active, largest).unwrap();
    assert_eq!((page.total, page.rows[0].size), (3, 30));

    // Removed ones: "recorded" is when they were removed.
    let ids: Vec<i64> = store
        .browse("", 10, active, BrowseOrder::default())
        .unwrap()
        .rows
        .iter()
        .map(|row| row.item_id)
        .collect();
    for (id, day) in ids.iter().zip(['3', '1', '2']) {
        store.retire_items(&[*id]).unwrap();
        store
            .conn
            .execute(
                "UPDATE item SET retired_at = ?2 WHERE id = ?1",
                params![id, format!("2026-03-0{day}T10:00:00.000Z")],
            )
            .unwrap();
    }
    assert_eq!(
        names(&store, Recorded, false, Listed::Retired),
        ["zeta.mkv", "Beta.mkv", "alpha.mkv"]
    );
}

/// How long listing a large database takes in each order, the list the
/// Databases screen shows (2 000 rows). Measured, not a limit to hold.
#[test]
#[ignore = "long: cargo test --release -p fileseal-core --lib -- --ignored --nocapture large"]
fn listing_a_large_database_in_every_order() {
    const ITEMS: i64 = 100_000;
    const CHECKS: i64 = 5;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.fileseal");
    let mut store = Store::create(&path, "Large").unwrap();
    let filled = std::time::Instant::now();
    {
        let tx = store.conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO operation (id, kind, started_at, app_version, status) \
             VALUES (1, 'verify', '2026-01-01T00:00:00.000Z', 'test', 'completed')",
            [],
        )
        .unwrap();
        for id in 1..=ITEMS {
            let (series, season, episode) = (id / 100, id / 20 % 5 + 1, id % 20 + 1);
            let name = format!("Series.{series}.S0{season}E{episode:02}.1080p.BluRay.x264.mkv");
            let file = format!(r"D:\Séries\Series {series}\Season {season}\{name}");
            let day = |n: i64| format!("2026-{:02}-{:02}T10:00:00.000Z", n % 12 + 1, n % 28 + 1);
            tx.execute(
                "INSERT INTO item (id, uuid, created_at) VALUES (?1, ?2, ?3)",
                params![id, format!("uuid-{id}"), day(id)],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO reference (id, item_id, size, created_at, origin) \
                 VALUES (?1, ?1, ?2, ?3, 'created')",
                params![id, id * 7919 % 1_000_003, day(id)],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO reference_hash (reference_id, algorithm, hex) VALUES (?1, 'sha256', ?2)",
                params![id, format!("{id:064x}")],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO location (item_id, path, path_key, file_name, first_seen_at, last_seen_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![id, file, platform::path_key(&file), name, day(id)],
            )
            .unwrap();
            // One file in ten never verified.
            if id % 10 != 0 {
                for check in 0..CHECKS {
                    let at = day(id + check * 3);
                    tx.execute(
                        "INSERT INTO verification (operation_id, item_id, reference_id, observed_path, \
                         observed_size, started_at, finished_at, outcome) \
                         VALUES (1, ?1, ?1, ?2, 1, ?3, ?3, 'match')",
                        params![id, file, at],
                    )
                    .unwrap();
                }
            }
        }
        tx.commit().unwrap();
    }
    println!(
        "{ITEMS} references, {CHECKS} verifications each: filled in {:?}",
        filled.elapsed()
    );
    for column in [
        BrowseColumn::Folder,
        BrowseColumn::Name,
        BrowseColumn::Size,
        BrowseColumn::Recorded,
        BrowseColumn::LastCheck,
    ] {
        for descending in [false, true] {
            let order = BrowseOrder { column, descending };
            let started = std::time::Instant::now();
            let page = store.browse("", 2_000, Listed::Active, order).unwrap();
            let all = started.elapsed();
            let started = std::time::Instant::now();
            let searched = store
                .browse("season 3", 2_000, Listed::Active, order)
                .unwrap();
            let search = started.elapsed();
            assert_eq!((page.total, page.rows.len()), (ITEMS as u64, 2_000));
            println!(
                "{column:?} {}: all {all:?}, search \"season 3\" ({} found) {search:?}",
                if descending { "desc" } else { "asc" },
                searched.total
            );
        }
    }
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
