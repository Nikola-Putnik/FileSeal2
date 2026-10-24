//! Step 5 scenarios: one or more per matching rule (docs/rapprochement.md),
//! through the real operations: create, verify, search, attach, accept.

use std::path::{Path, PathBuf};

use super::decide::{self, Observation};
use super::search::{self, SearchStatus};
use super::*;
use crate::matching::{ContentOutcome, Presence};
use crate::store::{OperationKind, SharedStore, Store};

const SHA256: &[Algorithm] = &[Algorithm::Sha256];

struct Fixture {
    dir: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("rules.fileseal"), "Rules").unwrap();
        Fixture { dir, store }
    }

    fn file(&self, relative: &str, content: &[u8]) -> PathBuf {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    fn create_with(&mut self, path: &Path, new_item: bool) -> CreateReport {
        let operation_id = self.store.begin_operation(OperationKind::Create).unwrap();
        let options = CreateOptions {
            algorithms: SHA256.to_vec(),
            write_sidecar: false,
            new_item,
            ..Default::default()
        };
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

    fn create(&mut self, path: &Path) -> CreateReport {
        self.create_with(path, false)
    }

    fn verify(&mut self, path: &Path) -> VerifyReport {
        let operation_id = self.store.begin_operation(OperationKind::Verify).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = DbTarget {
            store: &shared,
            operation_id,
        };
        verify_file(
            path,
            &VerifyOptions {
                use_sidecar: true,
                follow_moves: false,
            },
            Some(target),
            &Control::new(),
            &mut |_, _| {},
        )
    }

    fn search(&mut self, path: &Path) -> search::SearchReport {
        let shared = SharedStore::new(&mut self.store);
        search::search_file(path, &shared, &Control::new(), &mut |_, _| {})
    }
}

fn observation(report: &VerifyReport) -> Observation {
    Observation {
        size: report.size.unwrap(),
        modified: report.observed_mtime.clone(),
        digests: report.digests.clone(),
    }
}

fn move_file(from: &Path, to: &Path) {
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::rename(from, to).unwrap();
}

#[test]
fn r5_a_moved_file_is_found_by_content_then_attached() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Disk\Films\Blade Runner.mkv", b"the film");
    let item = fx.create(&old).item_id.unwrap();
    let new = fx.dir.path().join(r"Disk\Archive\Blade Runner.mkv");
    move_file(&old, &new);

    let report = fx.verify(&new);
    assert_eq!(
        report.status,
        VerifyStatus::NoReference,
        "not a match by itself"
    );
    let finding = report.finding.clone().expect("content looked up");
    assert_eq!(finding.outcome, ContentOutcome::ProbablyMoved);
    assert_eq!(finding.same_content[0].item_id, item);
    assert_eq!(finding.same_content[0].presence, Presence::Missing);
    assert_eq!(fx.search(&new).status, SearchStatus::ProbablyMoved);

    let attached = decide::attach(&new, item, &observation(&report), false, &mut fx.store).unwrap();
    assert!(attached.same_content);
    assert!(attached.previous_path.ends_with(r"Films\Blade Runner.mkv"));
    let again = fx.verify(&new);
    assert_eq!(
        again.status,
        VerifyStatus::Match,
        "now verified at its new location"
    );
    assert_eq!(again.item_id, Some(item));
    assert_eq!(fx.store.info().unwrap().references, 1, "still one item");
}

#[test]
fn r5_a_renamed_file_is_found_by_content() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Films\film (1).mkv", b"renamed content");
    let item = fx.create(&old).item_id.unwrap();
    let new = fx.dir.path().join(r"Films\Blade Runner (1982).mkv");
    move_file(&old, &new);
    let finding = fx.verify(&new).finding.expect("found");
    assert_eq!(
        (finding.outcome, finding.same_content[0].item_id),
        (ContentOutcome::ProbablyMoved, item)
    );
}

#[test]
fn r6_same_name_different_content_is_never_merged() {
    let mut fx = Fixture::new();
    let first = fx.file(r"A\film.mkv", b"first film");
    let item = fx.create(&first).item_id.unwrap();
    let other = fx.file(r"B\film.mkv", b"another film, same name");

    let report = fx.verify(&other);
    assert_eq!(report.status, VerifyStatus::NoReference);
    let finding = report.finding.clone().expect("a hint");
    assert_eq!(finding.outcome, ContentOutcome::Unknown);
    assert!(finding.same_content.is_empty());
    assert_eq!(finding.hints[0].item_id, item, "same name: a hint only");
    assert_eq!(fx.search(&other).status, SearchStatus::Unknown);

    // Attaching it would merge two different files: refused unless forced.
    let refused = decide::attach(&other, item, &observation(&report), false, &mut fx.store);
    assert_eq!(refused.unwrap_err().kind, ProblemKind::ContentDiffers);

    // Creating it makes a distinct item.
    let created = fx.create(&other);
    assert_eq!(created.status, CreateStatus::Created);
    assert_ne!(created.item_id, Some(item));
    assert_eq!(fx.store.info().unwrap().references, 2);
}

#[test]
fn r7_renamed_and_modified_can_be_attached_by_hand_then_accepted() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Films\draft.mkv", b"version 1");
    let item = fx.create(&old).item_id.unwrap();
    std::fs::remove_file(&old).unwrap();
    let new = fx.file(r"Films\final.mkv", b"version 2, longer");

    let report = fx.verify(&new);
    assert_eq!(
        report.finding.as_ref().map(|f| f.outcome),
        None,
        "neither the content nor the name says where it comes from"
    );
    // The user knows: attach despite the difference, then accept.
    let observed = Observation {
        size: std::fs::metadata(&new).unwrap().len(),
        modified: None,
        digests: vec![],
    };
    let attached = decide::attach(&new, item, &observed, true, &mut fx.store).unwrap();
    assert!(!attached.same_content);
    let verified = fx.verify(&new);
    assert_eq!(
        verified.status,
        VerifyStatus::Mismatch,
        "a known item that changed"
    );
    decide::accept(
        &new,
        &observation(&verified),
        verified.verification_id,
        false,
        Some(&mut fx.store),
    )
    .unwrap();
    assert_eq!(fx.verify(&new).status, VerifyStatus::Match);
}

#[test]
fn copies_are_separate_items_and_recognised_as_copies() {
    let mut fx = Fixture::new();
    let original = fx.file(r"Internal\film.mkv", b"same bytes");
    let backup = fx.file(r"Backup\film.mkv", b"same bytes");
    let first = fx.create(&original);
    let second = fx.create(&backup);
    assert_eq!(
        second.status,
        CreateStatus::Created,
        "the original is still there: a copy"
    );
    assert_eq!(
        second.finding.as_ref().map(|f| f.outcome),
        Some(ContentOutcome::CopyOfKnown)
    );
    assert_ne!(first.item_id, second.item_id);

    let third = fx.file(r"Usb\film copy.mkv", b"same bytes");
    let finding = fx.verify(&third).finding.unwrap();
    assert_eq!(finding.outcome, ContentOutcome::CopyOfKnown);
    assert_eq!(finding.same_content.len(), 2);
    assert_eq!(fx.search(&third).status, SearchStatus::CopyOfKnown);
}

#[test]
fn several_missing_copies_are_ambiguous() {
    let mut fx = Fixture::new();
    let a = fx.file(r"A\one.mkv", b"twins");
    let b = fx.file(r"B\two.mkv", b"twins");
    fx.create(&a);
    fx.create(&b);
    std::fs::remove_file(&a).unwrap();
    std::fs::remove_file(&b).unwrap();
    let found = fx.file(r"C\which one.mkv", b"twins");
    let report = fx.verify(&found);
    assert_eq!(
        report.finding.as_ref().map(|f| f.outcome),
        Some(ContentOutcome::Ambiguous)
    );
    assert_eq!(fx.search(&found).status, SearchStatus::Ambiguous);
}

#[test]
fn creating_a_moved_file_waits_for_a_decision() {
    let mut fx = Fixture::new();
    let old = fx.file(r"Old\film.mkv", b"moved bytes");
    let item = fx.create(&old).item_id.unwrap();
    let new = fx.dir.path().join(r"New\film.mkv");
    move_file(&old, &new);

    let report = fx.create(&new);
    assert_eq!(report.status, CreateStatus::NeedsDecision);
    assert_eq!(
        report.database.unwrap().outcome,
        WriteOutcome::NeedsDecision
    );
    assert_eq!(report.finding.unwrap().same_content[0].item_id, item);
    assert_eq!(fx.store.info().unwrap().references, 1, "nothing written");

    // The user says it is another copy after all.
    let forced = fx.create_with(&new, true);
    assert_eq!(forced.status, CreateStatus::Created);
    assert_eq!(fx.store.info().unwrap().references, 2);
}

#[test]
fn a_copy_on_an_unplugged_disk_is_not_a_move() {
    let mut fx = Fixture::new();
    let original = fx.file(r"A\film.mkv", b"backed up");
    let item = fx.create(&original).item_id.unwrap();
    // Record it on a drive letter that does not exist here: an unplugged
    // backup disk.
    let Some(letter) = ('M'..='Z').find(|l| !Path::new(&format!("{l}:\\")).exists()) else {
        return;
    };
    let unplugged = format!(r"{letter}:\Backups\film.mkv");
    fx.store.set_location_for_test(item, &unplugged);
    let copy = fx.file(r"B\film.mkv", b"backed up");
    let finding = fx.verify(&copy).finding.unwrap();
    assert_eq!(
        finding.same_content[0].presence,
        Presence::VolumeUnavailable
    );
    assert_eq!(finding.outcome, ContentOutcome::CopyOfKnown, "not a move");
}

#[test]
fn r2_a_drive_letter_change_is_recognised() {
    let mut fx = Fixture::new();
    let film = fx.file(r"Films\film.mkv", b"on an external disk");
    let item = fx.create(&film).item_id.unwrap();
    // As if recorded when the same disk was mounted as another letter: the
    // path changes, the volume and the place on it do not.
    let Some(letter) = ('M'..='Z').find(|l| !Path::new(&format!("{l}:\\")).exists()) else {
        return;
    };
    let old_path = format!(r"{letter}:\somewhere\film.mkv");
    fx.store.set_location_for_test(item, &old_path);

    let report = fx.verify(&film);
    assert_eq!(
        report.status,
        VerifyStatus::Match,
        "verified against its own reference"
    );
    assert_eq!(report.item_id, Some(item));
    assert_eq!(report.location_change.as_deref(), Some(old_path.as_str()));
    assert_eq!(fx.search(&film).status, SearchStatus::LocationRecognized);

    // The user confirms: the recorded location follows.
    decide::attach(&film, item, &observation(&report), false, &mut fx.store).unwrap();
    assert_eq!(fx.verify(&film).location_change, None);
}

#[test]
fn r4_another_items_content_never_makes_a_match() {
    let mut fx = Fixture::new();
    let a = fx.file(r"A\a.mkv", b"content of A");
    let b = fx.file(r"B\b.mkv", b"content of B");
    fx.create(&a);
    let b_item = fx.create(&b).item_id.unwrap();
    std::fs::write(&a, b"content of B").unwrap();

    let report = fx.verify(&a);
    assert_eq!(
        report.status,
        VerifyStatus::Mismatch,
        "A's reference is what counts"
    );
    let finding = report.finding.unwrap();
    assert_eq!(
        finding.same_content[0].item_id, b_item,
        "noted: identical to B"
    );
}

#[test]
fn accepting_a_new_version_keeps_the_history() {
    let mut fx = Fixture::new();
    let film = fx.file(r"Films\film.mkv", b"version 1");
    let item = fx.create(&film).item_id.unwrap();
    crate::sidecar::write_new(&film, &fx.verify(&film).digests[0]).unwrap();
    std::fs::write(&film, b"version 2").unwrap();

    let verified = fx.verify(&film);
    assert_eq!(verified.status, VerifyStatus::Mismatch);
    let accepted = decide::accept(
        &film,
        &observation(&verified),
        verified.verification_id,
        true,
        Some(&mut fx.store),
    )
    .unwrap();
    assert_eq!(accepted.database, Some(WriteOutcome::Written));
    assert_eq!(accepted.sidecars[0].outcome, WriteOutcome::Written);

    let after = fx.verify(&film);
    assert_eq!(
        after.status,
        VerifyStatus::Match,
        "database and hash file agree"
    );
    assert_eq!(after.sources.len(), 2);
    let versions = fx.store.references_of(item).unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].origin, "accepted");
    assert!(versions[0].superseded_at.is_none());
    assert!(
        versions[1].superseded_at.is_some(),
        "the old one is history"
    );

    // The same verification cannot be accepted twice.
    let twice = decide::accept(
        &film,
        &observation(&verified),
        verified.verification_id,
        false,
        Some(&mut fx.store),
    )
    .unwrap();
    assert_eq!(twice.database, Some(WriteOutcome::Failed));
    assert_eq!(
        twice.database_problem.unwrap().kind,
        ProblemKind::StaleVerification
    );
}

#[test]
fn a_file_changed_again_cannot_be_accepted_without_verifying_again() {
    let mut fx = Fixture::new();
    let film = fx.file(r"Films\film.mkv", b"version 1");
    fx.create(&film);
    std::fs::write(&film, b"version 2").unwrap();
    let verified = fx.verify(&film);
    std::fs::write(&film, b"version 3, longer").unwrap();
    let refused = decide::accept(
        &film,
        &observation(&verified),
        verified.verification_id,
        true,
        Some(&mut fx.store),
    );
    assert_eq!(refused.unwrap_err().kind, ProblemKind::StaleVerification);
}

#[test]
fn search_tells_without_writing() {
    let mut fx = Fixture::new();
    let film = fx.file(r"Films\film.mkv", b"known");
    fx.create(&film);
    assert_eq!(fx.search(&film).status, SearchStatus::KnownHere);
    std::fs::write(&film, b"changed").unwrap();
    assert_eq!(fx.search(&film).status, SearchStatus::ChangedHere);

    // No reference of this size: unknown without reading it.
    let unknown = fx.file(r"Other\new.mkv", b"a size nobody has");
    let report = fx.search(&unknown);
    assert_eq!(report.status, SearchStatus::Unknown);
    assert!(report.digests.is_empty(), "not read");
    assert_eq!(
        fx.store.verification_count().unwrap(),
        0,
        "a search records nothing"
    );
}

#[test]
fn empty_files_are_never_matched_by_their_content() {
    // All empty files have the same content: one is not "a copy" of
    // another, nor "moved" from where another one was.
    let mut fx = Fixture::new();
    let first = fx.file(r"A\empty.txt", b"");
    fx.create(&first);
    std::fs::remove_file(&first).unwrap();
    let other = fx.file(r"B\other.txt", b"");
    let report = fx.create(&other);
    assert_eq!(
        report.status,
        CreateStatus::Created,
        "not a decision to take"
    );
    assert_eq!(report.finding, None);
    let elsewhere = fx.file(r"C\third.txt", b"");
    assert_eq!(fx.search(&elsewhere).status, SearchStatus::Unknown);
}

#[test]
fn a_file_known_here_stays_known_here_among_many_copies() {
    // The database gives at most 20 items per content: the file's own
    // item must not depend on being among them.
    let mut fx = Fixture::new();
    let copies: Vec<PathBuf> = (0..25)
        .map(|i| fx.file(&format!(r"Copy {i}\same.bin"), b"one content, many copies"))
        .collect();
    for copy in &copies {
        assert_eq!(fx.create(copy).status, CreateStatus::Created);
    }
    // One copy gone: without the file's own item, this content would look
    // "probably moved" from there.
    std::fs::remove_file(&copies[0]).unwrap();
    let report = fx.search(&copies[24]);
    assert_eq!(report.status, SearchStatus::KnownHere);
    assert_eq!(
        report.finding.map(|finding| finding.outcome),
        Some(ContentOutcome::KnownHere)
    );
}
