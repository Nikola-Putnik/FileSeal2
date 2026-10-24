//! Disks seen from a database: where they are now, and reassigning a root.
//! A drive letter change is simulated by recording files under a letter
//! that does not exist here, keeping their disk and place on it.

use std::path::{Path, PathBuf};

use super::*;
use crate::algorithm::Algorithm;
use crate::ops::{self, CreateOptions, DbTarget, VerifyOptions, VerifyStatus};
use crate::reader::Control;
use crate::store::SharedStore;

struct Fixture {
    dir: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("disks.fileseal"), "Disks").unwrap();
        Fixture { dir, store }
    }

    fn file(&self, relative: &str, content: &[u8]) -> PathBuf {
        let path = relative
            .split('/')
            .fold(self.dir.path().to_path_buf(), |p, part| p.join(part));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    fn create(&mut self, path: &Path) -> i64 {
        let operation_id = self.store.begin_operation(OperationKind::Create).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let report = ops::create_reference(
            path,
            &CreateOptions {
                algorithms: vec![Algorithm::Sha256],
                write_sidecar: false,
                new_item: false,
                ..Default::default()
            },
            Some(DbTarget {
                store: &shared,
                operation_id,
            }),
            &Control::new(),
            &mut |_, _| {},
        );
        report.item_id.unwrap()
    }

    fn verify(&mut self, path: &Path) -> VerifyStatus {
        let operation_id = self.store.begin_operation(OperationKind::Verify).unwrap();
        let shared = SharedStore::new(&mut self.store);
        ops::verify_file(
            path,
            &VerifyOptions {
                use_sidecar: false,
                follow_moves: false,
            },
            Some(DbTarget {
                store: &shared,
                operation_id,
            }),
            &Control::new(),
            &mut |_, _| {},
        )
        .status
    }
}

/// A drive letter with nothing behind it on this computer.
fn absent_letter() -> Option<char> {
    ('M'..='Z').find(|l| !Path::new(&format!("{l}:\\")).exists())
}

/// The same path under another drive letter.
fn under(letter: char, path: &Path) -> String {
    let text = path.display().to_string();
    format!("{letter}:{}", &text[2..])
}

#[test]
fn a_disk_where_it_was_recorded_is_connected() {
    let mut fx = Fixture::new();
    for name in ["A/one.bin", "A/two.bin", "B/three.bin"] {
        let path = fx.file(name, name.as_bytes());
        fx.create(&path);
    }
    let disks = volumes(&fx.store).unwrap();
    assert_eq!(disks.len(), 1, "{disks:?}");
    assert_eq!(disks[0].files, 3);
    assert_eq!(disks[0].state, VolumeState::Connected);
    assert!(disks[0].serial.is_some());
    let root = platform::volume_root(fx.dir.path()).unwrap();
    assert_eq!(disks[0].recorded_root, root.display().to_string());
}

#[cfg(windows)]
#[test]
fn a_disk_back_under_another_letter_is_found_then_updated() {
    let mut fx = Fixture::new();
    let Some(letter) = absent_letter() else {
        return;
    };
    let files: Vec<PathBuf> = ["Films/a.mkv", "Films/b.mkv"]
        .iter()
        .map(|name| fx.file(name, name.as_bytes()))
        .collect();
    let items: Vec<i64> = files.iter().map(|f| fx.create(f)).collect();
    // Recorded when the disk was mounted as another letter.
    for (item, file) in items.iter().zip(&files) {
        fx.store.set_location_for_test(*item, &under(letter, file));
    }
    let disks = volumes(&fx.store).unwrap();
    assert_eq!(disks.len(), 1, "{disks:?}");
    let disk = &disks[0];
    assert_eq!(disk.recorded_root, format!("{letter}:\\"));
    let now = platform::volume_root(fx.dir.path())
        .unwrap()
        .display()
        .to_string();
    assert_eq!(disk.state, VolumeState::MovedTo { root: now.clone() });

    // The one decision: the disk's locations are updated, nothing is read.
    let request = Reassign {
        from: disk.recorded_root.clone(),
        to: now,
        serial: disk.serial.clone(),
        include_missing: false,
    };
    let planned = preview(&fx.store, &request).unwrap();
    assert_eq!((planned.files, planned.found, planned.missing), (2, 2, 0));
    assert_eq!(planned.examples[0].to, files[0].display().to_string());
    let report = reassign(&mut fx.store, &request).unwrap();
    assert_eq!((report.moved, report.left), (2, 0));
    assert_eq!(volumes(&fx.store).unwrap()[0].state, VolumeState::Connected);
    // Found again by path (R1), the former location kept as history.
    assert_eq!(fx.verify(&files[0]), VerifyStatus::Match);
    let history = fx.store.item_history(items[0]).unwrap();
    assert_eq!(history.locations.len(), 2);
    assert!(history.locations[0].current);
    assert_eq!(history.locations[1].path, under(letter, &files[0]));
}

#[cfg(windows)]
#[test]
fn a_disk_found_nowhere_is_not_connected() {
    let mut fx = Fixture::new();
    let Some(letter) = absent_letter() else {
        return;
    };
    let file = fx.file("Backup/a.bin", b"on a disk left in a drawer");
    let item = fx.create(&file);
    fx.store.set_location_for_test(item, &under(letter, &file));
    // Its place exists on no disk present.
    std::fs::remove_file(&file).unwrap();
    let disks = volumes(&fx.store).unwrap();
    assert_eq!(disks[0].state, VolumeState::NotConnected);
}

#[test]
fn a_folder_moved_elsewhere_is_reassigned() {
    let mut fx = Fixture::new();
    let kept = ["one.bin", "sub/two.bin", "three.bin"];
    let mut items = Vec::new();
    for name in kept {
        let path = fx.file(&format!("Old/{name}"), name.as_bytes());
        items.push(fx.create(&path));
    }
    let gone = fx.file("Old/gone.bin", b"deleted before the move");
    fx.create(&gone);
    std::fs::remove_file(&gone).unwrap();
    // A file already recorded where one of them goes: left alone.
    let busy = fx.file("New/three.bin", b"three.bin");
    fx.create(&busy);
    std::fs::rename(fx.dir.path().join("Old"), fx.dir.path().join("Moved")).unwrap();
    std::fs::create_dir_all(fx.dir.path().join("New")).unwrap();
    for name in ["one.bin", "sub/two.bin"] {
        let from = fx.dir.path().join("Moved").join(name);
        let to = fx.dir.path().join("New").join(name);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::rename(from, to).unwrap();
    }
    let request = Reassign {
        from: fx.dir.path().join("Old").display().to_string(),
        to: fx.dir.path().join("New").display().to_string(),
        serial: None,
        include_missing: false,
    };
    let planned = preview(&fx.store, &request).unwrap();
    assert_eq!((planned.files, planned.found, planned.missing), (4, 3, 1));
    let report = reassign(&mut fx.store, &request).unwrap();
    assert_eq!((report.moved, report.left), (2, 1));
    assert_eq!(report.taken.len(), 1, "New/three.bin is another item's");
    let moved = fx.dir.path().join("New").join("sub").join("two.bin");
    assert_eq!(fx.verify(&moved), VerifyStatus::Match);

    let same = Reassign {
        to: request.from.clone(),
        ..request
    };
    assert_eq!(
        preview(&fx.store, &same).unwrap_err().kind,
        ProblemKind::InvalidName
    );
}
