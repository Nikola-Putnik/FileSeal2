//! Removing items from a database and putting them back.

use std::path::{Path, PathBuf};

use super::*;
use crate::algorithm::Algorithm;
use crate::ops::search::{self, SearchStatus};
use crate::ops::{
    CreateOptions, CreateStatus, DbTarget, VerifyOptions, VerifyStatus, create_reference,
    verify_file,
};
use crate::reader::Control;
use crate::store::{Listed, SharedStore};

struct Fixture {
    dir: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("items.fileseal"), "Items").unwrap();
        Fixture { dir, store }
    }

    /// A file referenced in the database and next to it; its item.
    fn referenced(&mut self, relative: &str, content: &[u8]) -> (PathBuf, i64) {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        let report = self.create(&path);
        assert_eq!(report.status, CreateStatus::Created);
        (path, report.item_id.unwrap())
    }

    fn create(&mut self, path: &Path) -> crate::ops::CreateReport {
        let operation_id = self.store.begin_operation(OperationKind::Create).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let options = CreateOptions {
            algorithms: vec![Algorithm::Sha256],
            write_sidecar: true,
            ..Default::default()
        };
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

    /// Against the database alone.
    fn verify(&mut self, path: &Path) -> VerifyStatus {
        let operation_id = self.store.begin_operation(OperationKind::Verify).unwrap();
        let shared = SharedStore::new(&mut self.store);
        let target = DbTarget {
            store: &shared,
            operation_id,
        };
        let options = VerifyOptions {
            use_sidecar: false,
            follow_moves: false,
        };
        verify_file(
            path,
            &options,
            Some(target),
            &Control::new(),
            &mut |_, _| {},
        )
        .status
    }

    fn search(&mut self, path: &Path) -> SearchStatus {
        let shared = SharedStore::new(&mut self.store);
        search::search_file(path, &shared, &Control::new(), &mut |_, _| {}).status
    }

    fn listed(&self, listed: Listed) -> u64 {
        self.store.browse("", 100, listed).unwrap().total
    }
}

#[test]
fn a_removed_item_is_no_longer_used_but_its_history_stays() {
    let mut fx = Fixture::new();
    let (film, item) = fx.referenced(r"Films\film.mkv", b"a film");
    let (kept, _) = fx.referenced(r"Films\kept.mkv", b"another film");
    // A copy elsewhere: its content was known through the removed item.
    let copy = fx.dir.path().join("copy.mkv");
    std::fs::copy(&film, &copy).unwrap();

    let removed = remove(&mut fx.store, &[item]).unwrap();
    assert_eq!(removed.changed, 1);
    assert_eq!(fx.store.info().unwrap().references, 1);
    assert_eq!(
        (fx.listed(Listed::Active), fx.listed(Listed::Retired)),
        (1, 1)
    );
    let row = &fx.store.browse("", 10, Listed::Retired).unwrap().rows[0];
    assert_eq!(row.path, film.display().to_string(), "where it was");
    assert!(row.retired_at.is_some());

    assert_eq!(fx.verify(&film), VerifyStatus::NoReference);
    assert_eq!(fx.verify(&kept), VerifyStatus::Match);
    assert_eq!(fx.search(&copy), SearchStatus::Unknown);
    // The file and its hash file are not touched.
    assert!(film.exists());
    assert!(film.with_extension("mkv.sha256").exists());

    let history = fx.store.item_history(item).unwrap();
    assert!(history.retired_at.is_some());
    assert!(!history.locations[0].current);
    assert_eq!(history.versions.len(), 1);
    // Removing it again changes nothing.
    assert_eq!(remove(&mut fx.store, &[item]).unwrap().changed, 0);
}

#[test]
fn a_removed_item_can_be_put_back() {
    let mut fx = Fixture::new();
    let (film, item) = fx.referenced("film.mkv", b"a film");
    remove(&mut fx.store, &[item]).unwrap();
    let back = put_back(&mut fx.store, &[item]).unwrap();
    assert_eq!((back.changed, back.taken.len()), (1, 0));
    assert_eq!(fx.verify(&film), VerifyStatus::Match);
    assert_eq!(fx.store.item_history(item).unwrap().retired_at, None);
    assert_eq!(fx.listed(Listed::Retired), 0);
}

#[test]
fn a_place_taken_since_keeps_the_item_removed() {
    let mut fx = Fixture::new();
    let (film, item) = fx.referenced("film.mkv", b"a film");
    remove(&mut fx.store, &[item]).unwrap();
    // Referenced again: a new item at the same place.
    std::fs::remove_file(film.with_extension("mkv.sha256")).unwrap();
    assert_eq!(fx.create(&film).status, CreateStatus::Created);
    let back = put_back(&mut fx.store, &[item]).unwrap();
    assert_eq!(back.changed, 0);
    assert_eq!(back.taken, [film.display().to_string()]);
    assert_eq!(fx.listed(Listed::Retired), 1);
    assert_eq!(
        fx.verify(&film),
        VerifyStatus::Match,
        "through the new item"
    );
}

#[test]
fn every_match_of_a_search_can_be_removed_not_only_those_listed() {
    let mut fx = Fixture::new();
    let mut season = Vec::new();
    for i in 0..5 {
        season.push(
            fx.referenced(
                &format!(r"Series\S01\e{i}.mkv"),
                format!("episode {i}").as_bytes(),
            )
            .1,
        );
    }
    fx.referenced(r"Films\film.mkv", b"a film");
    assert_eq!(
        fx.store
            .browse(r"series\s01", 2, Listed::Active)
            .unwrap()
            .rows
            .len(),
        2
    );
    let all = fx
        .store
        .matching_items(r"SERIES\S01", Listed::Active)
        .unwrap();
    assert_eq!(all.len(), 5);
    remove(&mut fx.store, &all).unwrap();
    assert_eq!(fx.listed(Listed::Active), 1);
    let back = fx.store.matching_items("s01", Listed::Retired).unwrap();
    assert_eq!(back.len(), 5);
    assert_eq!(put_back(&mut fx.store, &back).unwrap().changed, 5);
    assert_eq!(fx.listed(Listed::Active), 6);
}

#[test]
fn a_read_only_database_refuses_to_remove() {
    let mut fx = Fixture::new();
    let (_, item) = fx.referenced("film.mkv", b"a film");
    let path = fx.store.path().to_path_buf();
    drop(fx.store);
    let set_read_only = |read_only: bool| {
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(read_only);
        std::fs::set_permissions(&path, permissions).unwrap();
    };
    set_read_only(true);
    let refused = Store::open(&path).map(|mut store| remove(&mut store, &[item]));
    set_read_only(false);
    assert!(refused.unwrap().is_err());
    assert_eq!(Store::open(&path).unwrap().info().unwrap().references, 1);
}
