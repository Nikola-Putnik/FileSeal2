//! Every database a released FileSeal could create must still open in this
//! version, with its data intact. One frozen database per schema version,
//! written by `examples/make_fixture.rs` and never regenerated.

use std::path::Path;

use fileseal_core::Algorithm;
use fileseal_core::store::{SCHEMA_VERSION, Store};

fn open_copy(name: &str) -> (tempfile::TempDir, Store) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join(name);
    std::fs::copy(&fixture, &copy).unwrap();
    let store = Store::open(&copy).unwrap();
    (dir, store)
}

#[test]
fn schema_v1_opens_with_its_data() {
    let (_dir, store) = open_copy("schema-v1.fileseal");
    let info = store.info().unwrap();
    assert_eq!(info.name, "Fixture");
    assert_eq!(info.schema_version, SCHEMA_VERSION, "brought up to date");
    assert_eq!(info.references, 2);
    let film = store
        .current_by_path(r"c:\fixture\film.mkv")
        .unwrap()
        .expect("reference kept");
    assert_eq!(film.size, 3);
    let algorithms: Vec<Algorithm> = film.digests.iter().map(|d| d.algorithm).collect();
    assert_eq!(algorithms, [Algorithm::Sha256, Algorithm::Md5]);
    let (_, outcome, _) = store
        .last_verification(film.item_id)
        .unwrap()
        .expect("history kept");
    assert_eq!(outcome, "match");
    assert!(
        store
            .current_by_path(r"c:\fixture\été.mkv")
            .unwrap()
            .is_some(),
        "non-ASCII names kept"
    );
}
