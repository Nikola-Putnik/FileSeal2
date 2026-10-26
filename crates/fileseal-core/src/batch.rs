//! Batches: one action (create, verify, search) on a list of files.
//!
//! Rules (cadrage §2, n° 14, 16, 17 and 22; §8):
//! - one reader per volume, several volumes at once (2 by default, 1 to 4):
//!   two readers on one disk would only make its head jump back and forth;
//! - files start in list order, as far as that rule allows; results come
//!   with their position in the list;
//! - an error on one file never stops the batch;
//! - a volume that disappears (unplugged disk, lost share): the file being
//!   read fails, the remaining files of that volume are "not processed",
//!   the other volumes go on;
//! - a database that can no longer be written stops the batch: results
//!   obtained so far stay, the remaining files are "not processed";
//! - pause and cancel act between two blocks of a file; a cancelled batch
//!   keeps what was finished, the file being read is "cancelled", the rest
//!   "not processed";
//! - the same physical file reached twice (hard links) is read once;
//! - the computer does not go to sleep during a batch.
//!
//! Every file of the list ends with exactly one entry, whatever happens:
//! processed + failed + cancelled + not processed + duplicates = total.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::algorithm::Digest;
use crate::faults::{self, FaultPoint};
use crate::ops::search::{self, SearchReport, SearchStatus};
use crate::ops::{
    self, CreateOptions, CreateReport, CreateStatus, DbTarget, SourceKind, SourceOutcome,
    SourceResult, VerifyOptions, VerifyReport, VerifyStatus,
};
use crate::platform::{self, FileId};
use crate::problem::{Problem, ProblemKind};
use crate::progress::ThroughputMeter;
use crate::reader::Control;
use crate::report::ReportEntry;
use crate::sidecar::SidecarNote;
use crate::store::{OperationKind, OperationStatus, SharedStore, Store};

/// Updates are handed over at most this often…
const TICK: Duration = Duration::from_millis(100);
/// …or as soon as this many results wait.
const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum BatchAction {
    Create(CreateOptions),
    Verify(VerifyOptions),
    /// Needs a database; writes nothing.
    Search,
}

impl BatchAction {
    /// The action's name in reports.
    pub fn code(&self) -> &'static str {
        match self {
            BatchAction::Create(_) => "create",
            BatchAction::Verify(_) => "verify",
            BatchAction::Search => "search",
        }
    }
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchOptions {
    pub action: BatchAction,
    /// Volumes read at the same time, 1 to 4.
    #[serde(default = "default_parallel_volumes")]
    pub parallel_volumes: u8,
}

fn default_parallel_volumes() -> u8 {
    2
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchFile {
    pub path: String,
    /// As found by the selection; measured again if absent.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    /// The line of a hash file the user opened that names this file: it is
    /// verified against that line only, as `sha256sum -c` does.
    #[serde(default)]
    pub listed: Option<ListedReference>,
}

impl BatchFile {
    pub fn new(path: impl Into<String>) -> BatchFile {
        BatchFile {
            path: path.into(),
            size: None,
            listed: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListedReference {
    /// The hash file.
    pub list: String,
    pub line: u32,
    pub digest: Digest,
    pub notes: Vec<SidecarNote>,
}

impl ListedReference {
    fn source(&self) -> SourceResult {
        SourceResult {
            source: SourceKind::Sidecar,
            path: self.list.clone(),
            expected: vec![self.digest.clone()],
            expected_size: None,
            reference_created_at: None,
            outcome: SourceOutcome::NotCompared,
            detail: None,
            notes: self.notes.clone(),
        }
    }
}

/// The result for one file of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchEntry {
    /// Position in the list.
    pub index: u32,
    pub path: String,
    pub outcome: EntryOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", content = "report", rename_all = "camelCase")]
#[ts(export)]
pub enum EntryOutcome {
    Created(CreateReport),
    Verified(VerifyReport),
    Searched(SearchReport),
    /// Not read.
    Skipped(Skip),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Skip {
    pub reason: SkipReason,
    /// For a duplicate: the same file, earlier in the list.
    pub same_as: Option<DuplicateOf>,
    /// Why the database or the volume became unavailable.
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateOf {
    pub index: u32,
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SkipReason {
    /// The batch was cancelled before this file.
    Cancelled,
    /// Its disk or share disappeared during the batch.
    VolumeUnavailable,
    /// The database could no longer be written: the batch stopped.
    DatabaseUnavailable,
    /// The same physical file as another one of the list (hard link).
    Duplicate,
}

/// Where an entry counts in the summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// The action came to a conclusion, good or bad (a mismatch, a
    /// conflict… are results).
    Processed,
    /// The file could not be read, or the action could not be done.
    Failed,
    /// Interrupted while the file was read.
    Cancelled,
    /// Never started.
    NotProcessed,
    Duplicate,
}

impl EntryOutcome {
    pub fn category(&self) -> Category {
        match self {
            EntryOutcome::Created(report) => match report.status {
                CreateStatus::Failed => Category::Failed,
                CreateStatus::Cancelled => Category::Cancelled,
                _ => Category::Processed,
            },
            EntryOutcome::Verified(report) => match report.status {
                VerifyStatus::NotFound
                | VerifyStatus::ReadError
                | VerifyStatus::Locked
                | VerifyStatus::ModifiedDuringRead
                | VerifyStatus::Failed => Category::Failed,
                VerifyStatus::Cancelled => Category::Cancelled,
                VerifyStatus::NotProcessed => Category::NotProcessed,
                _ => Category::Processed,
            },
            EntryOutcome::Searched(report) => match report.status {
                SearchStatus::NotFound
                | SearchStatus::ReadError
                | SearchStatus::Locked
                | SearchStatus::ModifiedDuringRead
                | SearchStatus::Failed => Category::Failed,
                SearchStatus::Cancelled => Category::Cancelled,
                _ => Category::Processed,
            },
            EntryOutcome::Skipped(skip) if skip.reason == SkipReason::Duplicate => {
                Category::Duplicate
            }
            EntryOutcome::Skipped(_) => Category::NotProcessed,
        }
    }

    /// The status code, as in reports and the database history.
    pub fn status_code(&self) -> String {
        match self {
            EntryOutcome::Created(report) => code(&report.status),
            EntryOutcome::Verified(report) => report.status.code().to_owned(),
            EntryOutcome::Searched(report) => code(&report.status),
            EntryOutcome::Skipped(skip) if skip.reason == SkipReason::Duplicate => {
                "duplicate".to_owned()
            }
            EntryOutcome::Skipped(_) => "notProcessed".to_owned(),
        }
    }

    fn problem(&self) -> Option<&Problem> {
        match self {
            EntryOutcome::Created(report) => report.problem.as_ref(),
            EntryOutcome::Verified(report) => report.problem.as_ref(),
            EntryOutcome::Searched(report) => report.problem.as_ref(),
            EntryOutcome::Skipped(_) => None,
        }
    }

    /// A database failure that means it can no longer be used (unplugged,
    /// full, damaged), as opposed to a refusal about this file alone. A
    /// database opened read-only refuses every write by design: verifying
    /// with it goes on, without history.
    fn database_lost(&self, opened_read_only: bool) -> Option<&Problem> {
        let found = match self {
            EntryOutcome::Created(report) => {
                report.database.as_ref().and_then(|d| d.problem.as_ref())
            }
            EntryOutcome::Verified(report) => {
                report.record_problem.as_ref().or(report.problem.as_ref())
            }
            EntryOutcome::Searched(report) => report.problem.as_ref(),
            EntryOutcome::Skipped(_) => None,
        }?;
        let lost = match found.kind {
            ProblemKind::DatabaseUnavailable
            | ProblemKind::DiskFull
            | ProblemKind::DatabaseCorrupt
            | ProblemKind::NotAFileSealDatabase => true,
            // The medium became write-protected during the batch.
            ProblemKind::DatabaseReadOnly => !opened_read_only,
            _ => false,
        };
        lost.then_some(found)
    }
}

fn code<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchProgress {
    pub files_done: u32,
    pub files_total: u32,
    /// Bytes of the files finished (their whole size, read or not) and of
    /// those being read.
    #[ts(type = "number")]
    pub bytes_done: u64,
    #[ts(type = "number")]
    pub bytes_total: u64,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
    pub bytes_per_second: Option<f64>,
    /// `None` while there is not enough measure ("estimating").
    #[ts(type = "number | null")]
    pub remaining_ms: Option<u64>,
    pub paused: bool,
    /// The files being read, one per busy volume.
    pub current: Vec<CurrentFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CurrentFile {
    pub index: u32,
    pub path: String,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatusCount {
    pub status: String,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchSummary {
    pub total: u32,
    pub processed: u32,
    pub failed: u32,
    pub cancelled: u32,
    pub not_processed: u32,
    pub duplicates: u32,
    /// Entries by status code, in a stable order.
    pub statuses: Vec<StatusCount>,
    /// The user cancelled the batch.
    pub stopped_by_user: bool,
    /// The database could no longer be written: the batch stopped there.
    pub database_problem: Option<Problem>,
    /// Roots of the volumes that disappeared during the batch.
    pub lost_volumes: Vec<String>,
    #[ts(type = "number")]
    pub elapsed_ms: u64,
}

/// What the batch hands over as it goes: results since the previous
/// update, and the progress. The last update carries the summary: whoever
/// receives updates through a channel knows from it that nothing follows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchUpdate {
    pub entries: Vec<BatchEntry>,
    pub progress: BatchProgress,
    pub summary: Option<BatchSummary>,
}

/// The line of the CSV report for an entry.
pub fn report_entry(action: &BatchAction, entry: &BatchEntry) -> ReportEntry {
    match &entry.outcome {
        EntryOutcome::Created(report) => ReportEntry::Create(report.clone()),
        EntryOutcome::Verified(report) => ReportEntry::Verify(report.clone()),
        EntryOutcome::Searched(report) => ReportEntry::Search(report.clone()),
        EntryOutcome::Skipped(skip) => ReportEntry::Skipped {
            action: action.code(),
            path: entry.path.clone(),
            status: entry.outcome.status_code(),
            detail: [
                Some(code(&skip.reason)),
                skip.same_as
                    .as_ref()
                    .map(|same| format!("same file as {}", same.path)),
                skip.problem.as_ref().map(ToString::to_string),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" | "),
        },
    }
}

/// Runs `options.action` on `files`, with `store` as the database if
/// given. Hands updates to `sink` (on the calling thread) as it goes;
/// returns the summary, also carried by the last update.
pub fn run(
    files: &[BatchFile],
    options: &BatchOptions,
    store: Option<&mut Store>,
    control: &Control,
    sink: &mut dyn FnMut(BatchUpdate),
) -> Result<BatchSummary, Problem> {
    run_on_volumes(files, options, store, control, sink, &volume_of)
}

/// Same, with the volume of each file given by `volume`: tests use it to
/// spread files over several "volumes" of one disk.
fn run_on_volumes(
    files: &[BatchFile],
    options: &BatchOptions,
    mut store: Option<&mut Store>,
    control: &Control,
    sink: &mut dyn FnMut(BatchUpdate),
    volume: &dyn Fn(&Path) -> String,
) -> Result<BatchSummary, Problem> {
    check(options, store.as_deref())?;
    let _awake = platform::keep_awake();
    let kind = match options.action {
        BatchAction::Create(_) => Some(OperationKind::Create),
        BatchAction::Verify(_) => Some(OperationKind::Verify),
        BatchAction::Search => None,
    };
    let operation_id = match (store.as_deref_mut(), kind) {
        (Some(store), Some(kind)) => store.begin_operation(kind)?,
        _ => 0,
    };
    // Commits are not forced to the disk one by one during the batch, but
    // all at once at its end (store.rs, `batch_durability`).
    if let Some(store) = store.as_deref_mut()
        && !store.is_read_only()
        && let Err(problem) = store.batch_durability(true)
    {
        tracing::warn!(%problem, "batch durability not changed");
    }
    let plan = Plan::new(files, volume);
    tracing::info!(
        action = options.action.code(),
        files = plan.files.len(),
        volumes = plan.volumes.len(),
        "batch started"
    );
    let read_only = store.as_deref().is_some_and(Store::is_read_only);
    let summary = {
        let shared = store.as_deref_mut().map(SharedStore::new);
        let batch = Batch {
            plan: &plan,
            action: &options.action,
            db: shared.as_ref().map(|store| DbTarget {
                store,
                operation_id,
            }),
            read_only,
            control,
            schedule: Schedule::new(&plan),
            live: Mutex::new(Live::default()),
            seen: Mutex::new(HashMap::new()),
        };
        let readers =
            usize::from(options.parallel_volumes.clamp(1, 4)).min(plan.volumes.len().max(1));
        batch.run(readers, sink)
    };
    if let Some(store) = store.as_deref_mut()
        && !store.is_read_only()
        && let Err(problem) = store.batch_durability(false)
    {
        tracing::warn!(%problem, "batch results not forced to the disk");
    }
    if let Some(store) = store
        && kind.is_some()
    {
        let status = if summary.database_problem.is_some() {
            OperationStatus::Failed
        } else if summary.stopped_by_user {
            OperationStatus::Cancelled
        } else {
            OperationStatus::Completed
        };
        let json = serde_json::to_string(&summary).ok();
        if let Err(problem) = store.finish_operation(operation_id, status, json.as_deref()) {
            tracing::warn!(%problem, "batch operation not closed in the database");
        }
    }
    tracing::info!(
        total = summary.total,
        processed = summary.processed,
        failed = summary.failed,
        not_processed = summary.not_processed,
        "batch finished"
    );
    Ok(summary)
}

/// What makes a batch impossible from the start.
fn check(options: &BatchOptions, store: Option<&Store>) -> Result<(), Problem> {
    let refuse = |kind, detail: &str| Err(Problem::new(kind, detail));
    match &options.action {
        BatchAction::Create(create) => {
            if create.algorithms.is_empty()
                || (!create.write_sidecar && !create.write_sums && store.is_none())
            {
                return refuse(ProblemKind::Io, "no algorithm or no destination selected");
            }
            if store.is_some_and(Store::is_read_only) {
                return refuse(
                    ProblemKind::DatabaseReadOnly,
                    "the database is opened read-only",
                );
            }
        }
        BatchAction::Verify(_) => {}
        BatchAction::Search if store.is_none() => {
            return refuse(ProblemKind::Io, "searching needs a database");
        }
        BatchAction::Search => {}
    }
    Ok(())
}

/// The volume of a file: the root of its disk, share or mount point.
fn volume_of(path: &Path) -> String {
    platform::volume_root_of_file(path)
        .map(|root| root.display().to_string())
        .unwrap_or_default()
}

struct Planned {
    path: PathBuf,
    text: String,
    size: u64,
    volume: usize,
    listed: Option<ListedReference>,
}

/// The files with their size and volume, and the volumes (their roots)
/// in order of their first file.
struct Plan {
    files: Vec<Planned>,
    volumes: Vec<String>,
    bytes: u64,
}

impl Plan {
    fn new(files: &[BatchFile], volume: &dyn Fn(&Path) -> String) -> Plan {
        let mut volumes: Vec<String> = Vec::new();
        let mut keys: Vec<String> = Vec::new();
        // Asking the system once per folder, not once per file.
        let mut by_folder: HashMap<PathBuf, usize> = HashMap::new();
        let files = files
            .iter()
            .map(|file| {
                let path = platform::absolute(Path::new(&file.path))
                    .unwrap_or_else(|_| PathBuf::from(&file.path));
                let text = platform::path_text(&path).unwrap_or_else(|_| file.path.clone());
                let size = file.size.unwrap_or_else(|| {
                    std::fs::metadata(platform::extended(&path)).map_or(0, |m| m.len())
                });
                let folder = path.parent().unwrap_or(&path).to_path_buf();
                let volume = *by_folder.entry(folder).or_insert_with(|| {
                    let root = volume(&path);
                    let key = platform::path_key(&root);
                    match keys.iter().position(|k| *k == key) {
                        Some(known) => known,
                        None => {
                            keys.push(key);
                            volumes.push(root);
                            volumes.len() - 1
                        }
                    }
                });
                Planned {
                    path,
                    text,
                    size,
                    volume,
                    listed: file.listed.clone(),
                }
            })
            .collect::<Vec<Planned>>();
        let bytes = files.iter().map(|f| f.size).sum();
        Plan {
            files,
            volumes,
            bytes,
        }
    }
}

/// Which file each reader takes next.
struct Schedule {
    state: Mutex<ScheduleState>,
    changed: Condvar,
}

struct ScheduleState {
    /// Per volume, its files not started yet, in list order.
    queues: Vec<VecDeque<usize>>,
    busy: Vec<bool>,
    lost: Vec<bool>,
    database_problem: Option<Problem>,
}

impl Schedule {
    fn new(plan: &Plan) -> Schedule {
        let mut queues = vec![VecDeque::new(); plan.volumes.len()];
        for (index, file) in plan.files.iter().enumerate() {
            queues[file.volume].push_back(index);
        }
        Schedule {
            state: Mutex::new(ScheduleState {
                busy: vec![false; queues.len()],
                lost: vec![false; queues.len()],
                queues,
                database_problem: None,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, ScheduleState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The first file of the list whose volume is free, waiting for one to
    /// become free. `None` when there is nothing more for this reader.
    fn next(&self, control: &Control) -> Option<usize> {
        let mut state = self.lock();
        loop {
            if control.is_cancelled() || state.database_problem.is_some() {
                return None;
            }
            let free = (0..state.queues.len())
                .filter(|&v| !state.busy[v] && !state.lost[v])
                .filter_map(|v| state.queues[v].front().map(|&index| (index, v)))
                .min();
            if let Some((index, volume)) = free {
                state.queues[volume].pop_front();
                state.busy[volume] = true;
                return Some(index);
            }
            if state.queues.iter().all(VecDeque::is_empty) {
                return None;
            }
            // Only busy volumes have files left: wait for one of them.
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The reader is done with `volume`; if the volume is gone, its files
    /// not started are returned, never to be read.
    fn release(&self, volume: usize, lost: bool) -> Vec<usize> {
        let mut state = self.lock();
        state.busy[volume] = false;
        let dropped = if lost {
            state.lost[volume] = true;
            state.queues[volume].drain(..).collect()
        } else {
            Vec::new()
        };
        self.changed.notify_all();
        dropped
    }

    /// The database is lost: every file not started is returned.
    fn stop(&self, problem: Problem) -> Vec<usize> {
        let mut state = self.lock();
        if state.database_problem.is_none() {
            state.database_problem = Some(problem);
        }
        let dropped = state.queues.iter_mut().flat_map(|q| q.drain(..)).collect();
        self.changed.notify_all();
        dropped
    }
}

/// What the readers are doing, for the progress.
#[derive(Default)]
struct Live {
    current: HashMap<usize, CurrentFile>,
}

struct Batch<'a> {
    plan: &'a Plan,
    action: &'a BatchAction,
    db: Option<DbTarget<'a>>,
    /// The database was opened read-only: no history is written.
    read_only: bool,
    control: &'a Control,
    schedule: Schedule,
    live: Mutex<Live>,
    /// Identities of the files read, to recognise hard links.
    seen: Mutex<HashMap<FileId, usize>>,
}

/// What a reader reports for one file.
enum Report {
    Entry(Box<BatchEntry>),
    /// A volume disappeared (its root, for the summary).
    VolumeLost(String),
}

impl Batch<'_> {
    fn run(&self, readers: usize, sink: &mut dyn FnMut(BatchUpdate)) -> BatchSummary {
        let (sender, receiver) = mpsc::channel::<Report>();
        std::thread::scope(|scope| {
            for reader in 0..readers {
                let sender = sender.clone();
                std::thread::Builder::new()
                    .name(format!("fileseal-reader-{reader}"))
                    .spawn_scoped(scope, move || self.read(reader, &sender))
                    .expect("a reader thread could be started");
            }
            drop(sender);
            Collector::new(self, sink).collect(&receiver)
        })
    }

    /// One reader: takes the next file of a free volume until none is left.
    fn read(&self, reader: usize, sender: &Sender<Report>) {
        while let Some(index) = self.schedule.next(self.control) {
            let file = &self.plan.files[index];
            let outcome = self.process(reader, index);
            lock(&self.live).current.remove(&reader);
            let mut lost = false;
            if outcome.problem().is_some() && volume_gone(&file.path) {
                lost = true;
                let root = self.plan.volumes[file.volume].clone();
                tracing::warn!(volume = %root, "volume lost during the batch");
                let _ = sender.send(Report::VolumeLost(root));
            }
            let lost_database = outcome.database_lost(self.read_only).cloned();
            let _ = sender.send(Report::Entry(Box::new(self.entry(index, outcome))));
            for dropped in self.schedule.release(file.volume, lost) {
                let _ = sender.send(Report::Entry(Box::new(self.skipped(
                    dropped,
                    SkipReason::VolumeUnavailable,
                    None,
                ))));
            }
            if let Some(problem) = lost_database {
                tracing::warn!(%problem, "database lost: batch stopped");
                for dropped in self.schedule.stop(problem.clone()) {
                    let entry = self.skipped(
                        dropped,
                        SkipReason::DatabaseUnavailable,
                        Some(problem.clone()),
                    );
                    let _ = sender.send(Report::Entry(Box::new(entry)));
                }
            }
        }
    }

    fn process(&self, reader: usize, index: usize) -> EntryOutcome {
        let file = &self.plan.files[index];
        lock(&self.live).current.insert(
            reader,
            CurrentFile {
                index: index as u32,
                path: file.text.clone(),
                done: 0,
                total: file.size,
            },
        );
        // A pause before starting: the file is shown as next, not opened.
        if self.control.checkpoint().is_err() {
            return EntryOutcome::Skipped(skip(SkipReason::Cancelled, None, None));
        }
        if let Ok(state) = platform::identify(&file.path)
            && let Some(id) = state.id
        {
            let mut seen = lock(&self.seen);
            if let Some(&first) = seen.get(&id) {
                let same = DuplicateOf {
                    index: first as u32,
                    path: self.plan.files[first].text.clone(),
                };
                return EntryOutcome::Skipped(skip(SkipReason::Duplicate, Some(same), None));
            }
            seen.insert(id, index);
        }
        let mut progress = |done: u64, total: u64| {
            if let Some(current) = lock(&self.live).current.get_mut(&reader) {
                current.done = done;
                current.total = total;
            }
        };
        match self.action {
            BatchAction::Create(options) => EntryOutcome::Created(ops::create_reference(
                &file.path,
                options,
                self.db,
                self.control,
                &mut progress,
            )),
            BatchAction::Verify(options) => EntryOutcome::Verified(match &file.listed {
                // From a hash file the user opened: that line is the
                // reference; the file's own hash files are not consulted.
                Some(listed) => ops::verify_against(
                    &file.path,
                    &VerifyOptions {
                        use_sidecar: false,
                        follow_moves: options.follow_moves,
                    },
                    vec![listed.source()],
                    self.db,
                    self.control,
                    &mut progress,
                ),
                None => ops::verify_file(&file.path, options, self.db, self.control, &mut progress),
            }),
            BatchAction::Search => {
                let store = self.db.expect("checked before the batch").store;
                EntryOutcome::Searched(search::search_file(
                    &file.path,
                    store,
                    self.control,
                    &mut progress,
                ))
            }
        }
    }

    fn entry(&self, index: usize, outcome: EntryOutcome) -> BatchEntry {
        BatchEntry {
            index: index as u32,
            path: self.plan.files[index].text.clone(),
            outcome,
        }
    }

    fn skipped(&self, index: usize, reason: SkipReason, problem: Option<Problem>) -> BatchEntry {
        self.entry(index, EntryOutcome::Skipped(skip(reason, None, problem)))
    }
}

fn skip(reason: SkipReason, same_as: Option<DuplicateOf>, problem: Option<Problem>) -> Skip {
    Skip {
        reason,
        same_as,
        problem,
    }
}

/// After a failed read: is it the whole volume that went away?
fn volume_gone(path: &Path) -> bool {
    faults::check(FaultPoint::VolumeGone, path).is_err() || !platform::volume_available(path)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Gathers the readers' results on the calling thread, measures the
/// progress and hands updates over.
struct Collector<'c, 'a> {
    batch: &'c Batch<'a>,
    sink: &'c mut dyn FnMut(BatchUpdate),
    start: Instant,
    last_sent: Instant,
    meter: ThroughputMeter,
    was_paused: bool,
    pending: Vec<BatchEntry>,
    done: Vec<bool>,
    files_done: u32,
    bytes_finished: u64,
    summary: BatchSummary,
    statuses: HashMap<String, u32>,
    /// Creating with folder hash lists (`write_sums`).
    lists: Option<Lists>,
}

/// The folder hash lists of a batch: each list is written once, when every
/// file of its folder in the batch has its result. Until then, the files
/// whose line waits are shown as such, and counted once the list is
/// written, with their final status.
struct Lists {
    /// The folder of each file of the list.
    folder_of: Vec<usize>,
    /// Files of each folder still to come.
    left: Vec<usize>,
    /// Entries waiting for their folder's list.
    waiting: HashMap<usize, Vec<BatchEntry>>,
}

impl Lists {
    fn new(plan: &Plan) -> Lists {
        let mut ids: HashMap<String, usize> = HashMap::new();
        let mut left: Vec<usize> = Vec::new();
        let folder_of = plan
            .files
            .iter()
            .map(|file| {
                let key = file
                    .path
                    .parent()
                    .map(|dir| platform::path_key(&dir.display().to_string()))
                    .unwrap_or_default();
                let next = ids.len();
                let id = *ids.entry(key).or_insert(next);
                if id == left.len() {
                    left.push(0);
                }
                left[id] += 1;
                id
            })
            .collect();
        Lists {
            folder_of,
            left,
            waiting: HashMap::new(),
        }
    }
}

impl<'c, 'a> Collector<'c, 'a> {
    fn new(batch: &'c Batch<'a>, sink: &'c mut dyn FnMut(BatchUpdate)) -> Collector<'c, 'a> {
        let start = Instant::now();
        Collector {
            batch,
            sink,
            start,
            last_sent: start,
            meter: ThroughputMeter::new(start),
            was_paused: false,
            pending: Vec::new(),
            done: vec![false; batch.plan.files.len()],
            files_done: 0,
            bytes_finished: 0,
            summary: BatchSummary {
                total: batch.plan.files.len() as u32,
                ..BatchSummary::default()
            },
            statuses: HashMap::new(),
            lists: match batch.action {
                BatchAction::Create(options) if options.write_sums => Some(Lists::new(batch.plan)),
                _ => None,
            },
        }
    }

    fn collect(mut self, receiver: &mpsc::Receiver<Report>) -> BatchSummary {
        loop {
            match receiver.recv_timeout(TICK) {
                Ok(Report::Entry(entry)) => self.add(*entry),
                Ok(Report::VolumeLost(root)) => {
                    if !self.summary.lost_volumes.contains(&root) {
                        self.summary.lost_volumes.push(root);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.pending.len() >= MAX_ENTRIES || self.last_sent.elapsed() >= TICK {
                self.send(None);
            }
        }
        // Whatever no reader reached: cancelled, or after the database was
        // lost. Every file of the list ends with one entry.
        let database_problem = self.batch.schedule.lock().database_problem.clone();
        for index in 0..self.done.len() {
            if !self.done[index] {
                let entry = match &database_problem {
                    Some(problem) => self.batch.skipped(
                        index,
                        SkipReason::DatabaseUnavailable,
                        Some(problem.clone()),
                    ),
                    None => self.batch.skipped(index, SkipReason::Cancelled, None),
                };
                self.add(entry);
            }
        }
        // Every file has its entry, so every list is written by now; in
        // case one was not, its lines are not left waiting.
        let left: Vec<usize> = self
            .lists
            .as_ref()
            .map(|lists| lists.waiting.keys().copied().collect())
            .unwrap_or_default();
        for folder in left {
            self.write_list(folder);
        }
        let mut statuses: Vec<StatusCount> = std::mem::take(&mut self.statuses)
            .into_iter()
            .map(|(status, count)| StatusCount { status, count })
            .collect();
        statuses.sort_by(|a, b| a.status.cmp(&b.status));
        let mut summary = std::mem::take(&mut self.summary);
        summary.statuses = statuses;
        summary.stopped_by_user = self.batch.control.is_cancelled();
        summary.database_problem = database_problem;
        summary.elapsed_ms = self.start.elapsed().as_millis() as u64;
        self.send(Some(summary.clone()));
        summary
    }

    fn add(&mut self, entry: BatchEntry) {
        let index = entry.index as usize;
        if std::mem::replace(&mut self.done[index], true) {
            // Cannot happen: each file is taken once. Never counted twice.
            tracing::error!(index, "two results for one file");
            return;
        }
        self.files_done += 1;
        self.bytes_finished += self.batch.plan.files[index].size;
        let Some(lists) = self.lists.as_mut() else {
            self.publish(entry);
            return;
        };
        let folder = lists.folder_of[index];
        lists.left[folder] -= 1;
        let complete = lists.left[folder] == 0;
        if matches!(&entry.outcome, EntryOutcome::Created(report) if report.status == CreateStatus::Pending)
        {
            // Shown at once as waiting; counted when its list is written.
            self.pending.push(entry.clone());
            lists.waiting.entry(folder).or_default().push(entry);
        } else {
            self.publish(entry);
        }
        if complete {
            self.write_list(folder);
        }
    }

    /// Writes the hash lists of a folder whose files are all done, then
    /// hands over the final entries of the files that waited for them.
    fn write_list(&mut self, folder: usize) {
        let Some(entries) = self
            .lists
            .as_mut()
            .and_then(|lists| lists.waiting.remove(&folder))
        else {
            return;
        };
        let mut places = Vec::with_capacity(entries.len());
        let mut reports = Vec::with_capacity(entries.len());
        for entry in entries {
            if let EntryOutcome::Created(report) = entry.outcome {
                places.push((entry.index, entry.path));
                reports.push(report);
            }
        }
        ops::finish_sums(&mut reports);
        for ((index, path), report) in places.into_iter().zip(reports) {
            self.publish(BatchEntry {
                index,
                path,
                outcome: EntryOutcome::Created(report),
            });
        }
    }

    /// An entry with its final status: counted, and handed over.
    fn publish(&mut self, entry: BatchEntry) {
        match entry.outcome.category() {
            Category::Processed => self.summary.processed += 1,
            Category::Failed => self.summary.failed += 1,
            Category::Cancelled => self.summary.cancelled += 1,
            Category::NotProcessed => self.summary.not_processed += 1,
            Category::Duplicate => self.summary.duplicates += 1,
        }
        *self
            .statuses
            .entry(entry.outcome.status_code())
            .or_default() += 1;
        self.pending.push(entry);
    }

    fn send(&mut self, summary: Option<BatchSummary>) {
        let now = Instant::now();
        self.last_sent = now;
        let mut current: Vec<CurrentFile> =
            lock(&self.batch.live).current.values().cloned().collect();
        current.sort_by_key(|c| c.index);
        let bytes_total = self.batch.plan.bytes;
        let reading: u64 = current.iter().map(|c| c.done).sum();
        let bytes_done = (self.bytes_finished + reading).min(bytes_total);
        let paused = self.batch.control.is_paused();
        if paused {
            // Time spent paused says nothing about the speed.
        } else if self.was_paused {
            self.meter.restart(now, bytes_done);
        } else {
            self.meter.record(now, bytes_done);
        }
        self.was_paused = paused;
        let progress = BatchProgress {
            files_done: self.files_done,
            files_total: self.done.len() as u32,
            bytes_done,
            bytes_total,
            elapsed_ms: now.duration_since(self.start).as_millis() as u64,
            bytes_per_second: self.meter.bytes_per_second(),
            remaining_ms: self
                .meter
                .remaining(now, bytes_done, bytes_total - bytes_done)
                .map(|d| d.as_millis() as u64),
            paused,
            current: if summary.is_some() {
                Vec::new()
            } else {
                current
            },
        };
        (self.sink)(BatchUpdate {
            entries: std::mem::take(&mut self.pending),
            progress,
            summary,
        });
    }
}

#[cfg(test)]
mod tests;
