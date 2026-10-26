//! Folder hash lists (`SHA256SUMS`, `MD5SUMS`) as a destination of Create,
//! and as references when verifying and accepting a new version.

use std::path::{Path, PathBuf};

use super::decide::{self, Observation};
use super::*;
use crate::faults::{self, FaultPoint};
use crate::store::{OperationKind, SharedStore, Store};

struct Fixture {
    dir: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("lists.fileseal"), "Lists").unwrap();
        Fixture { dir, store }
    }

    fn file(&self, relative: &str, content: &[u8]) -> PathBuf {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
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

    /// Created and finished, as a batch does once the folder is done.
    fn create_all(
        &mut self,
        paths: &[&Path],
        options: &CreateOptions,
        db: bool,
    ) -> Vec<CreateReport> {
        let mut reports: Vec<CreateReport> =
            paths.iter().map(|p| self.create(p, options, db)).collect();
        finish_sums(&mut reports);
        reports
    }

    fn verify(&mut self, path: &Path) -> VerifyReport {
        let options = VerifyOptions {
            use_sidecar: true,
            follow_moves: false,
        };
        verify_file(path, &options, None, &Control::new(), &mut |_, _| {})
    }
}

fn lists(algorithms: &[Algorithm]) -> CreateOptions {
    CreateOptions {
        algorithms: algorithms.to_vec(),
        write_sums: true,
        ..Default::default()
    }
}

fn digest_of(report: &CreateReport, algorithm: Algorithm) -> String {
    algorithm::find(&report.digests, algorithm)
        .unwrap()
        .hex
        .clone()
}

#[test]
fn the_list_is_written_once_the_files_are_done() {
    let mut fx = Fixture::new();
    let alien = fx.file(r"Movies\Alien (1979).mkv", b"alien");
    let blade = fx.file(r"Movies\Blade Runner (1982).mkv", b"blade runner");
    let options = lists(&[Algorithm::Sha256]);

    // Each file alone: its line waits for the folder.
    let waiting = fx.create(&alien, &options, false);
    assert_eq!(waiting.status, CreateStatus::Pending);
    assert_eq!(waiting.sidecars[0].outcome, WriteOutcome::Pending);
    assert!(waiting.sidecars[0].list);
    assert!(
        !fx.dir.path().join(r"Movies\SHA256SUMS").exists(),
        "not yet"
    );

    let reports = fx.create_all(&[&blade, &alien], &options, false);
    for report in &reports {
        assert_eq!(report.status, CreateStatus::Created, "{report:#?}");
        assert_eq!(report.sidecars[0].outcome, WriteOutcome::Written);
    }
    assert_eq!(
        std::fs::read_to_string(fx.dir.path().join(r"Movies\SHA256SUMS")).unwrap(),
        format!(
            "{}  Alien (1979).mkv\n{}  Blade Runner (1982).mkv\n",
            digest_of(&reports[1], Algorithm::Sha256),
            digest_of(&reports[0], Algorithm::Sha256)
        ),
        "one list for the folder, sorted"
    );
    // Run again: nothing to write.
    let again = fx.create_all(&[&alien], &options, false);
    assert_eq!(again[0].status, CreateStatus::AlreadyReferenced);
}

#[test]
fn one_list_per_folder_and_per_algorithm() {
    let mut fx = Fixture::new();
    let a = fx.file(r"Movies\Alien (1979)\Alien (1979).mkv", b"alien");
    let b = fx.file(r"Movies\Dune (2021)\Dune (2021).mkv", b"dune");
    let reports = fx.create_all(
        &[&a, &b],
        &lists(&[Algorithm::Sha256, Algorithm::Md5]),
        false,
    );
    for report in &reports {
        assert_eq!(report.status, CreateStatus::Created);
        assert_eq!(report.sidecars.len(), 2);
    }
    for folder in [r"Movies\Alien (1979)", r"Movies\Dune (2021)"] {
        for list in ["SHA256SUMS", "MD5SUMS"] {
            let text = std::fs::read_to_string(fx.dir.path().join(folder).join(list)).unwrap();
            assert_eq!(
                text.lines().count(),
                1,
                "{folder}\\{list}: its own file only"
            );
        }
    }
    assert!(!fx.dir.path().join(r"Movies\SHA256SUMS").exists());
}

#[test]
fn a_line_already_there_is_kept_and_one_that_disagrees_blocks_the_file() {
    let mut fx = Fixture::new();
    let alien = fx.file(r"Movies\Alien (1979).mkv", b"alien");
    let dune = fx.file(r"Movies\Dune (2021).mkv", b"dune");
    let options = lists(&[Algorithm::Sha256]);
    let first = fx.create_all(&[&alien], &options, false);
    let list = fx.dir.path().join(r"Movies\SHA256SUMS");
    // Dune's line says something else: its content changed, or the line is wrong.
    let wrong = "0".repeat(64);
    let before = format!(
        "{}  Alien (1979).mkv\n{wrong}  Dune (2021).mkv\n",
        digest_of(&first[0], Algorithm::Sha256)
    );
    std::fs::write(&list, &before).unwrap();

    let options = CreateOptions {
        write_sidecar: true,
        ..options
    };
    let reports = fx.create_all(&[&alien, &dune], &options, false);
    assert_eq!(
        reports[0].status,
        CreateStatus::Completed,
        "hash file added"
    );
    assert_eq!(
        reports[1].status,
        CreateStatus::Conflict,
        "{:#?}",
        reports[1]
    );
    assert!(
        !fx.dir
            .path()
            .join(r"Movies\Dune (2021).mkv.sha256")
            .exists(),
        "nothing written for a file in conflict"
    );
    assert_eq!(
        std::fs::read_to_string(&list).unwrap(),
        before,
        "list untouched"
    );
}

#[test]
fn a_file_its_list_covers_is_not_read_again() {
    let mut fx = Fixture::new();
    let alien = fx.file(r"Movies\Alien (1979).mkv", b"alien");
    let dune = fx.file(r"Movies\Dune (2021).mkv", b"dune");
    fx.create_all(&[&alien], &lists(&[Algorithm::Sha256]), false);
    let options = CreateOptions {
        existing: ExistingReferences::SkipComplete,
        ..lists(&[Algorithm::Sha256])
    };
    faults::arm_under(&fx.dir.path().join("Movies"), FaultPoint::Read, None);
    let skipped = fx.create(&alien, &options, false);
    faults::clear_under(&fx.dir.path().join("Movies"));
    assert_eq!(skipped.status, CreateStatus::Skipped, "not read");
    let read = fx.create_all(&[&dune], &options, false);
    assert_eq!(read[0].status, CreateStatus::Created, "no line yet: read");
}

#[test]
fn verifying_uses_the_folder_list() {
    let mut fx = Fixture::new();
    let alien = fx.file(r"Movies\Alien (1979).mkv", b"alien");
    fx.create_all(&[&alien], &lists(&[Algorithm::Sha256]), false);
    let report = fx.verify(&alien);
    assert_eq!(report.status, VerifyStatus::Match);
    assert_eq!(report.sources.len(), 1);
    assert_eq!(report.sources[0].source, SourceKind::HashList);
    assert!(report.sources[0].path.ends_with("SHA256SUMS"));

    std::fs::write(&alien, b"alien, changed").unwrap();
    assert_eq!(fx.verify(&alien).status, VerifyStatus::Mismatch);
}

#[test]
fn accepting_a_new_version_changes_its_line_only() {
    let mut fx = Fixture::new();
    let alien = fx.file(r"Movies\Alien (1979).mkv", b"alien");
    let dune = fx.file(r"Movies\Dune (2021).mkv", b"dune");
    fx.create_all(&[&alien, &dune], &lists(&[Algorithm::Sha256]), false);
    let list = fx.dir.path().join(r"Movies\SHA256SUMS");
    let dune_line = std::fs::read_to_string(&list)
        .unwrap()
        .lines()
        .find(|l| l.ends_with("Dune (2021).mkv"))
        .unwrap()
        .to_owned();

    std::fs::write(&alien, b"alien, director's cut").unwrap();
    let verified = fx.verify(&alien);
    assert_eq!(verified.status, VerifyStatus::Mismatch);
    let observed = Observation {
        size: verified.size.unwrap(),
        modified: verified.observed_mtime.clone(),
        digests: verified.digests.clone(),
    };
    let accepted = decide::accept(&alien, &observed, None, true, None).unwrap();
    assert_eq!(accepted.sidecars.len(), 1);
    assert!(accepted.sidecars[0].list);
    assert_eq!(accepted.sidecars[0].outcome, WriteOutcome::Written);

    assert_eq!(fx.verify(&alien).status, VerifyStatus::Match);
    assert!(
        std::fs::read_to_string(&list).unwrap().contains(&dune_line),
        "the other lines as they were"
    );
}

#[test]
fn a_list_can_be_recorded_in_the_database_without_reading() {
    let mut fx = Fixture::new();
    let alien = fx.file(r"Movies\Alien (1979).mkv", b"alien");
    // A list made by another tool.
    fx.create_all(&[&alien], &lists(&[Algorithm::Sha256]), false);
    let options = CreateOptions {
        existing: ExistingReferences::SkipComplete,
        import_hash_files: true,
        ..lists(&[Algorithm::Sha256])
    };
    faults::arm_under(&fx.dir.path().join("Movies"), FaultPoint::Read, None);
    let report = fx.create(&alien, &options, true);
    faults::clear_under(&fx.dir.path().join("Movies"));
    assert_eq!(report.status, CreateStatus::Imported, "{report:#?}");
    assert_eq!(fx.store.info().unwrap().references, 1);
}

#[test]
fn a_list_that_cannot_be_written_fails_for_its_files_only() {
    let mut fx = Fixture::new();
    let alien = fx.file(r"Movies\Alien (1979).mkv", b"alien");
    let options = CreateOptions {
        write_sidecar: true,
        ..lists(&[Algorithm::Sha256])
    };
    let mut reports = vec![fx.create(&alien, &options, false)];
    faults::arm(FaultPoint::SidecarPublish);
    finish_sums(&mut reports);
    faults::clear();
    let report = &reports[0];
    assert_eq!(report.status, CreateStatus::PartialSuccess, "{report:#?}");
    let list = report.sidecars.iter().find(|s| s.list).unwrap();
    assert_eq!(list.outcome, WriteOutcome::Failed);
    assert!(list.problem.is_some());
    assert!(
        fx.dir
            .path()
            .join(r"Movies\Alien (1979).mkv.sha256")
            .exists()
    );
}
