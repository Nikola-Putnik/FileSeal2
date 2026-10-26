//! Commands called by the interface. They translate between the UI and
//! `fileseal-core`; no business rule lives here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fileseal_core::batch::{self, BatchFile, BatchOptions, BatchSummary, BatchUpdate};
use fileseal_core::discovery::{self, DiscoveryBatch, DiscoveryOptions, DiscoverySummary};
use fileseal_core::ops::decide::{self, AcceptReport, AttachReport, Observation};
use fileseal_core::ops::manifest::{self, ListedFiles};
use fileseal_core::ops::{self, CreateOptions, CreateReport, CreateStatus, DbTarget};
use fileseal_core::progress::{ProgressReporter, ProgressSnapshot};
use fileseal_core::report::{self, ReportEntry};
use fileseal_core::settings::RecentDatabaseStatus;
use fileseal_core::store::{OperationKind, OperationStatus, SharedStore};
use fileseal_core::{Control, DatabaseInfo, Problem, ProblemKind, Store, platform, reader};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::settings::SettingsState;

#[derive(Default)]
pub struct AppState {
    /// The active database. Locked by the running job for its whole length.
    pub(crate) db: Arc<Mutex<Option<Store>>>,
    /// The running job, if any: one at a time.
    job: Mutex<Option<Arc<Control>>>,
    /// Results of the last operation, for the report export.
    last_report: Mutex<Vec<ReportEntry>>,
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic in a job must not make the application unusable.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    fn start_job(&self) -> Result<Arc<Control>, Problem> {
        let mut job = lock(&self.job);
        if job.is_some() {
            return Err(busy());
        }
        let control = Arc::new(Control::new());
        *job = Some(control.clone());
        Ok(control)
    }

    fn end_job(&self) {
        *lock(&self.job) = None;
    }

    /// An operation is running: closing the window must be confirmed.
    pub fn is_busy(&self) -> bool {
        lock(&self.job).is_some()
    }

    pub(crate) fn ensure_idle(&self) -> Result<(), Problem> {
        if lock(&self.job).is_some() {
            return Err(busy());
        }
        Ok(())
    }

    /// Quitting: the database is closed properly, its journal (`-wal`)
    /// merged back, so that the `.fileseal` file alone is complete. Tauri
    /// ends the process without dropping the state. Left to the next
    /// opening if a job still holds it.
    pub fn close_database(&self) {
        if let Ok(mut db) = self.db.try_lock() {
            *db = None;
        }
    }

    fn remember<T>(
        &self,
        result: &Result<T, Problem>,
        entries: impl FnOnce(&T) -> Vec<ReportEntry>,
    ) {
        if let Ok(report) = result {
            *lock(&self.last_report) = entries(report);
        }
    }
}

fn busy() -> Problem {
    Problem::new(ProblemKind::Busy, "an operation is already running")
}

/// Runs blocking work off the async runtime.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Problem> + Send + 'static,
) -> Result<T, Problem> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| Problem::new(ProblemKind::Io, format!("internal error: {err}")))?
}

#[tauri::command]
pub async fn database_create(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    path: String,
    name: String,
) -> Result<DatabaseInfo, Problem> {
    state.ensure_idle()?;
    let db = state.db.clone();
    let info = blocking(move || {
        let store = Store::create(Path::new(&path), &name)?;
        let info = store.info()?;
        *lock(&db) = Some(store);
        Ok(info)
    })
    .await?;
    settings.update_quietly(|s| s.remember_database(&info.path, &info.name));
    Ok(info)
}

#[tauri::command]
pub async fn database_open(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    path: String,
) -> Result<DatabaseInfo, Problem> {
    state.ensure_idle()?;
    let info = open_database(&state, path).await?;
    settings.update_quietly(|s| s.remember_database(&info.path, &info.name));
    Ok(info)
}

async fn open_database(state: &AppState, path: String) -> Result<DatabaseInfo, Problem> {
    let db = state.db.clone();
    blocking(move || {
        let mut guard = lock(&db);
        // Close the current one first: reopening the same file would
        // otherwise be refused as "in use" by ourselves.
        *guard = None;
        let store = Store::open(Path::new(&path))?;
        let info = store.info()?;
        *guard = Some(store);
        Ok(info)
    })
    .await
}

/// Reopens the database that was active when FileSeal was last closed.
/// `None` if there was none; an error (shown, not fatal) if it cannot be
/// opened now, e.g. on an unplugged drive. It stays in the recent list.
#[tauri::command]
pub async fn database_reopen_last(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
) -> Result<Option<DatabaseInfo>, Problem> {
    state.ensure_idle()?;
    if lock(&state.db).is_some() {
        return lock(&state.db).as_ref().map(Store::info).transpose();
    }
    let Some(path) = settings.snapshot().last_database else {
        return Ok(None);
    };
    let info = open_database(&state, path).await?;
    settings.update_quietly(|s| s.remember_database(&info.path, &info.name));
    Ok(Some(info))
}

#[tauri::command]
pub async fn database_close(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
) -> Result<(), Problem> {
    state.ensure_idle()?;
    *lock(&state.db) = None;
    // Closed on purpose: not reopened at the next start.
    settings.update_quietly(|s| s.last_database = None);
    Ok(())
}

#[tauri::command]
pub async fn database_recent(
    settings: State<'_, SettingsState>,
) -> Result<Vec<RecentDatabaseStatus>, Problem> {
    let settings = settings.snapshot();
    blocking(move || Ok(settings.recent_status())).await
}

/// Removes a database from the recent list; the file is not touched.
#[tauri::command]
pub async fn database_forget(
    settings: State<'_, SettingsState>,
    path: String,
) -> Result<Vec<RecentDatabaseStatus>, Problem> {
    let settings = settings.update(|s| s.forget_database(&path))?;
    blocking(move || Ok(settings.recent_status())).await
}

#[tauri::command]
pub async fn database_current(state: State<'_, AppState>) -> Result<Option<DatabaseInfo>, Problem> {
    state.ensure_idle()?;
    lock(&state.db).as_ref().map(Store::info).transpose()
}

#[tauri::command]
pub async fn create_reference(
    state: State<'_, AppState>,
    path: String,
    options: CreateOptions,
    use_database: bool,
    on_progress: Channel<ProgressSnapshot>,
) -> Result<CreateReport, Problem> {
    let control = state.start_job()?;
    let db = state.db.clone();
    let result = blocking(move || {
        let _awake = platform::keep_awake();
        let mut guard = lock(&db);
        let store = database_for(&mut guard, use_database)?;
        run_on_file(
            store,
            OperationKind::Create,
            &path,
            &on_progress,
            |file, target, reporter| {
                let mut report = ops::create_reference(file, &options, target, &control, reporter);
                // One file: its line goes to its folder's hash list at once.
                ops::finish_sums(std::slice::from_mut(&mut report));
                let status = match report.status {
                    CreateStatus::Cancelled => OperationStatus::Cancelled,
                    CreateStatus::Failed => OperationStatus::Failed,
                    _ => OperationStatus::Completed,
                };
                (report.size, status, report)
            },
        )
    })
    .await;
    state.end_job();
    state.remember(&result, |r| vec![ReportEntry::Create(r.clone())]);
    result
}

/// Runs one action on a list of files (cadrage §2, n° 14 to 17): one
/// reader per volume, results and progress sent as they come. The last
/// update carries the summary. Cancelled by `cancel_job`, paused by
/// `pause_job`.
#[tauri::command]
pub async fn run_batch(
    state: State<'_, AppState>,
    files: Vec<BatchFile>,
    options: BatchOptions,
    use_database: bool,
    on_update: Channel<BatchUpdate>,
) -> Result<BatchSummary, Problem> {
    let control = state.start_job()?;
    let db = state.db.clone();
    let action = options.action.clone();
    let result = blocking(move || {
        let mut guard = lock(&db);
        let store = database_for(&mut guard, use_database)?;
        // By position: a file waiting for its folder's hash list has a
        // first entry, then its final one, which replaces it.
        let mut entries = BTreeMap::new();
        let summary = batch::run(&files, &options, store, &control, &mut |update| {
            for entry in &update.entries {
                entries.insert(entry.index, entry.clone());
            }
            // The window may have been closed: nothing to do about it here.
            let _ = on_update.send(update);
        })?;
        Ok((summary, entries.into_values().collect::<Vec<_>>()))
    })
    .await;
    state.end_job();
    state.remember(&result, |(_, entries)| {
        entries
            .iter()
            .map(|entry| batch::report_entry(&action, entry))
            .collect()
    });
    result.map(|(summary, _)| summary)
}

/// The files a hash file lists, each with its line, to verify them as a
/// batch (like `sha256sum -c`).
#[tauri::command]
pub async fn expand_hash_file(path: String) -> Result<ListedFiles, Problem> {
    blocking(move || manifest::list(Path::new(&path))).await
}

/// Opens Explorer on the folder of a file, with the file selected.
#[tauri::command]
pub async fn reveal_in_folder(path: String) -> Result<(), Problem> {
    platform::reveal(Path::new(&path)).map_err(|err| Problem::from_io(&err))
}

/// The page of FileSeal's latest release, in the browser: where the
/// portable version is downloaded, and every version's notes. A fixed
/// address, never one given by the page.
#[tauri::command]
pub async fn open_release_page() -> Result<(), Problem> {
    platform::open_web_page(RELEASES).map_err(|err| Problem::from_io(&err))
}

const RELEASES: &str = "https://github.com/Nikola-Putnik/FileSeal2/releases/latest";

/// Lists the files that the chosen files and folders stand for, sending
/// them in batches as they are found. Cancelled by `cancel_job`.
#[tauri::command]
pub async fn discover_selection(
    state: State<'_, AppState>,
    paths: Vec<String>,
    options: DiscoveryOptions,
    on_batch: Channel<DiscoveryBatch>,
) -> Result<DiscoverySummary, Problem> {
    let control = state.start_job()?;
    let result = blocking(move || {
        let roots: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        Ok(discovery::discover(
            &roots,
            &options,
            &control,
            &mut |batch| {
                // The window may have been closed: nothing to do about it here.
                let _ = on_batch.send(batch);
            },
        ))
    })
    .await;
    state.end_job();
    result
}

/// R8: records that a known item is now the file at `path`.
#[tauri::command]
pub async fn attach_file(
    state: State<'_, AppState>,
    path: String,
    item_id: i64,
    observation: Observation,
    allow_different: bool,
) -> Result<AttachReport, Problem> {
    state.ensure_idle()?;
    let db = state.db.clone();
    blocking(move || {
        let mut guard = lock(&db);
        let store = guard.as_mut().ok_or_else(no_database)?;
        decide::attach(
            Path::new(&path),
            item_id,
            &observation,
            allow_different,
            store,
        )
    })
    .await
}

/// Accepts the content a verification observed as the new reference.
#[tauri::command]
pub async fn accept_version(
    state: State<'_, AppState>,
    path: String,
    observation: Observation,
    verification_id: Option<i64>,
    update_sidecars: bool,
) -> Result<AcceptReport, Problem> {
    state.ensure_idle()?;
    let db = state.db.clone();
    blocking(move || {
        let mut guard = lock(&db);
        decide::accept(
            Path::new(&path),
            &observation,
            verification_id,
            update_sidecars,
            guard.as_mut(),
        )
    })
    .await
}

/// Writes the results of the last operation as CSV.
#[tauri::command]
pub async fn export_report(
    state: State<'_, AppState>,
    path: String,
    separator: char,
) -> Result<usize, Problem> {
    let entries = lock(&state.last_report).clone();
    blocking(move || {
        report::write_csv(Path::new(&path), &entries, separator)?;
        Ok(entries.len())
    })
    .await
}

fn no_database() -> Problem {
    Problem::new(ProblemKind::NotFound, "no database is open")
}

#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>) -> Result<(), Problem> {
    if let Some(control) = lock(&state.job).as_ref() {
        control.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn pause_job(state: State<'_, AppState>) -> Result<(), Problem> {
    if let Some(control) = lock(&state.job).as_ref() {
        control.pause();
    }
    Ok(())
}

#[tauri::command]
pub async fn resume_job(state: State<'_, AppState>) -> Result<(), Problem> {
    if let Some(control) = lock(&state.job).as_ref() {
        control.resume();
    }
    Ok(())
}

/// The user confirmed closing during an operation: it is cancelled, which
/// takes effect at its next block (nothing half-written is ever published),
/// then the application quits.
#[tauri::command]
pub async fn close_confirmed(app: AppHandle, state: State<'_, AppState>) -> Result<(), Problem> {
    if let Some(control) = lock(&state.job).as_ref() {
        control.cancel();
    }
    for _ in 0..100 {
        if !state.is_busy() {
            break;
        }
        sleep_off_runtime(std::time::Duration::from_millis(50)).await;
    }
    tracing::info!(busy = state.is_busy(), "closing during an operation");
    state.close_database();
    app.exit(0);
    Ok(())
}

async fn sleep_off_runtime(duration: std::time::Duration) {
    // Off the async runtime's threads, like any wait here.
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(duration)).await;
}

fn database_for(
    guard: &mut Option<Store>,
    use_database: bool,
) -> Result<Option<&mut Store>, Problem> {
    if !use_database {
        return Ok(None);
    }
    guard
        .as_mut()
        .map(Some)
        .ok_or_else(|| Problem::new(ProblemKind::NotFound, "no database is open"))
}

/// One file, as one operation of the database (if any), with progress.
fn run_on_file<R>(
    mut store: Option<&mut Store>,
    kind: OperationKind,
    path: &str,
    on_progress: &Channel<ProgressSnapshot>,
    action: impl FnOnce(
        &Path,
        Option<DbTarget>,
        &mut dyn FnMut(u64, u64),
    ) -> (Option<u64>, OperationStatus, R),
) -> Result<R, Problem> {
    let file = PathBuf::from(path);
    let operation_id = match store.as_deref_mut() {
        Some(store) => Some(store.begin_operation(kind)?),
        None => None,
    };
    let total = reader::file_size(&file).unwrap_or(0);
    let mut reporter = ProgressReporter::new(1, total, |snapshot| {
        // The window may have been closed: nothing to do about it here.
        let _ = on_progress.send(snapshot.clone());
    });
    let (size, status, report) = {
        let shared = store.as_deref_mut().map(SharedStore::new);
        let target = match (shared.as_ref(), operation_id) {
            (Some(store), Some(operation_id)) => Some(DbTarget {
                store,
                operation_id,
            }),
            _ => None,
        };
        action(&file, target, &mut |done, total| {
            reporter.file_progress(path, done, total)
        })
    };
    reporter.file_finished(size.unwrap_or(0));
    if let (Some(store), Some(id)) = (store, operation_id)
        && let Err(problem) = store.finish_operation(id, status, None)
    {
        tracing::warn!(%problem, "operation not closed in the database");
    }
    Ok(report)
}
