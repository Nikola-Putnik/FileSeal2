//! Batches on real files. Several "volumes" are simulated by giving each
//! top folder of the test directory its own volume name; disk loss, read
//! errors, a full database and a slow disk are injected (`faults`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::*;
use crate::algorithm::Algorithm;
use crate::faults;

const MIB: usize = 1 << 20;

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

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Writes the files (`"A/one.bin"`…) with distinct contents of `size`
    /// bytes; returns them in that order.
    fn files(&self, names: &[&str], size: usize) -> Vec<BatchFile> {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let path = self.path(name);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                let mut content = vec![i as u8; size];
                content.extend_from_slice(name.as_bytes());
                std::fs::write(&path, content).unwrap();
                BatchFile::new(path.to_string_lossy())
            })
            .collect()
    }

    /// `"A/one.bin"` under the test folder, with the system's separators.
    fn path(&self, name: &str) -> PathBuf {
        name.split('/')
            .fold(self.root().to_path_buf(), |path, part| path.join(part))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        faults::clear_under(self.dir.path());
    }
}

fn create(sidecar: bool) -> BatchOptions {
    BatchOptions {
        action: BatchAction::Create(CreateOptions {
            algorithms: vec![Algorithm::Sha256],
            write_sidecar: sidecar,
            new_item: false,
            ..Default::default()
        }),
        parallel_volumes: 2,
    }
}

fn verify() -> BatchOptions {
    BatchOptions {
        action: BatchAction::Verify(VerifyOptions {
            use_sidecar: true,
            follow_moves: false,
        }),
        parallel_volumes: 2,
    }
}

struct Outcome {
    /// By position in the list.
    entries: Vec<BatchEntry>,
    updates: Vec<BatchUpdate>,
    summary: BatchSummary,
}

impl Outcome {
    fn statuses(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| e.outcome.status_code())
            .collect()
    }

    fn skip(&self, index: usize) -> &Skip {
        match &self.entries[index].outcome {
            EntryOutcome::Skipped(skip) => skip,
            other => panic!("not skipped: {other:?}"),
        }
    }
}

/// Runs a batch with one volume per top folder of `root`, and checks what
/// must hold for every batch: one entry per file, counts that add up, the
/// summary in the last update and nothing after it.
fn run_batch(
    root: &Path,
    files: &[BatchFile],
    options: &BatchOptions,
    store: Option<&mut Store>,
    control: &Control,
    on_update: &mut dyn FnMut(&BatchUpdate),
) -> Outcome {
    let root = root.to_path_buf();
    let volume = move |path: &Path| {
        path.strip_prefix(&root)
            .ok()
            .and_then(|p| p.components().next())
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut updates: Vec<BatchUpdate> = Vec::new();
    let summary = run_on_volumes(
        files,
        options,
        store,
        control,
        &mut |update| {
            assert!(
                updates.last().is_none_or(|u| u.summary.is_none()),
                "nothing after the last update"
            );
            on_update(&update);
            updates.push(update);
        },
        &volume,
    )
    .unwrap();
    let mut entries: Vec<BatchEntry> = updates
        .iter()
        .flat_map(|u| u.entries.iter().cloned())
        .collect();
    entries.sort_by_key(|e| e.index);
    let indices: HashSet<u32> = entries.iter().map(|e| e.index).collect();
    assert_eq!(entries.len(), files.len(), "one entry per file");
    assert_eq!(indices.len(), files.len(), "each file once");
    assert_eq!(updates.last().unwrap().summary.as_ref(), Some(&summary));
    assert_eq!(summary.total as usize, files.len());
    assert_eq!(
        summary.processed
            + summary.failed
            + summary.cancelled
            + summary.not_processed
            + summary.duplicates,
        summary.total,
        "every file counted once"
    );
    let by_status: u32 = summary.statuses.iter().map(|s| s.count).sum();
    assert_eq!(by_status, summary.total);
    let last = &updates.last().unwrap().progress;
    assert_eq!(last.files_done, summary.total);
    assert_eq!(last.bytes_done, last.bytes_total);
    assert!(last.current.is_empty());
    Outcome {
        entries,
        updates,
        summary,
    }
}

fn quiet(_: &BatchUpdate) {}

#[test]
fn create_then_verify_then_search_a_list_on_two_volumes() {
    let mut fx = Fixture::new();
    let files = fx.files(
        &["A/a1.bin", "A/a2.bin", "B/b1.bin", "A/a3.bin", "B/b2.bin"],
        1000,
    );
    let root = fx.root().to_path_buf();
    let control = Control::new();

    let created = run_batch(
        &root,
        &files,
        &create(true),
        Some(&mut fx.store),
        &control,
        &mut quiet,
    );
    assert_eq!(created.statuses(), ["created"; 5]);
    assert_eq!(created.summary.processed, 5);
    assert_eq!(fx.store.info().unwrap().references, 5);
    // Commits wait for the disk again once the batch is over.
    assert_eq!(fx.store.synchronous(), 2, "FULL");

    let verified = run_batch(
        &root,
        &files,
        &verify(),
        Some(&mut fx.store),
        &control,
        &mut quiet,
    );
    assert_eq!(verified.statuses(), ["match"; 5]);
    assert_eq!(fx.store.verification_count().unwrap(), 5, "history kept");
    assert_eq!(
        verified.summary.statuses,
        [StatusCount {
            status: "match".into(),
            count: 5
        }]
    );

    let search = BatchOptions {
        action: BatchAction::Search,
        parallel_volumes: 2,
    };
    let searched = run_batch(
        &root,
        &files,
        &search,
        Some(&mut fx.store),
        &control,
        &mut quiet,
    );
    assert_eq!(searched.statuses(), ["knownHere"; 5]);
    // Positions and paths are those of the list.
    for (entry, file) in searched.entries.iter().zip(&files) {
        assert_eq!(entry.path, file.path);
    }
}

#[test]
fn every_file_ends_with_exactly_one_entry() {
    let fx = Fixture::new();
    let mut files = fx.files(&["A/ok.bin", "A/also ok.bin"], 100);
    files.push(BatchFile::new(fx.path("A/missing.bin").to_string_lossy()));
    files.push(BatchFile::new(fx.path("A").to_string_lossy()));
    files.push(files[0].clone());
    let outcome = run_batch(
        fx.root(),
        &files,
        &create(true),
        None,
        &Control::new(),
        &mut quiet,
    );
    assert_eq!(
        outcome.statuses(),
        ["created", "created", "failed", "failed", "duplicate"]
    );
    let summary = &outcome.summary;
    assert_eq!(
        (summary.processed, summary.failed, summary.duplicates),
        (2, 2, 1)
    );
    assert_eq!(outcome.skip(4).same_as.as_ref().unwrap().index, 0);
    assert!(!summary.stopped_by_user);
}

#[test]
fn hard_links_are_read_once() {
    let mut fx = Fixture::new();
    let mut files = fx.files(&["A/a.bin", "A/c.bin"], 100);
    std::fs::hard_link(fx.path("A/a.bin"), fx.path("A/b.bin")).unwrap();
    files.insert(1, BatchFile::new(fx.path("A/b.bin").to_string_lossy()));
    let root = fx.root().to_path_buf();
    let outcome = run_batch(
        &root,
        &files,
        &create(false),
        Some(&mut fx.store),
        &Control::new(),
        &mut quiet,
    );
    assert_eq!(outcome.statuses(), ["created", "duplicate", "created"]);
    let same = outcome.skip(1).same_as.clone().unwrap();
    assert_eq!((same.index, same.path), (0, files[0].path.clone()));
    assert_eq!(
        fx.store.info().unwrap().references,
        2,
        "one item per physical file"
    );
}

#[test]
fn one_reader_per_volume_and_volumes_in_parallel() {
    let fx = Fixture::new();
    let files = fx.files(&["A/a1.bin", "A/a2.bin", "B/b1.bin", "B/b2.bin"], 2 * MIB);
    let volume_of = |index: u32| if index < 2 { 'A' } else { 'B' };

    // Paused from the start: each reader takes its first file and waits.
    let control = Control::new();
    control.pause();
    let mut seen_paused: Option<Vec<u32>> = None;
    let mut most_at_once = 0;
    let outcome = run_batch(
        fx.root(),
        &files,
        &verify(),
        None,
        &control,
        &mut |update| {
            let current: Vec<u32> = update.progress.current.iter().map(|c| c.index).collect();
            let volumes: HashSet<char> = current.iter().map(|&i| volume_of(i)).collect();
            assert_eq!(
                volumes.len(),
                current.len(),
                "one reader per volume: {current:?}"
            );
            most_at_once = most_at_once.max(current.len());
            if update.progress.paused && current.len() == 2 {
                seen_paused = Some(current);
                control.resume();
            }
        },
    );
    assert_eq!(seen_paused, Some(vec![0, 2]), "first file of each volume");
    assert_eq!(most_at_once, 2);
    assert_eq!(outcome.summary.processed, 4);

    // One volume at a time: files start in list order.
    let options = BatchOptions {
        parallel_volumes: 1,
        ..verify()
    };
    let mut order = Vec::new();
    run_batch(
        fx.root(),
        &files,
        &options,
        None,
        &Control::new(),
        &mut |update| {
            assert!(update.progress.current.len() <= 1);
            order.extend(update.entries.iter().map(|e| e.index));
        },
    );
    assert_eq!(order, [0, 1, 2, 3]);
}

#[test]
fn cancelling_keeps_what_is_done_and_leaves_the_rest_not_processed() {
    let mut fx = Fixture::new();
    let names: Vec<String> = (0..8).map(|i| format!("A/{i}.bin")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    // 3 blocks of 20 ms each: about 60 ms per file.
    let files = fx.files(&names, 2 * MIB + 10);
    faults::arm_under(fx.root(), FaultPoint::SlowRead, None);
    let control = Control::new();
    let root = fx.root().to_path_buf();
    let outcome = run_batch(
        &root,
        &files,
        &create(true),
        Some(&mut fx.store),
        &control,
        &mut |update| {
            if !update.entries.is_empty() {
                control.cancel();
            }
        },
    );
    let summary = &outcome.summary;
    assert!(summary.stopped_by_user);
    assert!(summary.processed >= 1, "{summary:?}");
    assert!(summary.cancelled <= 1, "only the file being read");
    assert!(summary.not_processed >= 4, "{summary:?}");
    // Nothing half done: a reference for each file created, and only them.
    assert_eq!(
        fx.store.info().unwrap().references,
        u64::from(summary.processed)
    );
    for (entry, file) in outcome.entries.iter().zip(&files) {
        let sidecar = Path::new(&file.path).with_extension("bin.sha256");
        assert_eq!(
            sidecar.exists(),
            entry.outcome.status_code() == "created",
            "{}",
            entry.path
        );
        if entry.outcome.category() == Category::NotProcessed {
            assert_eq!(
                entry.outcome,
                EntryOutcome::Skipped(skip(SkipReason::Cancelled, None, None))
            );
        }
    }
}

#[test]
fn cancelling_while_paused_starts_nothing() {
    let fx = Fixture::new();
    let files = fx.files(&["A/1.bin", "A/2.bin", "B/3.bin"], 100);
    let control = Control::new();
    control.pause();
    let outcome = run_batch(
        fx.root(),
        &files,
        &verify(),
        None,
        &control,
        &mut |update| {
            if update.progress.paused {
                control.cancel();
            }
        },
    );
    assert_eq!(outcome.summary.not_processed, 3);
    assert!(outcome.summary.stopped_by_user);
}

#[test]
fn a_read_error_does_not_stop_the_batch() {
    let fx = Fixture::new();
    let files = fx.files(&["A/1.bin", "A/bad.bin", "A/3.bin"], 100);
    run_batch(
        fx.root(),
        &files,
        &create(true),
        None,
        &Control::new(),
        &mut quiet,
    );
    // ERROR_CRC: a bad sector.
    faults::arm_under(&fx.path("A/bad.bin"), FaultPoint::Read, Some(23));
    let outcome = run_batch(
        fx.root(),
        &files,
        &verify(),
        None,
        &Control::new(),
        &mut quiet,
    );
    assert_eq!(outcome.statuses(), ["match", "readError", "match"]);
    assert!(
        outcome.summary.lost_volumes.is_empty(),
        "the disk is still there"
    );
}

#[test]
fn a_lost_volume_leaves_its_files_not_processed_and_the_others_go_on() {
    let fx = Fixture::new();
    let files = fx.files(
        &[
            "A/a1.bin", "A/a2.bin", "B/b1.bin", "A/a3.bin", "B/b2.bin", "A/a4.bin",
        ],
        100,
    );
    run_batch(
        fx.root(),
        &files,
        &create(true),
        None,
        &Control::new(),
        &mut quiet,
    );
    // A is unplugged while a2 is read: ERROR_NOT_READY, and A is gone.
    faults::arm_under(&fx.path("A/a2.bin"), FaultPoint::Read, Some(21));
    faults::arm_under(&fx.path("A"), FaultPoint::VolumeGone, None);
    let outcome = run_batch(
        fx.root(),
        &files,
        &verify(),
        None,
        &Control::new(),
        &mut quiet,
    );
    assert_eq!(
        outcome.statuses(),
        [
            "match",
            "readError",
            "match",
            "notProcessed",
            "match",
            "notProcessed"
        ]
    );
    assert_eq!(outcome.skip(3).reason, SkipReason::VolumeUnavailable);
    assert_eq!(outcome.summary.lost_volumes, ["A"]);
    assert!(outcome.summary.database_problem.is_none());
}

#[test]
fn a_lost_database_stops_the_batch() {
    let mut fx = Fixture::new();
    let files = fx.files(&["A/1.bin", "A/2.bin", "A/3.bin", "B/4.bin"], 100);
    // ERROR_DISK_FULL on the database's disk.
    let db = fx.store.path().to_path_buf();
    faults::arm_under(&db, FaultPoint::DatabaseWrite, Some(112));
    let options = BatchOptions {
        parallel_volumes: 1,
        ..create(true)
    };
    let root = fx.root().to_path_buf();
    let outcome = run_batch(
        &root,
        &files,
        &options,
        Some(&mut fx.store),
        &Control::new(),
        &mut quiet,
    );
    // The hash file of the first one was written: a partial success.
    assert_eq!(
        outcome.statuses(),
        [
            "partialSuccess",
            "notProcessed",
            "notProcessed",
            "notProcessed"
        ]
    );
    let problem = outcome.summary.database_problem.clone().unwrap();
    assert_eq!(problem.kind, ProblemKind::DiskFull);
    let skip = outcome.skip(1);
    assert_eq!(skip.reason, SkipReason::DatabaseUnavailable);
    assert_eq!(
        skip.problem.as_ref().map(|p| p.kind),
        Some(ProblemKind::DiskFull)
    );
    assert!(
        !fx.path("A/2.bin.sha256").exists(),
        "nothing done after the stop"
    );
    // Shown in the report with the reason.
    let line = crate::report::to_csv(&[report_entry(&options.action, &outcome.entries[1])], ',');
    assert!(line.contains("create,"), "{line}");
    assert!(line.contains("notProcessed"), "{line}");
    assert!(line.contains("databaseUnavailable"), "{line}");
}

#[test]
fn a_read_only_database_verifies_without_history_and_does_not_stop() {
    let mut fx = Fixture::new();
    let files = fx.files(&["A/1.bin", "A/2.bin", "A/3.bin"], 100);
    let root = fx.root().to_path_buf();
    run_batch(
        &root,
        &files,
        &create(false),
        Some(&mut fx.store),
        &Control::new(),
        &mut quiet,
    );
    let path = fx.store.path().to_path_buf();
    // Reopened from a write-protected file.
    fx.store = Store::create(&root.join("other.fileseal"), "Other").unwrap();
    let set_read_only = |read_only: bool| {
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(read_only);
        std::fs::set_permissions(&path, permissions).unwrap();
    };
    set_read_only(true);
    let outcome = Store::open(&path).map(|mut store| {
        assert!(store.is_read_only());
        run_batch(
            &root,
            &files,
            &verify(),
            Some(&mut store),
            &Control::new(),
            &mut quiet,
        )
    });
    set_read_only(false);
    let outcome = outcome.unwrap();
    assert_eq!(outcome.statuses(), ["match"; 3]);
    assert!(outcome.summary.database_problem.is_none());
    for entry in &outcome.entries {
        let EntryOutcome::Verified(report) = &entry.outcome else {
            panic!("verified")
        };
        assert_eq!(
            report.record_problem.as_ref().map(|p| p.kind),
            Some(ProblemKind::DatabaseReadOnly)
        );
    }
}

#[test]
fn files_listed_in_a_hash_file_are_verified_against_their_line_only() {
    let fx = Fixture::new();
    let files = fx.files(&["A/one.bin", "A/two.bin"], 100);
    // Correct hash files next to both: they must not be consulted.
    run_batch(
        fx.root(),
        &files,
        &create(true),
        None,
        &Control::new(),
        &mut quiet,
    );
    let one = crate::reader::hash_file(
        &fx.path("A/one.bin"),
        &[Algorithm::Sha256],
        &Control::new(),
        &mut |_, _| {},
    )
    .unwrap()
    .digests
    .remove(0)
    .hex;
    let zeros = "0".repeat(64);
    std::fs::write(
        fx.path("A/SHA256SUMS"),
        format!(
            "{one}  one.bin
{zeros}  two.bin
not a line
{one}  gone.bin
"
        ),
    )
    .unwrap();
    let listed = crate::ops::manifest::list(&fx.path("A/SHA256SUMS")).unwrap();
    assert_eq!(listed.algorithm, Some(Algorithm::Sha256));
    assert_eq!(listed.files.len(), 3);
    assert_eq!(listed.problems.len(), 1);
    assert_eq!(listed.problems[0].line, 3);
    let outcome = run_batch(
        fx.root(),
        &listed.files,
        &verify(),
        None,
        &Control::new(),
        &mut quiet,
    );
    assert_eq!(outcome.statuses(), ["match", "mismatch", "notFound"]);
    let EntryOutcome::Verified(report) = &outcome.entries[1].outcome else {
        panic!("verified")
    };
    assert_eq!(
        report.sources.len(),
        1,
        "the line only, not the hash file next to it"
    );
    assert_eq!(report.sources[0].path, listed.path);
}

#[test]
fn progress_counts_bytes_and_measures_nothing_while_paused() {
    let fx = Fixture::new();
    let files = fx.files(&["A/1.bin", "A/2.bin"], MIB);
    let total: u64 = files
        .iter()
        .map(|f| std::fs::metadata(&f.path).unwrap().len())
        .sum();
    let outcome = run_batch(
        fx.root(),
        &files,
        &verify(),
        None,
        &Control::new(),
        &mut quiet,
    );
    let last = &outcome.updates.last().unwrap().progress;
    assert_eq!((last.bytes_total, last.files_total), (total, 2));
    // Done in well under 5 s: no remaining time was ever made up.
    assert!(
        outcome
            .updates
            .iter()
            .all(|u| u.progress.remaining_ms.is_none())
    );
    // Sizes given by the selection are used as they are.
    let given: Vec<BatchFile> = files
        .iter()
        .map(|f| BatchFile {
            path: f.path.clone(),
            size: Some(10),
            listed: None,
        })
        .collect();
    let outcome = run_batch(
        fx.root(),
        &given,
        &verify(),
        None,
        &Control::new(),
        &mut quiet,
    );
    assert_eq!(outcome.updates.last().unwrap().progress.bytes_total, 20);
}

#[test]
fn impossible_batches_are_refused_before_starting() {
    let mut fx = Fixture::new();
    let files = fx.files(&["A/1.bin"], 10);
    let refused = |options: &BatchOptions, store: Option<&mut Store>| {
        run(&files, options, store, &Control::new(), &mut |_| {
            panic!("nothing is sent")
        })
        .unwrap_err()
        .kind
    };
    let search = BatchOptions {
        action: BatchAction::Search,
        parallel_volumes: 2,
    };
    assert_eq!(refused(&search, None), ProblemKind::Io);
    assert_eq!(refused(&create(false), None), ProblemKind::Io);
    let no_algorithm = BatchOptions {
        action: BatchAction::Create(CreateOptions {
            algorithms: Vec::new(),
            write_sidecar: true,
            new_item: false,
            ..Default::default()
        }),
        parallel_volumes: 2,
    };
    assert_eq!(refused(&no_algorithm, Some(&mut fx.store)), ProblemKind::Io);
}

#[test]
fn an_empty_list_ends_at_once() {
    let outcome = run_batch(
        Path::new("."),
        &[],
        &verify(),
        None,
        &Control::new(),
        &mut quiet,
    );
    assert_eq!(outcome.summary.total, 0);
    assert_eq!(outcome.updates.len(), 1);
}
