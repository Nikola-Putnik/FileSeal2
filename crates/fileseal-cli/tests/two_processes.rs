//! A database open in one FileSeal is refused, not shared, by another
//! process: the exclusive lock holds across processes, not only threads.

use std::process::Command;

use fileseal_core::Store;

#[test]
fn a_database_open_elsewhere_is_reported_in_use() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("shared.fileseal");
    let file = dir.path().join("a.bin");
    std::fs::write(&file, b"abc").unwrap();
    let held = Store::create(&db, "Shared").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_fileseal-cli"))
        .arg("verify")
        .arg(&file)
        .arg("--db")
        .arg(&db)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("DatabaseInUse"), "{stderr}");

    drop(held);
    let output = Command::new(env!("CARGO_BIN_EXE_fileseal-cli"))
        .arg("verify")
        .arg(&file)
        .arg("--db")
        .arg(&db)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "opens once released (no reference: not a match)"
    );
}
