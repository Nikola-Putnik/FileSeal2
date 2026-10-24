//! Creating without reading the files already referenced, and recording
//! their hash files in a database (`ExistingReferences`, `import_hash_files`).
//! A read fault is armed where a file must not be read: reading it would
//! fail the creation.

use std::path::{Path, PathBuf};

use super::*;
use crate::faults::{self, FaultPoint};
use crate::store::{OperationKind, SharedStore, Store};

const SHA256: &[Algorithm] = &[Algorithm::Sha256];
const MD5: &[Algorithm] = &[Algorithm::Md5];

struct Fixture {
    dir: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("unread.fileseal"), "Unread").unwrap();
        Fixture { dir, store }
    }

    fn file(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn create(&mut self, path: &Path, options: &CreateOptions, db: bool) -> CreateReport {
        let operation_id = self.store.begin_operation(OperationKind::Create).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = db.then_some(DbTarget {
            store: &shared,
            operation_id,
        });
        create_reference(path, options, target, &Control::new(), &mut |_, _| {})
    }

    /// Created as the step 1 way: read, every destination written.
    fn reference(&mut self, path: &Path, algorithms: &[Algorithm], sidecar: bool, db: bool) {
        let report = self.create(
            path,
            &options(algorithms, sidecar, ExistingReferences::Read),
            db,
        );
        assert_eq!(report.status, CreateStatus::Created);
    }

    /// Same, but the read fails if it happens.
    fn create_unread(&mut self, path: &Path, options: &CreateOptions, db: bool) -> CreateReport {
        faults::arm(FaultPoint::Read);
        let report = self.create(path, options, db);
        faults::clear();
        report
    }

    fn verify_with_database(&mut self, path: &Path) -> VerifyReport {
        let operation_id = self.store.begin_operation(OperationKind::Verify).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = DbTarget {
            store: &shared,
            operation_id,
        };
        verify_file(
            path,
            &VerifyOptions {
                use_sidecar: false,
                follow_moves: false,
            },
            Some(target),
            &Control::new(),
            &mut |_, _| {},
        )
    }
}

fn options(algorithms: &[Algorithm], sidecar: bool, existing: ExistingReferences) -> CreateOptions {
    CreateOptions {
        algorithms: algorithms.to_vec(),
        write_sidecar: sidecar,
        existing,
        ..Default::default()
    }
}

fn importing(
    algorithms: &[Algorithm],
    sidecar: bool,
    existing: ExistingReferences,
) -> CreateOptions {
    CreateOptions {
        import_hash_files: true,
        ..options(algorithms, sidecar, existing)
    }
}

#[test]
fn a_file_with_every_reference_asked_is_not_read() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"a film");
    fx.reference(&film, SHA256, true, true);
    let report = fx.create_unread(
        &film,
        &options(SHA256, true, ExistingReferences::SkipComplete),
        true,
    );
    assert_eq!(report.status, CreateStatus::Skipped, "{report:?}");
    assert_eq!(report.sidecars[0].outcome, WriteOutcome::NotRead);
    assert_eq!(report.database.unwrap().outcome, WriteOutcome::NotRead);
    assert_eq!(report.size, Some(6));
    assert_eq!(report.digests.len(), 1, "what it already has, shown");
    // The same file read again, as before: checked.
    let report = fx.create(
        &film,
        &options(SHA256, true, ExistingReferences::Read),
        true,
    );
    assert_eq!(report.status, CreateStatus::AlreadyReferenced);
}

#[test]
fn a_missing_algorithm_is_read_unless_any_reference_is_enough() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"an old film");
    fx.reference(&film, MD5, true, false);
    // SHA-256 asked, only an .md5: read, the .md5 checked, the .sha256 written.
    let report = fx.create(
        &film,
        &options(SHA256, true, ExistingReferences::SkipComplete),
        false,
    );
    assert_eq!(report.status, CreateStatus::Created);
    let md5 = report
        .sidecars
        .iter()
        .find(|s| s.algorithm == Algorithm::Md5)
        .unwrap();
    assert_eq!(md5.outcome, WriteOutcome::AlreadyPresent);
    std::fs::remove_file(sidecar::path_for(&film, Algorithm::Sha256).unwrap()).unwrap();
    // Any reference enough: the .md5 is, and nothing is added.
    let report = fx.create_unread(
        &film,
        &options(SHA256, true, ExistingReferences::SkipAny),
        false,
    );
    assert_eq!(report.status, CreateStatus::Skipped);
    assert!(
        !sidecar::path_for(&film, Algorithm::Sha256)
            .unwrap()
            .exists()
    );
    let sha = report
        .sidecars
        .iter()
        .find(|s| s.algorithm == Algorithm::Sha256)
        .unwrap();
    assert_eq!((sha.requested, sha.outcome), (true, WriteOutcome::Skipped));
}

#[test]
fn a_file_without_any_reference_is_read_and_created() {
    let mut fx = Fixture::new();
    let film = fx.file("new.mkv", b"never seen");
    let report = fx.create(
        &film,
        &options(SHA256, true, ExistingReferences::SkipAny),
        true,
    );
    assert_eq!(report.status, CreateStatus::Created);
}

#[test]
fn hash_files_are_recorded_in_the_database_without_reading() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"film with its hash files");
    fx.reference(&film, &[Algorithm::Sha256, Algorithm::Md5], true, false);
    let report = fx.create_unread(
        &film,
        &importing(SHA256, true, ExistingReferences::SkipComplete),
        true,
    );
    assert_eq!(report.status, CreateStatus::Imported, "{report:?}");
    let database = report.database.unwrap();
    assert_eq!(database.outcome, WriteOutcome::Imported);
    // Every digest of its hash files, not only the algorithms asked.
    assert_eq!(database.added, [Algorithm::Sha256, Algorithm::Md5]);
    assert_eq!(fx.store.info().unwrap().references, 1);
    let history = fx.store.item_history(report.item_id.unwrap()).unwrap();
    assert_eq!(history.versions[0].origin, "imported");
    // Then verified against the database alone: it matches.
    assert_eq!(fx.verify_with_database(&film).status, VerifyStatus::Match);
}

#[test]
fn an_imported_reference_keeps_what_the_hash_file_said() {
    // The file changed after its hash file was written: importing records
    // the hash file's value, so the next verification finds the change.
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"as it was");
    fx.reference(&film, SHA256, true, false);
    std::fs::write(&film, b"damaged").unwrap();
    let report = fx.create_unread(
        &film,
        &importing(SHA256, false, ExistingReferences::SkipComplete),
        true,
    );
    assert_eq!(report.status, CreateStatus::Imported);
    assert_eq!(
        fx.verify_with_database(&film).status,
        VerifyStatus::Mismatch
    );
}

#[test]
fn without_importing_a_file_unknown_to_the_database_is_read() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"in no database yet");
    fx.reference(&film, SHA256, true, false);
    let report = fx.create(
        &film,
        &options(SHA256, true, ExistingReferences::SkipComplete),
        true,
    );
    assert_eq!(
        report.status,
        CreateStatus::Completed,
        "read, the database written"
    );
    assert_eq!(report.database.unwrap().outcome, WriteOutcome::Written);
}

#[test]
fn references_that_disagree_are_read_and_reported() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"first content");
    fx.reference(&film, SHA256, false, true);
    // A hash file written for another content, by another tool.
    let other = fx.file("other.mkv", b"other content");
    fx.reference(&other, SHA256, true, false);
    let text = sidecar_text(&other).replace("other.mkv", "film.mkv");
    std::fs::write(sidecar::path_for(&film, Algorithm::Sha256).unwrap(), text).unwrap();
    let report = fx.create(
        &film,
        &importing(SHA256, true, ExistingReferences::SkipAny),
        true,
    );
    assert_eq!(report.status, CreateStatus::Conflict);
}

#[test]
fn an_invalid_hash_file_is_read_and_reported() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"content");
    fx.reference(&film, MD5, true, false);
    std::fs::write(
        sidecar::path_for(&film, Algorithm::Sha256).unwrap(),
        "not a hash",
    )
    .unwrap();
    let report = fx.create(
        &film,
        &options(MD5, true, ExistingReferences::SkipAny),
        false,
    );
    assert_eq!(report.status, CreateStatus::Conflict);
}

#[test]
fn a_known_content_that_moved_here_still_needs_a_decision() {
    let mut fx = Fixture::new();
    let first = fx.file("first.mkv", b"moved film");
    fx.reference(&first, SHA256, true, true);
    // Moved by hand with its hash file, under another name.
    let moved = fx.dir.path().join("moved.mkv");
    std::fs::rename(&first, &moved).unwrap();
    let text = sidecar_text(&first).replace("first.mkv", "moved.mkv");
    std::fs::remove_file(sidecar::path_for(&first, Algorithm::Sha256).unwrap()).unwrap();
    std::fs::write(sidecar::path_for(&moved, Algorithm::Sha256).unwrap(), text).unwrap();
    let report = fx.create(
        &moved,
        &importing(SHA256, true, ExistingReferences::SkipComplete),
        true,
    );
    assert_eq!(report.status, CreateStatus::NeedsDecision);
    assert_eq!(fx.store.info().unwrap().references, 1, "nothing imported");
}

fn sidecar_text(path: &Path) -> String {
    std::fs::read_to_string(sidecar::path_for(path, Algorithm::Sha256).unwrap()).unwrap()
}
