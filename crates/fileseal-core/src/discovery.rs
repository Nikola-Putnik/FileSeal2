//! Turns what the user selected (files and folders) into the list of files
//! to process. Runs in the background and hands its findings over in
//! batches, so the first files show up at once and a large folder can be
//! cancelled at any moment.
//!
//! Rules (cadrage §2, n° 10 to 13):
//! - a file chosen directly is always kept: exclusions and the extension
//!   filter only apply to what is found inside a folder;
//! - links and junctions inside a folder are never followed (no loops
//!   possible); a link chosen directly is followed;
//! - every exclusion is reported with its reason, never silently dropped;
//! - a file reached twice (same path, overlapping folders, mapped drive and
//!   its network path) is kept once. Hard links are recognised later, when
//!   the file is opened: telling them apart here would mean opening every
//!   file of the selection.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{FileType, Metadata};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::platform::{self, Attributes};
use crate::problem::{Problem, ProblemKind};
use crate::reader::Control;

/// A batch is handed over once it holds this many entries…
const BATCH_ENTRIES: usize = 1000;
/// …or this long after the previous one, whichever comes first.
const BATCH_INTERVAL: Duration = Duration::from_millis(100);

/// Folders that belong to Windows, excluded even when hidden files are
/// included (compared without case).
const SYSTEM_FOLDERS: [&str; 2] = ["$recycle.bin", "system volume information"];

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct DiscoveryOptions {
    /// Look into subfolders too.
    pub recursive: bool,
    /// Keep hidden and system files found in folders.
    pub include_hidden: bool,
    /// Extensions to keep, without the dot, any case; `""` keeps files
    /// without extension. Empty: every file.
    pub extensions: Vec<String>,
}

impl Default for DiscoveryOptions {
    fn default() -> DiscoveryOptions {
        DiscoveryOptions {
            recursive: true,
            include_hidden: false,
            extensions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FoundFile {
    pub path: String,
    /// As listed by the folder: the reading step checks it again.
    #[ts(type = "number")]
    pub size: u64,
    /// Lowercase, without the dot; empty when the name has none.
    pub extension: String,
    /// Chosen by the user rather than found in a folder.
    pub direct: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExclusionReason {
    Hidden,
    System,
    /// `$RECYCLE.BIN`, `System Volume Information`.
    SystemFolder,
    /// `.md5`, `.sha256`, `SHA256SUMS`…: they describe other files.
    HashFile,
    /// A FileSeal database, its companion files or a copy made before a
    /// migration.
    FileSealFile,
    /// A temporary file FileSeal is writing (or left after a crash).
    TemporaryFile,
    /// A cloud placeholder ("online only"): reading it would download it.
    OnlineOnly,
    /// A symbolic link or a junction: not followed inside a folder.
    Link,
    /// Device, pipe or socket (Unix only).
    Special,
    /// Already in the selection.
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Exclusion {
    pub path: String,
    pub reason: ExclusionReason,
    /// A whole folder was left out, not a single file.
    pub folder: bool,
}

/// A folder that could not be listed, or an entry that could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TraversalError {
    pub path: String,
    pub problem: Problem,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiscoveryProgress {
    pub files: u32,
    #[ts(type = "number")]
    pub bytes: u64,
    pub folders_scanned: u32,
    pub excluded: u32,
    pub errors: u32,
    /// The folder being listed, for a slow network share.
    pub current_folder: Option<String>,
}

/// What was found since the previous batch, with the totals so far. A batch
/// may carry progress only, while a large folder is being listed. The last
/// one carries the summary: whoever receives the batches through a channel
/// knows from it that nothing more will come.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiscoveryBatch {
    pub files: Vec<FoundFile>,
    pub exclusions: Vec<Exclusion>,
    pub errors: Vec<TraversalError>,
    pub progress: DiscoveryProgress,
    /// On the last batch only.
    pub summary: Option<DiscoverySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReasonCount {
    pub reason: ExclusionReason,
    pub count: u32,
}

/// Files of one extension that passed the exclusions, whether or not the
/// extension filter kept them: enough to offer each extension as a choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExtensionCount {
    pub extension: String,
    pub files: u32,
    #[ts(type = "number")]
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiscoverySummary {
    pub files: u32,
    #[ts(type = "number")]
    pub bytes: u64,
    pub folders_scanned: u32,
    /// By reason, in a stable order.
    pub excluded: Vec<ReasonCount>,
    /// Left out by the extension filter (not listed one by one).
    pub filtered_out: u32,
    pub errors: u32,
    /// Most frequent first.
    pub extensions: Vec<ExtensionCount>,
    /// The selection is incomplete: the user stopped it.
    pub cancelled: bool,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}

/// Lists the files of `roots`, in order: each root in turn, and inside a
/// folder its files (sorted as Explorer does) before its subfolders. Hands
/// batches to `sink` as it goes; returns the totals.
pub fn discover(
    roots: &[PathBuf],
    options: &DiscoveryOptions,
    control: &Control,
    sink: &mut dyn FnMut(DiscoveryBatch),
) -> DiscoverySummary {
    let mut walk = Walk::new(options, control, sink);
    for root in roots {
        if control.is_cancelled() {
            break;
        }
        walk.root(root);
    }
    walk.finish()
}

/// Extension as used by the filter: after the last dot, unless that dot
/// starts the name (`.bashrc` has none), lowercased.
pub fn extension_of(name: &str) -> String {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[dot + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// Explorer's order: case ignored and digits compared as numbers, so that
/// "Episode 2" comes before "Episode 10". Exact ties are broken by the
/// plain comparison, so the order never depends on the listing.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let n = digits(&mut x);
                let m = digits(&mut y);
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let order = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(c), Some(d)) => {
                let order = c.to_lowercase().cmp(d.to_lowercase());
                if order != Ordering::Equal {
                    return order;
                }
                x.next();
                y.next();
            }
        }
    }
}

fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut run = String::new();
    while let Some(c) = chars.next_if(char::is_ascii_digit) {
        run.push(c);
    }
    run
}

fn is_hidden(name: &str, attributes: Attributes) -> bool {
    // Outside Windows, a leading dot is what hides a name.
    attributes.hidden() || (!cfg!(windows) && name.starts_with('.'))
}

/// Why a file found in a folder is left out, if it is. The most specific
/// reason wins: a hidden FileSeal database is reported as a database.
fn file_exclusion(
    name: &str,
    attributes: Attributes,
    include_hidden: bool,
) -> Option<ExclusionReason> {
    let lower = name.to_lowercase();
    if is_fileseal_file(&lower) {
        Some(ExclusionReason::FileSealFile)
    } else if lower.starts_with(".fileseal-") && lower.ends_with(".tmp") {
        Some(ExclusionReason::TemporaryFile)
    } else if is_hash_file(&lower) {
        Some(ExclusionReason::HashFile)
    } else if !include_hidden && is_hidden(name, attributes) {
        Some(ExclusionReason::Hidden)
    } else if !include_hidden && attributes.system() {
        Some(ExclusionReason::System)
    } else if attributes.online_only() {
        Some(ExclusionReason::OnlineOnly)
    } else {
        None
    }
}

/// Why a folder is not looked into, if it is not. The system attribute
/// alone does not exclude a folder: Windows sets it on ordinary folders
/// whose icon or name was customised (`desktop.ini`).
fn folder_exclusion(
    name: &str,
    attributes: Attributes,
    include_hidden: bool,
) -> Option<ExclusionReason> {
    let lower = name.to_lowercase();
    if SYSTEM_FOLDERS.contains(&lower.as_str()) {
        Some(ExclusionReason::SystemFolder)
    } else if !include_hidden && is_hidden(name, attributes) {
        Some(ExclusionReason::Hidden)
    } else {
        None
    }
}

fn is_fileseal_file(lower: &str) -> bool {
    [
        ".fileseal",
        ".fileseal-wal",
        ".fileseal-shm",
        ".fileseal-journal",
        ".fileseal.partial",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
}

fn is_hash_file(lower: &str) -> bool {
    const EXTENSIONS: [&str; 7] = [
        "md5",
        "sha1",
        "sha256",
        "sha512",
        "sfv",
        "md5sum",
        "sha256sum",
    ];
    const LISTS: [&str; 5] = ["md5sums", "sha1sums", "sha256sums", "sha512sums", "b2sums"];
    let stem = lower.strip_suffix(".txt").unwrap_or(lower);
    EXTENSIONS.contains(&extension_of(lower).as_str()) || LISTS.contains(&stem)
}

/// 128 bits of a path key: what the duplicate check remembers, so that a
/// selection of millions of files does not keep millions of paths.
fn fingerprint(key: &str) -> u128 {
    let half = |salt: u8| {
        let mut hasher = DefaultHasher::new();
        salt.hash(&mut hasher);
        key.hash(&mut hasher);
        hasher.finish()
    };
    (u128::from(half(0)) << 64) | u128::from(half(1))
}

/// A folder waiting to be listed: its path and its duplicate-check key.
struct Folder {
    path: PathBuf,
    key: String,
}

/// An entry of a folder, as listed.
struct Entry {
    name: String,
    file_type: FileType,
    metadata: Option<Metadata>,
}

struct Walk<'a> {
    options: &'a DiscoveryOptions,
    filter: Option<HashSet<String>>,
    control: &'a Control,
    sink: &'a mut dyn FnMut(DiscoveryBatch),
    start: Instant,
    last_flush: Instant,
    batch: DiscoveryBatch,
    progress: DiscoveryProgress,
    seen_files: HashSet<u128>,
    seen_folders: HashSet<u128>,
    excluded: HashMap<ExclusionReason, u32>,
    filtered_out: u32,
    extensions: HashMap<String, (u32, u64)>,
}

impl<'a> Walk<'a> {
    fn new(
        options: &'a DiscoveryOptions,
        control: &'a Control,
        sink: &'a mut dyn FnMut(DiscoveryBatch),
    ) -> Walk<'a> {
        let filter = (!options.extensions.is_empty()).then(|| {
            options
                .extensions
                .iter()
                .map(|e| e.trim_start_matches('.').to_lowercase())
                .collect()
        });
        let start = Instant::now();
        Walk {
            options,
            filter,
            control,
            sink,
            start,
            last_flush: start,
            batch: DiscoveryBatch::default(),
            progress: DiscoveryProgress::default(),
            seen_files: HashSet::new(),
            seen_folders: HashSet::new(),
            excluded: HashMap::new(),
            filtered_out: 0,
            extensions: HashMap::new(),
        }
    }

    fn root(&mut self, root: &Path) {
        let path = match platform::absolute(root) {
            Ok(path) => path,
            Err(err) => return self.error(root, Problem::from_io(&err)),
        };
        let text = match platform::path_text(&path) {
            Ok(text) => text,
            Err(problem) => return self.error(&path, problem),
        };
        // Followed: a link chosen directly stands for what it points to.
        let metadata = match std::fs::metadata(platform::extended(&path)) {
            Ok(metadata) => metadata,
            Err(err) => return self.error(&path, Problem::from_io(&err)),
        };
        let key = platform::canonical_key(&text);
        if metadata.is_dir() {
            self.walk(Folder { path, key });
        } else if metadata.is_file() {
            self.admit(text, &key, metadata.len(), true);
        } else {
            self.error(
                &path,
                Problem::new(ProblemKind::NotAFile, "neither a file nor a folder"),
            );
        }
        self.flush_if_due();
    }

    /// Depth first, files of a folder before its subfolders.
    fn walk(&mut self, root: Folder) {
        let mut pending = vec![root];
        while let Some(folder) = pending.pop() {
            if self.control.is_cancelled() {
                return;
            }
            if !self.seen_folders.insert(fingerprint(&folder.key)) {
                self.exclude(&folder.path, ExclusionReason::Duplicate, true);
                continue;
            }
            self.progress.folders_scanned += 1;
            self.progress.current_folder = Some(folder.path.display().to_string());
            let entries = self.list(&folder.path);
            let mut subfolders = Vec::new();
            for entry in entries {
                if self.control.is_cancelled() {
                    return;
                }
                if let Some(sub) = self.entry(&folder, entry) {
                    subfolders.push(sub);
                }
                self.flush_if_due();
            }
            // Popped in reverse: the first subfolder is walked first.
            pending.extend(subfolders.into_iter().rev());
            self.flush_if_due();
        }
    }

    /// The folder's entries, sorted. Problems are reported, never fatal.
    fn list(&mut self, dir: &Path) -> Vec<Entry> {
        let reader = match std::fs::read_dir(platform::extended(dir)) {
            Ok(reader) => reader,
            Err(err) => {
                self.error(dir, Problem::from_io(&err));
                return Vec::new();
            }
        };
        let mut entries = Vec::new();
        for item in reader {
            if self.control.is_cancelled() {
                break;
            }
            let item = match item {
                Ok(item) => item,
                Err(err) => {
                    self.error(dir, Problem::from_io(&err));
                    continue;
                }
            };
            let name = match item.file_name().into_string() {
                Ok(name) => name,
                Err(raw) => {
                    self.unrepresentable(dir, raw);
                    continue;
                }
            };
            let file_type = match item.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    self.error(&dir.join(&name), Problem::from_io(&err));
                    continue;
                }
            };
            // Free on Windows (the listing carries it); one call elsewhere.
            let metadata = if file_type.is_symlink() {
                None
            } else {
                match item.metadata() {
                    Ok(metadata) => Some(metadata),
                    Err(err) => {
                        self.error(&dir.join(&name), Problem::from_io(&err));
                        continue;
                    }
                }
            };
            entries.push(Entry {
                name,
                file_type,
                metadata,
            });
            // A folder of a million entries on a slow share: keep the count moving.
            self.flush_if_due();
        }
        entries.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        // Files first, then folders, each in name order.
        entries.sort_by_key(|entry| entry.file_type.is_dir());
        entries
    }

    /// Handles one entry; returns it if it is a folder to walk.
    fn entry(&mut self, folder: &Folder, entry: Entry) -> Option<Folder> {
        let path = folder.path.join(&entry.name);
        if entry.file_type.is_symlink() {
            let is_dir = is_link_to_folder(&entry.file_type);
            self.exclude(&path, ExclusionReason::Link, is_dir);
            return None;
        }
        let metadata = entry.metadata?;
        let attributes = Attributes::of(&metadata);
        let key = child_key(&folder.key, &entry.name);
        if entry.file_type.is_dir() {
            if let Some(reason) =
                folder_exclusion(&entry.name, attributes, self.options.include_hidden)
            {
                self.exclude(&path, reason, true);
                return None;
            }
            return self.options.recursive.then_some(Folder { path, key });
        }
        if !entry.file_type.is_file() {
            self.exclude(&path, ExclusionReason::Special, false);
            return None;
        }
        if let Some(reason) = file_exclusion(&entry.name, attributes, self.options.include_hidden) {
            self.exclude(&path, reason, false);
            return None;
        }
        // Built from valid UTF-8 parts, so this cannot fail.
        let text = path.to_string_lossy().into_owned();
        self.admit(text, &key, metadata.len(), false);
        None
    }

    /// Duplicate check, extension count, then the filter (never applied
    /// to a file chosen directly). A file left out by the filter is not
    /// remembered: chosen directly later, it is kept.
    fn admit(&mut self, path: String, key: &str, size: u64, direct: bool) {
        let fingerprint = fingerprint(key);
        if self.seen_files.contains(&fingerprint) {
            self.exclude(Path::new(&path), ExclusionReason::Duplicate, false);
            return;
        }
        let extension = extension_of(&file_name(&path));
        self.count_extension(&extension, size);
        if !direct
            && self
                .filter
                .as_ref()
                .is_some_and(|keep| !keep.contains(&extension))
        {
            self.filtered_out += 1;
            return;
        }
        self.seen_files.insert(fingerprint);
        self.progress.files += 1;
        self.progress.bytes += size;
        self.batch.files.push(FoundFile {
            path,
            size,
            extension,
            direct,
        });
    }

    fn count_extension(&mut self, extension: &str, size: u64) {
        let count = self.extensions.entry(extension.to_owned()).or_default();
        count.0 += 1;
        count.1 += size;
    }

    fn exclude(&mut self, path: &Path, reason: ExclusionReason, folder: bool) {
        *self.excluded.entry(reason).or_default() += 1;
        self.progress.excluded += 1;
        self.batch.exclusions.push(Exclusion {
            path: path.display().to_string(),
            reason,
            folder,
        });
    }

    fn error(&mut self, path: &Path, problem: Problem) {
        tracing::warn!(path = %path.display(), %problem, "selection");
        self.progress.errors += 1;
        self.batch.errors.push(TraversalError {
            path: path.display().to_string(),
            problem,
        });
    }

    fn unrepresentable(&mut self, dir: &Path, raw: OsString) {
        let path = dir.join(raw);
        let problem = Problem::new(
            ProblemKind::NameNotRepresentable,
            format!("{} is not valid Unicode", path.display()),
        );
        self.error(&path, problem);
    }

    fn flush_if_due(&mut self) {
        let entries =
            self.batch.files.len() + self.batch.exclusions.len() + self.batch.errors.len();
        if entries >= BATCH_ENTRIES || self.last_flush.elapsed() >= BATCH_INTERVAL {
            self.flush();
        }
    }

    fn flush(&mut self) {
        self.last_flush = Instant::now();
        let mut batch = std::mem::take(&mut self.batch);
        batch.progress = self.progress.clone();
        (self.sink)(batch);
    }

    fn finish(mut self) -> DiscoverySummary {
        self.progress.current_folder = None;
        let summary = self.summary();
        self.batch.summary = Some(summary.clone());
        self.flush();
        summary
    }

    fn summary(&self) -> DiscoverySummary {
        let mut excluded: Vec<ReasonCount> = self
            .excluded
            .iter()
            .map(|(&reason, &count)| ReasonCount { reason, count })
            .collect();
        excluded.sort_by_key(|count| count.reason);
        let mut extensions: Vec<ExtensionCount> = self
            .extensions
            .iter()
            .map(|(extension, &(files, bytes))| ExtensionCount {
                extension: extension.clone(),
                files,
                bytes,
            })
            .collect();
        extensions.sort_by(|a, b| {
            b.files
                .cmp(&a.files)
                .then_with(|| natural_cmp(&a.extension, &b.extension))
        });
        DiscoverySummary {
            files: self.progress.files,
            bytes: self.progress.bytes,
            folders_scanned: self.progress.folders_scanned,
            excluded,
            filtered_out: self.filtered_out,
            errors: self.progress.errors,
            extensions,
            cancelled: self.control.is_cancelled(),
            elapsed_ms: u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX),
        }
    }
}

fn file_name(path_text: &str) -> String {
    Path::new(path_text)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The key of a folder's entry, built from the folder's key: the same as
/// `platform::canonical_key` of its path, without asking the system again.
fn child_key(folder_key: &str, name: &str) -> String {
    let separator = std::path::MAIN_SEPARATOR;
    let name = platform::path_key(name);
    if folder_key.ends_with(separator) {
        format!("{folder_key}{name}")
    } else {
        format!("{folder_key}{separator}{name}")
    }
}

#[cfg(windows)]
fn is_link_to_folder(file_type: &FileType) -> bool {
    std::os::windows::fs::FileTypeExt::is_symlink_dir(file_type)
}

#[cfg(not(windows))]
fn is_link_to_folder(_file_type: &FileType) -> bool {
    // Telling would mean following the link, which the walk never does.
    false
}

#[cfg(test)]
mod tests;
