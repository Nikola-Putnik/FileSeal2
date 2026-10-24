//! Locations followed automatically (`follow_moves`): a file found
//! elsewhere than recorded is attached to its item without asking, when its
//! content or its place on its disk says which item it is; a copy never is.

use std::path::{Path, PathBuf};

use super::*;
use crate::matching::ContentOutcome;
use crate::store::{OperationKind, SharedStore, Store};

struct Fixture {
    dir: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("follow.fileseal"), "Follow").unwrap();
        Fixture { dir, store }
    }

    fn file(&self, relative: &str, content: &[u8]) -> PathBuf {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    fn create_with(&mut self, path: &Path, options: CreateOptions) -> CreateReport {
        let operation_id = self.store.begin_operation(OperationKind::Create).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = DbTarget {
            store: &shared,
            operation_id,
        };
        create_reference(
            path,
            &options,
            Some(target),
            &Control::new(),
            &mut |_, _| {},
        )
    }

    /// SHA-256 in the database only, locations followed or not.
    fn create(&mut self, path: &Path, follow_moves: bool) -> CreateReport {
        self.create_with(
            path,
            CreateOptions {
                algorithms: vec![Algorithm::Sha256],
                follow_moves,
                ..Default::default()
            },
        )
    }

    fn verify(&mut self, path: &Path, follow_moves: bool) -> VerifyReport {
        let operation_id = self.store.begin_operation(OperationKind::Verify).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = DbTarget {
            store: &shared,
            operation_id,
        };
        let options = VerifyOptions {
            use_sidecar: true,
            follow_moves,
        };
        verify_file(
            path,
            &options,
            Some(target),
            &Control::new(),
            &mut |_, _| {},
        )
    }
}

fn move_file(from: &Path, to: &Path) {
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::rename(from, to).unwrap();
}

fn text(path: &Path) -> String {
    path.display().to_string()
}

/// A drive letter with no disk behind it here.
fn unused_letter() -> Option<char> {
    ('M'..='Z').find(|l| !Path::new(&format!("{l}:\\")).exists())
}

#[test]
fn verifying_a_moved_file_attaches_it_then_compares() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Films\Blade Runner.mkv", b"the film");
    let item = fx.create(&old, false).item_id.unwrap();
    let new = fx.path(r"Archive\Blade Runner (1982).mkv");
    move_file(&old, &new);

    let report = fx.verify(&new, true);
    assert_eq!(report.status, VerifyStatus::Match, "against its item");
    assert_eq!(report.item_id, Some(item));
    assert_eq!(report.relocated_from, Some(text(&old)));
    assert_eq!(report.finding, None, "nothing left to decide");
    let database = report
        .sources
        .iter()
        .find(|s| s.source == SourceKind::Database);
    assert_eq!(database.unwrap().path, text(&new));

    // Known at its new place from now on, with the former one in history.
    let again = fx.verify(&new, false);
    assert_eq!(
        (again.status, again.relocated_from),
        (VerifyStatus::Match, None)
    );
    let history = fx.store.item_history(item).unwrap();
    assert_eq!(history.locations.len(), 2);
    assert!(history.locations[0].current);
    assert_eq!(history.locations[0].path, text(&new));
    let followed = history.verifications.last().unwrap();
    assert!(
        followed
            .detail
            .as_deref()
            .unwrap()
            .contains("location followed"),
        "the history says why it moved"
    );
    assert_eq!(fx.store.info().unwrap().references, 1, "still one item");
}

#[test]
fn creating_for_a_moved_file_attaches_it_and_completes_it() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Old\film.mkv", b"moved bytes");
    let item = fx.create(&old, false).item_id.unwrap();
    let new = fx.path(r"New\film.mkv");
    move_file(&old, &new);

    let report = fx.create_with(
        &new,
        CreateOptions {
            algorithms: vec![Algorithm::Sha256, Algorithm::Md5],
            follow_moves: true,
            ..Default::default()
        },
    );
    assert_eq!(report.status, CreateStatus::Completed, "MD5 added");
    let database = report.database.unwrap();
    assert_eq!(database.outcome, WriteOutcome::Completed);
    assert_eq!(database.added, vec![Algorithm::Md5]);
    assert_eq!(report.item_id, Some(item));
    assert_eq!(report.relocated_from, Some(text(&old)));
    assert_eq!(report.finding, None);
    assert_eq!(fx.store.info().unwrap().references, 1);
    assert_eq!(fx.verify(&new, false).status, VerifyStatus::Match);
}

#[test]
fn a_new_item_asked_for_is_not_attached() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Old\film.mkv", b"moved bytes");
    let item = fx.create(&old, false).item_id.unwrap();
    let new = fx.path(r"New\film.mkv");
    move_file(&old, &new);
    let report = fx.create_with(
        &new,
        CreateOptions {
            algorithms: vec![Algorithm::Sha256],
            new_item: true,
            follow_moves: true,
            ..Default::default()
        },
    );
    assert_eq!(report.status, CreateStatus::Created);
    assert_ne!(report.item_id, Some(item));
    assert_eq!(report.relocated_from, None);
}

#[test]
fn a_copy_is_never_attached() {
    let mut fx = Fixture::new();
    let original = fx.file(r"Internal\film.mkv", b"same bytes");
    let item = fx.create(&original, false).item_id.unwrap();
    let copy = fx.file(r"Backup\film.mkv", b"same bytes");

    let verified = fx.verify(&copy, true);
    assert_eq!(verified.status, VerifyStatus::NoReference);
    assert_eq!(verified.relocated_from, None);
    assert_eq!(
        verified.finding.map(|f| f.outcome),
        Some(ContentOutcome::CopyOfKnown)
    );
    let created = fx.create(&copy, true);
    assert_eq!(created.status, CreateStatus::Created, "a new item");
    assert_ne!(created.item_id, Some(item));
    assert_eq!(fx.verify(&original, false).status, VerifyStatus::Match);
}

#[test]
fn a_copy_on_an_unplugged_disk_is_not_attached() {
    let mut fx = Fixture::new();
    let original = fx.file(r"A\film.mkv", b"backed up");
    let item = fx.create(&original, false).item_id.unwrap();
    let Some(letter) = unused_letter() else {
        return;
    };
    fx.store
        .set_location_for_test(item, &format!(r"{letter}:\Backups\film.mkv"));
    let copy = fx.file(r"B\film.mkv", b"backed up");
    let report = fx.verify(&copy, true);
    assert_eq!(report.relocated_from, None, "it may still be there");
    assert_eq!(
        report.finding.map(|f| f.outcome),
        Some(ContentOutcome::CopyOfKnown)
    );
}

#[test]
fn missing_twins_are_attached_one_each() {
    let mut fx = Fixture::new();
    let one = fx.file(r"A\one.mkv", b"twins");
    let two = fx.file(r"B\two.mkv", b"twins");
    let first = fx.create(&one, false).item_id.unwrap();
    let second = fx.create(&two, false).item_id.unwrap();
    std::fs::remove_file(&one).unwrap();
    std::fs::remove_file(&two).unwrap();

    // Ambiguous: the one with the same name, then the one left.
    let named = fx.file(r"C\two.mkv", b"twins");
    let report = fx.verify(&named, true);
    assert_eq!(
        (report.status, report.item_id),
        (VerifyStatus::Match, Some(second))
    );
    let other = fx.file(r"C\renamed.mkv", b"twins");
    let report = fx.verify(&other, true);
    assert_eq!(
        (report.status, report.item_id),
        (VerifyStatus::Match, Some(first))
    );
    // None missing any more: a third one is a copy.
    let third = fx.file(r"C\third.mkv", b"twins");
    assert_eq!(fx.verify(&third, true).relocated_from, None);
}

#[test]
fn a_drive_letter_change_is_followed_even_without_reading() {
    let mut fx = Fixture::new();
    let film = fx.file(r"Films\film.mkv", b"on an external disk");
    let item = fx.create(&film, false).item_id.unwrap();
    let Some(letter) = unused_letter() else {
        return;
    };
    let old_path = format!(r"{letter}:\Films\film.mkv");
    fx.store.set_location_for_test(item, &old_path);

    // Skipped as already referenced: its place on its disk says which item
    // it is, without reading it.
    let report = fx.create_with(
        &film,
        CreateOptions {
            algorithms: vec![Algorithm::Sha256],
            existing: ExistingReferences::SkipComplete,
            follow_moves: true,
            ..Default::default()
        },
    );
    assert_eq!(report.status, CreateStatus::Skipped);
    assert_eq!(report.relocated_from.as_deref(), Some(old_path.as_str()));
    assert_eq!(report.location_change, None, "nothing left to propose");
    let verified = fx.verify(&film, false);
    assert_eq!(verified.status, VerifyStatus::Match);
    assert_eq!(verified.location_change, None, "recorded at its new letter");

    // The same when verifying.
    fx.store.set_location_for_test(item, &old_path);
    let report = fx.verify(&film, true);
    assert_eq!(report.status, VerifyStatus::Match);
    assert_eq!(report.relocated_from.as_deref(), Some(old_path.as_str()));
    assert_eq!(report.location_change, None);
}

#[test]
fn an_item_moved_meanwhile_is_not_moved_again() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Old\film.mkv", b"taken");
    let item = fx.create(&old, false).item_id.unwrap();
    let new = fx.path(r"New\film.mkv");
    move_file(&old, &new);
    let (_, location) = locate(&new).unwrap();
    // Another file of the batch took it first: it is no longer at `old`.
    let elsewhere = fx.path(r"Elsewhere\film.mkv");
    fx.store.set_location_for_test(item, &text(&elsewhere));
    let moved = fx
        .store
        .relocate_from(item, &platform::path_key(&text(&old)), &location)
        .unwrap();
    assert!(!moved);
    let current = fx.store.current_of_item(item).unwrap().unwrap();
    assert_eq!(current.path, text(&elsewhere), "left as it was");
}
