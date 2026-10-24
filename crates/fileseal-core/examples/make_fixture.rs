//! Writes the frozen test database of the current schema version, used by
//! tests/schema_fixtures.rs to prove that every later FileSeal still opens
//! it. Run once per schema version, then commit the file; never regenerate
//! an existing one.
//!
//! cargo run -p fileseal-core --example make_fixture -- crates/fileseal-core/tests/fixtures

use std::path::PathBuf;

use fileseal_core::store::{
    NewLocation, NewVerification, OperationKind, OperationStatus, SCHEMA_VERSION, Store,
};
use fileseal_core::{Algorithm, Digest};

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("output folder"));
    let path = dir.join(format!("schema-v{SCHEMA_VERSION}.fileseal"));
    assert!(
        !path.exists(),
        "{} exists: fixtures are never regenerated",
        path.display()
    );
    let mut store = Store::create(&path, "Fixture").unwrap();
    let digest = |algorithm, hex: &str| Digest {
        algorithm,
        hex: hex.to_owned(),
    };
    // "abc", known by both algorithms.
    let abc = [
        digest(
            Algorithm::Sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        digest(Algorithm::Md5, "900150983cd24fb0d6963f7d28e17f72"),
    ];
    let film = store
        .insert_reference(&location(r"C:\Fixture\Film.mkv", 3), 3, &abc)
        .unwrap();
    // The empty file, SHA-256 only, with a non-ASCII name.
    store
        .insert_reference(
            &location(r"C:\Fixture\Été.mkv", 0),
            0,
            &[digest(
                Algorithm::Sha256,
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            )],
        )
        .unwrap();
    let operation = store.begin_operation(OperationKind::Verify).unwrap();
    store
        .record_verification(&NewVerification {
            operation_id: operation,
            item_id: Some(film.item_id),
            reference_id: Some(film.reference_id),
            observed_path: r"C:\Fixture\Film.mkv",
            observed_size: Some(3),
            observed_mtime: Some("2026-10-22T09:00:00.000Z".to_owned()),
            started_at: "2026-10-22T09:00:00.000Z",
            outcome: "match",
            detail: None,
            digests: &abc,
        })
        .unwrap();
    store
        .finish_operation(operation, OperationStatus::Completed, None)
        .unwrap();
    drop(store);
    println!("wrote {}", path.display());
}

fn location(path: &str, size: u64) -> NewLocation {
    NewLocation {
        path: path.to_owned(),
        path_key: path.to_lowercase(),
        file_name: path.rsplit('\\').next().unwrap().to_owned(),
        last_size: Some(size),
        last_mtime: Some("2026-10-22T08:00:00.000Z".to_owned()),
        volume_serial: None,
        volume_relative_key: None,
    }
}
