//! Discovery on real folders made for each test: order, exclusions,
//! links, duplicates, awkward names, cancellation.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::*;
use crate::reader;

struct Run {
    files: Vec<FoundFile>,
    exclusions: Vec<Exclusion>,
    errors: Vec<TraversalError>,
    summary: DiscoverySummary,
    batches: usize,
}

impl Run {
    fn paths(&self) -> Vec<String> {
        self.files.iter().map(|f| f.path.clone()).collect()
    }

    fn names(&self, root: &Path) -> Vec<String> {
        self.files.iter().map(|f| relative(root, &f.path)).collect()
    }

    fn excluded(&self, root: &Path) -> Vec<(String, ExclusionReason, bool)> {
        self.exclusions
            .iter()
            .map(|e| (relative(root, &e.path), e.reason, e.folder))
            .collect()
    }

    fn reason_of(&self, root: &Path, name: &str) -> Option<ExclusionReason> {
        self.exclusions
            .iter()
            .find(|e| relative(root, &e.path) == name)
            .map(|e| e.reason)
    }
}

fn relative(root: &Path, path: &str) -> String {
    Path::new(path)
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_owned())
}

fn run_with(roots: &[PathBuf], options: &DiscoveryOptions, control: &Control) -> Run {
    let mut run = Run {
        files: Vec::new(),
        exclusions: Vec::new(),
        errors: Vec::new(),
        summary: DiscoverySummary {
            files: 0,
            bytes: 0,
            folders_scanned: 0,
            excluded: Vec::new(),
            filtered_out: 0,
            errors: 0,
            extensions: Vec::new(),
            cancelled: false,
            elapsed_ms: 0,
        },
        batches: 0,
    };
    let (mut files, mut exclusions, mut errors, mut batches) = (vec![], vec![], vec![], 0);
    let mut last = DiscoveryProgress::default();
    let mut final_summary = None;
    run.summary = discover(roots, options, control, &mut |batch| {
        assert!(final_summary.is_none(), "nothing after the last batch");
        final_summary = batch.summary;
        batches += 1;
        files.extend(batch.files);
        exclusions.extend(batch.exclusions);
        errors.extend(batch.errors);
        last = batch.progress;
    });
    run.files = files;
    run.exclusions = exclusions;
    run.errors = errors;
    run.batches = batches;
    assert_eq!(final_summary.as_ref(), Some(&run.summary));
    // The totals carried by the batches add up to the summary.
    assert_eq!(last.files as usize, run.files.len());
    assert_eq!(run.summary.files as usize, run.files.len());
    assert_eq!(last.excluded as usize, run.exclusions.len());
    assert_eq!(run.summary.errors as usize, run.errors.len());
    let excluded: u32 = run.summary.excluded.iter().map(|c| c.count).sum();
    assert_eq!(excluded as usize, run.exclusions.len());
    run
}

fn run(roots: &[PathBuf], options: &DiscoveryOptions) -> Run {
    run_with(roots, options, &Control::new())
}

fn write(path: &Path, content: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn tree(files: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in files {
        write(&dir.path().join(name), name.as_bytes());
    }
    dir
}

#[cfg(windows)]
fn attrib(flag: &str, path: &Path) {
    let status = Command::new("attrib").arg(flag).arg(path).status().unwrap();
    assert!(status.success(), "attrib {flag} {}", path.display());
}

#[test]
fn files_come_before_subfolders_in_explorer_order() {
    let dir = tree(&[
        "Episode 10.mkv",
        "episode 2.mkv",
        "B/inner.txt",
        "a/z.txt",
        "a/sub/deep.txt",
        "Zed.txt",
    ]);
    let found = run(&[dir.path().to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(
        found.names(dir.path()),
        [
            "episode 2.mkv",
            "Episode 10.mkv",
            "Zed.txt",
            "a/z.txt",
            "a/sub/deep.txt",
            "B/inner.txt",
        ]
    );
    assert!(found.files.iter().all(|f| !f.direct));
    assert_eq!(found.summary.folders_scanned, 4);
    assert_eq!(
        found.summary.bytes,
        found.files.iter().map(|f| f.size).sum::<u64>()
    );
    assert!(!found.summary.cancelled);
}

#[test]
fn natural_order_compares_numbers_as_numbers() {
    let mut names = vec!["b", "A10", "a2", "a02", "A1", "a", "x.9", "x.10"];
    names.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(names, ["a", "A1", "a02", "a2", "A10", "b", "x.9", "x.10"]);
}

#[test]
fn a_flat_selection_ignores_subfolders() {
    let dir = tree(&["top.txt", "sub/inner.txt"]);
    let options = DiscoveryOptions {
        recursive: false,
        ..DiscoveryOptions::default()
    };
    let found = run(&[dir.path().to_path_buf()], &options);
    assert_eq!(found.names(dir.path()), ["top.txt"]);
    assert_eq!(found.summary.folders_scanned, 1);
    assert!(found.exclusions.is_empty(), "not an exclusion: not asked");
}

#[test]
fn extensions_are_split_as_documented() {
    assert_eq!(extension_of("film.MKV"), "mkv");
    assert_eq!(extension_of("archive.tar.gz"), "gz");
    assert_eq!(extension_of(".bashrc"), "");
    assert_eq!(extension_of("README"), "");
    assert_eq!(extension_of("name."), "");
    assert_eq!(extension_of("ÉTÉ.ÀVI"), "àvi");
}

#[test]
fn the_extension_filter_ignores_case_and_offers_no_extension() {
    let dir = tree(&["a.mkv", "B.MKV", "c.Avi", "README", "d.txt", "e.srt"]);
    let options = DiscoveryOptions {
        extensions: vec!["MKV".into(), ".avi".into(), String::new()],
        ..DiscoveryOptions::default()
    };
    let found = run(&[dir.path().to_path_buf()], &options);
    assert_eq!(
        found.names(dir.path()),
        ["a.mkv", "B.MKV", "c.Avi", "README"]
    );
    assert_eq!(found.summary.filtered_out, 2);
    // Counted before the filter, so that each one can be offered.
    let counts: Vec<(&str, u32)> = found
        .summary
        .extensions
        .iter()
        .map(|c| (c.extension.as_str(), c.files))
        .collect();
    assert_eq!(
        counts,
        [("mkv", 2), ("", 1), ("avi", 1), ("srt", 1), ("txt", 1)]
    );
    assert_eq!(found.summary.extensions[0].bytes, 10, "a.mkv + B.MKV");
}

#[test]
fn hash_files_and_fileseal_files_are_left_out_of_folders() {
    let dir = tree(&[
        "film.mkv",
        "film.mkv.sha256",
        "film.mkv.MD5",
        "old.sfv",
        "SHA256SUMS",
        "md5sums.txt",
        "notes.txt",
        "films.fileseal",
        "films.fileseal-wal",
        "films.before-v2-20261022-101500.fileseal",
        "films.before-v2-20261022-101500.fileseal.partial",
        ".fileseal-a1b2c3.tmp",
    ]);
    let found = run(&[dir.path().to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(found.names(dir.path()), ["film.mkv", "notes.txt"]);
    let root = dir.path();
    for name in [
        "film.mkv.sha256",
        "film.mkv.MD5",
        "old.sfv",
        "SHA256SUMS",
        "md5sums.txt",
    ] {
        assert_eq!(
            found.reason_of(root, name),
            Some(ExclusionReason::HashFile),
            "{name}"
        );
    }
    for name in [
        "films.fileseal",
        "films.fileseal-wal",
        "films.before-v2-20261022-101500.fileseal",
        "films.before-v2-20261022-101500.fileseal.partial",
    ] {
        assert_eq!(
            found.reason_of(root, name),
            Some(ExclusionReason::FileSealFile),
            "{name}"
        );
    }
    assert_eq!(
        found.reason_of(root, ".fileseal-a1b2c3.tmp"),
        Some(ExclusionReason::TemporaryFile)
    );
    let counts: Vec<(ExclusionReason, u32)> = found
        .summary
        .excluded
        .iter()
        .map(|c| (c.reason, c.count))
        .collect();
    assert_eq!(
        counts,
        [
            (ExclusionReason::HashFile, 5),
            (ExclusionReason::FileSealFile, 4),
            (ExclusionReason::TemporaryFile, 1),
        ]
    );
}

#[test]
fn a_file_chosen_directly_is_always_kept() {
    let dir = tree(&["film.mkv.sha256", "base.fileseal", "x.txt"]);
    let roots = [
        dir.path().join("film.mkv.sha256"),
        dir.path().join("base.fileseal"),
    ];
    let options = DiscoveryOptions {
        extensions: vec!["mkv".into()],
        ..DiscoveryOptions::default()
    };
    let found = run(&roots, &options);
    assert_eq!(
        found.names(dir.path()),
        ["film.mkv.sha256", "base.fileseal"]
    );
    assert!(found.files.iter().all(|f| f.direct));
    assert_eq!(found.files[0].extension, "sha256");
    assert!(found.exclusions.is_empty());
}

#[test]
fn files_reached_twice_are_kept_once() {
    let dir = tree(&["a.txt", "sub/b.txt", "sub/c.txt"]);
    let root = dir.path();
    // Same file directly and through its folder, a folder inside another
    // already walked, the same folder with another case.
    let upper = PathBuf::from(root.to_string_lossy().to_uppercase());
    let roots = [
        root.join("sub").join("b.txt"),
        root.to_path_buf(),
        root.join("sub"),
        if cfg!(windows) {
            upper
        } else {
            root.to_path_buf()
        },
    ];
    let found = run(&roots, &DiscoveryOptions::default());
    assert_eq!(found.names(root), ["sub/b.txt", "a.txt", "sub/c.txt"]);
    assert_eq!(
        found
            .excluded(root)
            .into_iter()
            .map(|(_, reason, folder)| (reason, folder))
            .collect::<Vec<_>>(),
        [
            (ExclusionReason::Duplicate, false),
            (ExclusionReason::Duplicate, true),
            (ExclusionReason::Duplicate, true),
        ]
    );
    assert_eq!(
        found.reason_of(root, "sub/b.txt"),
        Some(ExclusionReason::Duplicate)
    );
    assert_eq!(
        found.summary.extensions[0].files, 3,
        "duplicates not counted"
    );
}

#[test]
fn a_file_left_out_by_the_filter_is_kept_when_chosen_directly() {
    let dir = tree(&["film.mkv", "readme.txt"]);
    let roots = [dir.path().to_path_buf(), dir.path().join("readme.txt")];
    let options = DiscoveryOptions {
        extensions: vec!["mkv".into()],
        ..DiscoveryOptions::default()
    };
    let found = run(&roots, &options);
    assert_eq!(found.names(dir.path()), ["film.mkv", "readme.txt"]);
    assert!(found.files[1].direct);
    assert!(found.exclusions.is_empty(), "not a duplicate");
}

#[test]
fn the_same_file_given_twice_is_kept_once() {
    let dir = tree(&["a.txt"]);
    let file = dir.path().join("a.txt");
    let dotted = dir.path().join(".").join("a.txt");
    let found = run(&[file.clone(), dotted], &DiscoveryOptions::default());
    assert_eq!(found.paths(), [file.to_string_lossy()]);
    assert_eq!(found.exclusions[0].reason, ExclusionReason::Duplicate);
}

#[test]
fn hard_links_are_two_paths_here_and_one_file_when_read() {
    // Telling hard links apart needs their identity, which means opening
    // them: that is done when they are read (step 7), not while listing.
    let dir = tree(&["a.bin"]);
    std::fs::hard_link(dir.path().join("a.bin"), dir.path().join("b.bin")).unwrap();
    let found = run(&[dir.path().to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(found.names(dir.path()), ["a.bin", "b.bin"]);
    let id = |name: &str| platform::identify(&dir.path().join(name)).unwrap().id;
    assert_eq!(id("a.bin"), id("b.bin"));
}

#[test]
fn a_missing_root_is_an_error_not_a_stop() {
    let dir = tree(&["a.txt"]);
    let roots = [dir.path().join("missing"), dir.path().join("a.txt")];
    let found = run(&roots, &DiscoveryOptions::default());
    assert_eq!(found.names(dir.path()), ["a.txt"]);
    assert_eq!(found.errors.len(), 1);
    assert_eq!(found.errors[0].problem.kind, ProblemKind::NotFound);
    assert_eq!(relative(dir.path(), &found.errors[0].path), "missing");
}

#[test]
fn cancelling_stops_the_walk() {
    let dir = tempfile::tempdir().unwrap();
    for folder in 0..5 {
        for file in 0..600 {
            write(&dir.path().join(format!("{folder}/{file}.txt")), b"x");
        }
    }
    let control = Control::new();
    let mut seen = 0;
    let summary = discover(
        &[dir.path().to_path_buf()],
        &DiscoveryOptions::default(),
        &control,
        &mut |batch| {
            seen += batch.files.len();
            if seen > 0 {
                control.cancel();
            }
        },
    );
    assert!(summary.cancelled);
    assert!(summary.files < 3000, "stopped early: {}", summary.files);
    assert_eq!(summary.files as usize, seen);

    let control = Control::new();
    control.cancel();
    let found = run_with(
        &[dir.path().to_path_buf()],
        &DiscoveryOptions::default(),
        &control,
    );
    assert!(found.files.is_empty());
    assert!(found.summary.cancelled);
}

#[test]
fn large_folders_are_handed_over_in_batches() {
    let dir = tempfile::tempdir().unwrap();
    for file in 0..2500 {
        write(&dir.path().join(format!("{file}.txt")), b"x");
    }
    let found = run(&[dir.path().to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(found.files.len(), 2500);
    assert!(found.batches >= 3, "{} batches", found.batches);
    assert_eq!(relative(dir.path(), &found.files[0].path), "0.txt");
    assert_eq!(relative(dir.path(), &found.files[2499].path), "2499.txt");
}

#[test]
fn classification_of_attributes() {
    let none = Attributes::default();
    let hidden = Attributes::from_bits(0x2);
    let system = Attributes::from_bits(0x4);
    let online = Attributes::from_bits(0x0040_0000);
    let offline = Attributes::from_bits(0x1000);
    assert_eq!(file_exclusion("a.mkv", none, false), None);
    assert_eq!(
        file_exclusion("a.mkv", hidden, false),
        Some(ExclusionReason::Hidden)
    );
    assert_eq!(file_exclusion("a.mkv", hidden, true), None);
    assert_eq!(
        file_exclusion("a.mkv", system, false),
        Some(ExclusionReason::System)
    );
    assert_eq!(
        file_exclusion("a.mkv", online, true),
        Some(ExclusionReason::OnlineOnly)
    );
    assert_eq!(
        file_exclusion("a.mkv", offline, false),
        Some(ExclusionReason::OnlineOnly)
    );
    assert_eq!(
        file_exclusion("a.fileseal", hidden, false),
        Some(ExclusionReason::FileSealFile),
        "the most specific reason"
    );
    // A customised folder carries the system attribute: still walked.
    assert_eq!(folder_exclusion("Films", system, false), None);
    assert_eq!(
        folder_exclusion("Films", hidden, false),
        Some(ExclusionReason::Hidden)
    );
    assert_eq!(
        folder_exclusion("$Recycle.Bin", none, true),
        Some(ExclusionReason::SystemFolder)
    );
}

#[test]
fn system_folders_are_never_walked() {
    let dir = tree(&[
        "$RECYCLE.BIN/deleted.mkv",
        "System Volume Information/x",
        "kept.txt",
    ]);
    let options = DiscoveryOptions {
        include_hidden: true,
        ..DiscoveryOptions::default()
    };
    let found = run(&[dir.path().to_path_buf()], &options);
    assert_eq!(found.names(dir.path()), ["kept.txt"]);
    assert_eq!(
        found.excluded(dir.path()),
        [
            ("$RECYCLE.BIN".into(), ExclusionReason::SystemFolder, true),
            (
                "System Volume Information".into(),
                ExclusionReason::SystemFolder,
                true
            ),
        ]
    );
    // Chosen directly, it is walked: the user asked for it.
    let found = run(&[dir.path().join("$RECYCLE.BIN")], &options);
    assert_eq!(found.files.len(), 1);
}

#[cfg(windows)]
#[test]
fn hidden_and_system_files_are_excluded_unless_asked() {
    let dir = tree(&[
        "visible.txt",
        "hidden.txt",
        "system.txt",
        "Hidden folder/inside.txt",
        "Custom folder/inside.txt",
    ]);
    let root = dir.path();
    attrib("+H", &root.join("hidden.txt"));
    attrib("+S", &root.join("system.txt"));
    attrib("+H", &root.join("Hidden folder"));
    attrib("+S", &root.join("Custom folder"));

    let found = run(&[root.to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(
        found.names(root),
        ["visible.txt", "Custom folder/inside.txt"]
    );
    assert_eq!(
        found.excluded(root),
        [
            ("hidden.txt".into(), ExclusionReason::Hidden, false),
            ("system.txt".into(), ExclusionReason::System, false),
            ("Hidden folder".into(), ExclusionReason::Hidden, true),
        ]
    );

    let options = DiscoveryOptions {
        include_hidden: true,
        ..DiscoveryOptions::default()
    };
    let found = run(&[root.to_path_buf()], &options);
    assert_eq!(found.files.len(), 5);
    assert!(found.exclusions.is_empty());
    // Attributes put back, or the temporary folder cannot be removed.
    attrib("-S", &root.join("system.txt"));
}

#[cfg(windows)]
#[test]
fn junctions_are_not_followed_so_loops_are_impossible() {
    let dir = tree(&["real/film.mkv"]);
    let root = dir.path();
    // A junction inside the folder pointing back at the folder: a loop.
    let status = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(root.join("real").join("loop"))
        .arg(root.join("real"))
        .output()
        .unwrap();
    assert!(status.status.success(), "{status:?}");
    let found = run(&[root.join("real")], &DiscoveryOptions::default());
    assert_eq!(found.names(root), ["real/film.mkv"]);
    assert_eq!(
        found.excluded(root),
        [("real/loop".into(), ExclusionReason::Link, true)]
    );
    // Chosen directly, a junction is followed.
    let found = run(
        &[root.join("real").join("loop")],
        &DiscoveryOptions::default(),
    );
    assert_eq!(found.names(root), ["real/loop/film.mkv"]);
    assert_eq!(
        found.excluded(root),
        [("real/loop/loop".into(), ExclusionReason::Link, true)]
    );
}

#[cfg(windows)]
#[test]
fn symbolic_links_are_excluded_when_they_can_be_made() {
    let dir = tree(&["target.txt", "folder/x.txt"]);
    let root = dir.path();
    let file_link =
        std::os::windows::fs::symlink_file(root.join("target.txt"), root.join("link.txt"));
    if file_link.is_err() {
        // Needs developer mode or the "create symbolic links" privilege.
        eprintln!("symbolic links not allowed here: test skipped");
        return;
    }
    std::os::windows::fs::symlink_dir(root.join("folder"), root.join("folder link")).unwrap();
    let found = run(&[root.to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(found.names(root), ["target.txt", "folder/x.txt"]);
    assert_eq!(
        found.excluded(root),
        [
            ("link.txt".into(), ExclusionReason::Link, false),
            ("folder link".into(), ExclusionReason::Link, true),
        ]
    );
    // A link chosen directly is followed.
    let found = run(&[root.join("link.txt")], &DiscoveryOptions::default());
    assert_eq!(found.files[0].size, "target.txt".len() as u64);
}

#[cfg(windows)]
#[test]
fn reserved_names_and_trailing_dots_are_the_files_themselves() {
    let dir = tree(&["ordinary.txt"]);
    let root = dir.path();
    let names = ["nul", "con.txt", "name.", "space "];
    for name in names {
        // Only the extended form can create these names.
        let path = format!(r"\\?\{}\{name}", root.display());
        std::fs::write(&path, format!("content of {name}")).unwrap();
    }
    let found = run(&[root.to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(
        found.names(root),
        ["con.txt", "name.", "nul", "ordinary.txt", "space "]
    );
    for file in &found.files {
        let name = relative(root, &file.path);
        let expected = format!("content of {name}");
        if name == "ordinary.txt" {
            continue;
        }
        assert_eq!(file.size, expected.len() as u64, "{name}");
        // Read through the path found: the file, not a device or another name.
        let hashed = reader::hash_file(
            Path::new(&file.path),
            &[crate::Algorithm::Sha256],
            &Control::new(),
            &mut |_, _| {},
        )
        .unwrap();
        let mut hasher = crate::algorithm::MultiHasher::new(&[crate::Algorithm::Sha256]);
        hasher.update(expected.as_bytes());
        assert_eq!(hashed.digests, hasher.finish(), "{name}");
    }
    // Given directly too.
    let found = run(&[root.join("name.")], &DiscoveryOptions::default());
    assert_eq!(found.names(root), ["name."]);
}

#[cfg(windows)]
#[test]
fn names_that_are_not_unicode_are_reported() {
    use std::os::windows::ffi::OsStringExt;
    let dir = tree(&["fine.txt"]);
    let bad = std::ffi::OsString::from_wide(&[0xD800, u16::from(b'x')]);
    std::fs::write(dir.path().join(&bad), b"x").unwrap();
    let found = run(&[dir.path().to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(found.names(dir.path()), ["fine.txt"]);
    assert_eq!(found.errors.len(), 1);
    assert_eq!(
        found.errors[0].problem.kind,
        ProblemKind::NameNotRepresentable
    );
}

#[test]
fn unicode_and_long_paths_are_found_and_readable() {
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().join("Été 日本語 🎬");
    for _ in 0..12 {
        deep.push("a fairly long folder name to go past 260");
    }
    let file = deep.join("Ünïcödé film.mkv");
    write(&file, b"long");
    assert!(file.to_string_lossy().len() > 300);
    let found = run(&[dir.path().to_path_buf()], &DiscoveryOptions::default());
    assert_eq!(found.paths(), [file.to_string_lossy()]);
    assert_eq!(found.summary.folders_scanned, 14);
    let hashed = reader::hash_file(
        Path::new(&found.files[0].path),
        &[crate::Algorithm::Sha256],
        &Control::new(),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(hashed.size, 4);
}

#[cfg(windows)]
#[test]
fn a_folder_that_cannot_be_listed_is_an_error_and_the_rest_goes_on() {
    let dir = tree(&["open/a.txt", "closed/b.txt", "z.txt"]);
    let closed = dir.path().join("closed");
    let icacls = |args: &[&str]| {
        let output = Command::new("icacls")
            .arg(&closed)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    };
    // Everyone (S-1-1-0) may not list the folder.
    icacls(&["/deny", "*S-1-1-0:(RD)"]);
    let found = run(&[dir.path().to_path_buf()], &DiscoveryOptions::default());
    icacls(&["/remove:d", "*S-1-1-0"]);
    assert_eq!(found.names(dir.path()), ["z.txt", "open/a.txt"]);
    assert_eq!(found.errors.len(), 1);
    assert_eq!(found.errors[0].problem.kind, ProblemKind::PermissionDenied);
    assert_eq!(relative(dir.path(), &found.errors[0].path), "closed");
}

/// `cargo test -p fileseal-core --release -- --ignored fifty_thousand --nocapture`
#[test]
#[ignore = "long: 50,000 files"]
fn fifty_thousand_files() {
    let dir = tempfile::tempdir().unwrap();
    for folder in 0..100 {
        let path = dir.path().join(format!("folder {folder}"));
        std::fs::create_dir(&path).unwrap();
        for file in 0..500 {
            std::fs::write(path.join(format!("file {file}.mkv")), b"").unwrap();
        }
    }
    let start = Instant::now();
    let mut first = None;
    let mut files = 0;
    let summary = discover(
        &[dir.path().to_path_buf()],
        &DiscoveryOptions::default(),
        &Control::new(),
        &mut |batch| {
            if first.is_none() && !batch.files.is_empty() {
                first = Some(start.elapsed());
            }
            files += batch.files.len();
        },
    );
    let total = start.elapsed();
    eprintln!(
        "50,000 files: first batch after {:?}, all after {total:?}",
        first.unwrap()
    );
    assert_eq!(summary.files, 50_000);
    assert_eq!(files, 50_000);
    assert!(first.unwrap() < Duration::from_secs(1));
}
