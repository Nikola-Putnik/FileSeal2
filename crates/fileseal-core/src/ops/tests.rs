//! Scenarios of the plan's step 1: create, verify, protection of existing
//! references, partial success, cancellation.

use std::path::{Path, PathBuf};

use super::*;
use crate::faults::{self, FaultPoint};
use crate::store::{OperationKind, SharedStore, Store};

const SHA256: &[Algorithm] = &[Algorithm::Sha256];

struct Fixture {
    dir: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("test.fileseal"), "Test").unwrap();
        Fixture { dir, store }
    }

    fn file(&self, name: &str, content: &[u8]) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn create(
        &mut self,
        path: &Path,
        algorithms: &[Algorithm],
        sidecar: bool,
        db: bool,
    ) -> CreateReport {
        let operation_id = self.store.begin_operation(OperationKind::Create).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = db.then_some(DbTarget {
            store: &shared,
            operation_id,
        });
        let options = CreateOptions {
            algorithms: algorithms.to_vec(),
            write_sidecar: sidecar,
            new_item: false,
            ..Default::default()
        };
        create_reference(path, &options, target, &Control::new(), &mut |_, _| {})
    }

    fn verify(&mut self, path: &Path, sidecar: bool, db: bool) -> VerifyReport {
        let operation_id = self.store.begin_operation(OperationKind::Verify).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = db.then_some(DbTarget {
            store: &shared,
            operation_id,
        });
        verify_file(
            path,
            &VerifyOptions {
                use_sidecar: sidecar,
            },
            target,
            &Control::new(),
            &mut |_, _| {},
        )
    }

    fn references(&self) -> u64 {
        self.store.info().unwrap().references
    }
}

fn sidecar_text(path: &Path, algorithm: Algorithm) -> Option<String> {
    std::fs::read_to_string(sidecar::path_for(path, algorithm).unwrap()).ok()
}

#[test]
fn create_then_verify_with_both_destinations() {
    let mut fx = Fixture::new();
    let film = fx.file("Blade Runner.mkv", b"abc");
    let created = fx.create(&film, SHA256, true, true);
    assert_eq!(created.status, CreateStatus::Created, "{created:#?}");
    assert_eq!(created.sidecars[0].outcome, WriteOutcome::Written);
    assert_eq!(
        created.database.as_ref().unwrap().outcome,
        WriteOutcome::Written
    );
    assert_eq!(
        sidecar_text(&film, Algorithm::Sha256).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  Blade Runner.mkv\n"
    );
    assert_eq!(fx.references(), 1);

    let verified = fx.verify(&film, true, true);
    assert_eq!(verified.status, VerifyStatus::Match, "{verified:#?}");
    assert_eq!(verified.sources.len(), 2, "hash file and database");
    assert!(
        verified
            .sources
            .iter()
            .all(|s| s.outcome == SourceOutcome::Match)
    );
    assert_eq!(
        fx.store.verification_count().unwrap(),
        1,
        "recorded in history"
    );
}

#[test]
fn source_files_are_never_modified() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"precious content");
    let before = (
        std::fs::read(&film).unwrap(),
        std::fs::metadata(&film).unwrap().modified().unwrap(),
    );
    fx.create(&film, &[Algorithm::Sha256, Algorithm::Md5], true, true);
    fx.verify(&film, true, true);
    let after = (
        std::fs::read(&film).unwrap(),
        std::fs::metadata(&film).unwrap().modified().unwrap(),
    );
    assert_eq!(before, after);
}

#[test]
fn creating_again_keeps_identical_references() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    fx.create(&film, SHA256, true, true);
    let sidecar = sidecar::path_for(&film, Algorithm::Sha256).unwrap();
    let written_at = std::fs::metadata(&sidecar).unwrap().modified().unwrap();
    let again = fx.create(&film, SHA256, true, true);
    assert_eq!(again.status, CreateStatus::AlreadyReferenced);
    assert_eq!(again.sidecars[0].outcome, WriteOutcome::AlreadyPresent);
    assert_eq!(
        std::fs::metadata(&sidecar).unwrap().modified().unwrap(),
        written_at,
        "not rewritten"
    );
    assert_eq!(fx.references(), 1, "no second item");
}

#[test]
fn a_modified_file_is_never_given_a_new_reference_silently() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"version 1");
    fx.create(&film, SHA256, true, true);
    let original = sidecar_text(&film, Algorithm::Sha256).unwrap();
    std::fs::write(&film, b"version 2").unwrap();

    let again = fx.create(&film, &[Algorithm::Sha256, Algorithm::Md5], true, true);
    assert_eq!(again.status, CreateStatus::Conflict, "{again:#?}");
    assert_eq!(
        sidecar_text(&film, Algorithm::Sha256).unwrap(),
        original,
        "not overwritten"
    );
    assert_eq!(
        sidecar_text(&film, Algorithm::Md5),
        None,
        "no new algorithm on changed content"
    );
    let db = again.database.unwrap();
    assert_eq!(db.outcome, WriteOutcome::Conflict);
    assert_eq!(db.existing_size, Some(9));
}

#[test]
fn an_algorithm_is_added_only_after_the_existing_reference_matched() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    fx.create(&film, &[Algorithm::Md5], true, false);
    let completed = fx.create(&film, SHA256, true, false);
    assert_eq!(completed.status, CreateStatus::Created, "{completed:#?}");
    let md5 = completed
        .sidecars
        .iter()
        .find(|s| s.algorithm == Algorithm::Md5)
        .unwrap();
    assert_eq!(
        (md5.requested, md5.outcome),
        (false, WriteOutcome::AlreadyPresent),
        "checked"
    );
    assert!(sidecar_text(&film, Algorithm::Sha256).is_some());

    // With the database: the reference gains the new algorithm.
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    fx.create(&film, &[Algorithm::Md5], false, true);
    let completed = fx.create(&film, &[Algorithm::Md5, Algorithm::Sha256], false, true);
    assert_eq!(completed.status, CreateStatus::Completed);
    let db = completed.database.unwrap();
    assert_eq!(
        (db.outcome, db.added),
        (WriteOutcome::Completed, vec![Algorithm::Sha256])
    );
}

#[test]
fn an_invalid_hash_file_blocks_and_is_left_alone() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    let garbage = fx.file("film.mkv.sha256", b"not a hash");
    let report = fx.create(&film, SHA256, true, true);
    assert_eq!(report.status, CreateStatus::Conflict);
    assert_eq!(report.sidecars[0].outcome, WriteOutcome::Invalid);
    assert_eq!(report.database.unwrap().outcome, WriteOutcome::Skipped);
    assert_eq!(std::fs::read(&garbage).unwrap(), b"not a hash");
    assert_eq!(fx.references(), 0);
}

#[test]
fn one_changed_byte_is_a_mismatch_and_the_reference_stays() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", &[0u8; 4096]);
    fx.create(&film, SHA256, true, true);
    let mut content = vec![0u8; 4096];
    content[2048] = 1;
    std::fs::write(&film, &content).unwrap();
    let report = fx.verify(&film, true, true);
    assert_eq!(report.status, VerifyStatus::Mismatch, "{report:#?}");
    assert_ne!(
        report.digests, report.sources[0].expected,
        "observed hash kept"
    );
    let again = fx.verify(&film, true, true);
    assert_eq!(
        again.status,
        VerifyStatus::Mismatch,
        "verification replaced nothing"
    );
}

#[test]
fn a_size_change_is_reported_as_a_mismatch_with_its_detail() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    fx.create(&film, SHA256, false, true);
    std::fs::write(&film, b"abcd").unwrap();
    let report = fx.verify(&film, false, true);
    assert_eq!(report.status, VerifyStatus::Mismatch);
    assert_eq!(
        report.sources[0].detail.as_deref(),
        Some("size changed: 3 → 4 bytes")
    );
}

#[test]
fn verifying_never_creates_a_reference() {
    let mut fx = Fixture::new();
    let film = fx.file("unknown.mkv", b"abc");
    let report = fx.verify(&film, true, true);
    assert_eq!(report.status, VerifyStatus::NoReference);
    assert!(
        report.digests.is_empty(),
        "no reference of this size: the file is not even read"
    );
    assert_eq!(sidecar_text(&film, Algorithm::Sha256), None);
    assert_eq!(fx.references(), 0);
    assert_eq!(fx.store.verification_count().unwrap(), 0);
}

#[test]
fn hash_file_and_database_that_disagree_give_a_conflict() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    fx.create(&film, SHA256, false, true);
    fx.file(
        "film.mkv.sha256",
        format!("{}  film.mkv\n", "0".repeat(64)).as_bytes(),
    );
    let report = fx.verify(&film, true, true);
    assert_eq!(report.status, VerifyStatus::Conflict, "{report:#?}");
    let outcomes: Vec<_> = report
        .sources
        .iter()
        .map(|s| (s.source, s.outcome))
        .collect();
    assert_eq!(
        outcomes,
        [
            (SourceKind::Sidecar, SourceOutcome::Mismatch),
            (SourceKind::Database, SourceOutcome::Match)
        ]
    );
}

#[test]
fn a_failed_hash_file_gives_a_partial_success() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    faults::arm(FaultPoint::SidecarPublish);
    let report = fx.create(&film, SHA256, true, true);
    faults::clear();
    assert_eq!(report.status, CreateStatus::PartialSuccess, "{report:#?}");
    assert_eq!(report.sidecars[0].outcome, WriteOutcome::Failed);
    assert!(report.sidecars[0].problem.is_some());
    assert_eq!(report.database.unwrap().outcome, WriteOutcome::Written);
    assert_eq!(sidecar_text(&film, Algorithm::Sha256), None);
    assert_eq!(
        fx.verify(&film, true, true).status,
        VerifyStatus::Match,
        "the database still works"
    );
}

#[test]
fn a_failed_database_write_gives_a_partial_success() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    faults::arm(FaultPoint::DatabaseWrite);
    let report = fx.create(&film, SHA256, true, true);
    faults::clear();
    assert_eq!(report.status, CreateStatus::PartialSuccess);
    assert_eq!(report.sidecars[0].outcome, WriteOutcome::Written);
    assert_eq!(report.database.unwrap().outcome, WriteOutcome::Failed);
    assert_eq!(fx.references(), 0);
}

#[test]
fn every_destination_failing_is_a_failure() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    faults::arm(FaultPoint::SidecarPublish);
    faults::arm(FaultPoint::DatabaseWrite);
    let report = fx.create(&film, SHA256, true, true);
    faults::clear();
    assert_eq!(report.status, CreateStatus::Failed);
}

#[test]
fn cancelling_leaves_nothing() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", &vec![1u8; 3 << 20]);
    let operation_id = fx.store.begin_operation(OperationKind::Create).unwrap();
    let control = Control::new();
    let report = create_reference(
        &film,
        &CreateOptions {
            algorithms: SHA256.to_vec(),
            write_sidecar: true,
            new_item: false,
            ..Default::default()
        },
        Some(DbTarget {
            store: &SharedStore::new(&mut fx.store),
            operation_id,
        }),
        &control,
        &mut |done, _| {
            if done > 0 {
                control.cancel();
            }
        },
    );
    assert_eq!(report.status, CreateStatus::Cancelled);
    assert_eq!(sidecar_text(&film, Algorithm::Sha256), None);
    assert_eq!(fx.references(), 0);
    let names: Vec<_> = std::fs::read_dir(fx.dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !names.iter().any(|n| n.ends_with(".tmp")),
        "no temporary file: {names:?}"
    );
}

#[test]
fn missing_file_and_legacy_hash_file() {
    let mut fx = Fixture::new();
    let missing = fx.dir.path().join("gone.mkv");
    assert_eq!(
        fx.verify(&missing, true, true).status,
        VerifyStatus::NotFound
    );

    let film = fx.file("Star Wars.mkv", b"abc");
    fx.file(
        "Star Wars.mkv.md5",
        b"MD5: 900150983cd24fb0d6963f7d28e17f72  Star Wars.mkv\r\n",
    );
    let report = fx.verify(&film, true, false);
    assert_eq!(report.status, VerifyStatus::Match);
    assert_eq!(report.sources[0].notes, [SidecarNote::NonStandardFormat]);
}

fn last_outcome(fx: &Fixture, path: &Path) -> Option<(String, Option<String>)> {
    let key = crate::platform::path_key(
        &crate::platform::absolute(path)
            .unwrap()
            .display()
            .to_string(),
    );
    let known = fx.store.current_by_path(&key).unwrap()?;
    let (_, outcome, detail) = fx.store.last_verification(known.item_id).unwrap()?;
    Some((outcome, detail))
}

#[test]
fn a_change_during_the_read_publishes_nothing() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", b"abc");
    faults::arm(FaultPoint::ChangeDuringRead);
    let report = fx.create(&film, &[Algorithm::Sha256, Algorithm::Md5], true, true);
    faults::clear();
    assert_eq!(report.status, CreateStatus::Failed);
    assert_eq!(
        report.problem.unwrap().kind,
        ProblemKind::ModifiedDuringRead
    );
    assert!(
        report.digests.is_empty(),
        "no digest of a file that changed"
    );
    assert_eq!(sidecar_text(&film, Algorithm::Sha256), None);
    assert_eq!(sidecar_text(&film, Algorithm::Md5), None);
    assert_eq!(fx.references(), 0);

    fx.create(&film, SHA256, true, true);
    faults::arm(FaultPoint::ChangeDuringRead);
    let verified = fx.verify(&film, true, true);
    faults::clear();
    assert_eq!(verified.status, VerifyStatus::ModifiedDuringRead);
    assert!(verified.digests.is_empty());
    assert_eq!(
        last_outcome(&fx, &film).map(|(o, _)| o).as_deref(),
        Some("modifiedDuringRead"),
        "recorded in the history"
    );
    assert_eq!(
        fx.verify(&film, true, true).status,
        VerifyStatus::Match,
        "the reference is intact"
    );
}

#[cfg(windows)]
#[test]
fn a_failing_disk_shows_in_the_history() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", &vec![5u8; 3 << 20]);
    fx.create(&film, SHA256, true, true);
    faults::arm_os_error(FaultPoint::Read, 23); // ERROR_CRC: unreadable sector
    let report = fx.verify(&film, true, true);
    faults::clear();
    assert_eq!(report.status, VerifyStatus::ReadError);
    assert_eq!(report.problem.unwrap().kind, ProblemKind::MediaError);
    let (outcome, detail) = last_outcome(&fx, &film).unwrap();
    assert_eq!(outcome, "readError");
    assert!(detail.unwrap().starts_with("MediaError"));
}

#[cfg(windows)]
#[test]
fn an_unplugged_drive_during_creation_writes_nothing() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", &vec![5u8; 3 << 20]);
    faults::arm_os_error(FaultPoint::Read, 21); // ERROR_NOT_READY
    let report = fx.create(&film, SHA256, true, true);
    faults::clear();
    assert_eq!(report.status, CreateStatus::Failed);
    assert_eq!(report.problem.unwrap().kind, ProblemKind::DeviceUnavailable);
    assert_eq!(sidecar_text(&film, Algorithm::Sha256), None);
    assert_eq!(fx.references(), 0);
}

#[test]
fn a_cancelled_verification_is_not_history() {
    let mut fx = Fixture::new();
    let film = fx.file("film.mkv", &vec![5u8; 3 << 20]);
    fx.create(&film, SHA256, true, true);
    let operation_id = fx.store.begin_operation(OperationKind::Verify).unwrap();
    let control = Control::new();
    let report = verify_file(
        &film,
        &VerifyOptions { use_sidecar: true },
        Some(DbTarget {
            store: &SharedStore::new(&mut fx.store),
            operation_id,
        }),
        &control,
        &mut |done, _| {
            if done > 0 {
                control.cancel();
            }
        },
    );
    assert_eq!(report.status, VerifyStatus::Cancelled);
    assert_eq!(fx.store.verification_count().unwrap(), 0);
}

#[test]
fn an_identical_hash_file_in_the_old_format_is_kept_byte_for_byte() {
    let mut fx = Fixture::new();
    let film = fx.file("Star Wars.mkv", b"abc");
    let legacy = b"MD5: 900150983cd24fb0d6963f7d28e17f72  Star Wars.mkv\r\n";
    let md5 = fx.file("Star Wars.mkv.md5", legacy);
    let report = fx.create(&film, &[Algorithm::Md5, Algorithm::Sha256], true, false);
    assert_eq!(report.status, CreateStatus::Completed, "{report:#?}");
    let md5_result = report
        .sidecars
        .iter()
        .find(|s| s.algorithm == Algorithm::Md5)
        .unwrap();
    assert_eq!(md5_result.outcome, WriteOutcome::AlreadyPresent);
    assert_eq!(md5_result.notes, [SidecarNote::NonStandardFormat]);
    assert_eq!(
        std::fs::read(&md5).unwrap(),
        legacy,
        "not rewritten in the new format"
    );
    assert!(
        sidecar_text(&film, Algorithm::Sha256).is_some(),
        "the new algorithm was added"
    );
}

#[test]
fn a_hash_file_renamed_with_its_file_is_still_its_reference() {
    let mut fx = Fixture::new();
    let film = fx.file("new name.mkv", b"abc");
    let old = format!(
        "{}  old name.mkv\n",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let sidecar = fx.file("new name.mkv.sha256", old.as_bytes());
    let created = fx.create(&film, SHA256, true, false);
    assert_eq!(created.status, CreateStatus::AlreadyReferenced);
    assert_eq!(
        created.sidecars[0].notes,
        [SidecarNote::NameDiffers {
            written: "old name.mkv".to_owned()
        }]
    );
    assert_eq!(
        std::fs::read_to_string(&sidecar).unwrap(),
        old,
        "left as it was"
    );
    let verified = fx.verify(&film, true, false);
    assert_eq!(verified.status, VerifyStatus::Match);
}

#[test]
fn paths_longer_than_260_characters() {
    let mut fx = Fixture::new();
    let mut dir = fx.dir.path().to_path_buf();
    while dir.as_os_str().len() < 300 {
        dir.push("un dossier au nom assez long pour dépasser MAX_PATH");
    }
    std::fs::create_dir_all(&dir).unwrap();
    let film = dir.join("Le film au bout du chemin.mkv");
    std::fs::write(&film, b"abc").unwrap();
    assert!(film.as_os_str().len() > 260);
    let created = fx.create(&film, &[Algorithm::Sha256, Algorithm::Md5], true, true);
    assert_eq!(created.status, CreateStatus::Created, "{created:#?}");
    let verified = fx.verify(&film, true, true);
    assert_eq!(verified.status, VerifyStatus::Match);
    assert_eq!(verified.sources.len(), 3, "two hash files and the database");
}
