//! Commands of the Databases screen: the active database's name and file
//! (move, backup, restore), its disks, its references and their history,
//! items removed and put back.
//! The rules are in `fileseal-core`; nothing here decides anything.

use std::path::Path;

use fileseal_core::ops::items::{self, ItemsChanged};
use fileseal_core::ops::volumes::{self, Reassign, ReassignPreview, ReassignReport, VolumeStatus};
use fileseal_core::store::files::{self, CopyReport, Inspection};
use fileseal_core::store::{ItemHistory, Listed, ReferencePage};
use fileseal_core::{DatabaseInfo, Problem, ProblemKind, Store};
use serde::Serialize;
use tauri::State;

use crate::commands::{AppState, blocking, lock};
use crate::settings::SettingsState;

/// Rows listed at most; the search narrows them down.
const BROWSE_LIMIT: u32 = 2_000;

fn no_database() -> Problem {
    Problem::new(ProblemKind::NotFound, "no database is open")
}

/// Runs `work` on the open database, off the async runtime.
async fn with_database<T: Send + 'static>(
    state: &AppState,
    work: impl FnOnce(&mut Store) -> Result<T, Problem> + Send + 'static,
) -> Result<T, Problem> {
    state.ensure_idle()?;
    let db = state.db.clone();
    blocking(move || {
        let mut guard = lock(&db);
        work(guard.as_mut().ok_or_else(no_database)?)
    })
    .await
}

#[tauri::command]
pub async fn database_rename(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    name: String,
) -> Result<DatabaseInfo, Problem> {
    let info = with_database(&state, move |store| {
        store.rename(&name)?;
        store.info()
    })
    .await?;
    settings.update_quietly(|s| s.remember_database(&info.path, &info.name));
    Ok(info)
}

/// Moves or renames the database file; the recent list follows.
#[tauri::command]
pub async fn database_move(
    state: State<'_, AppState>,
    settings: State<'_, SettingsState>,
    target: String,
) -> Result<DatabaseInfo, Problem> {
    state.ensure_idle()?;
    let db = state.db.clone();
    let (old, info) = blocking(move || {
        let mut slot = lock(&db);
        let old = slot
            .as_ref()
            .ok_or_else(no_database)?
            .path()
            .display()
            .to_string();
        files::move_to(&mut slot, Path::new(&target))?;
        let info = slot.as_ref().ok_or_else(no_database)?.info()?;
        Ok((old, info))
    })
    .await?;
    settings.update_quietly(|s| {
        s.forget_database(&old);
        s.remember_database(&info.path, &info.name);
    });
    Ok(info)
}

#[tauri::command]
pub async fn database_backup(
    state: State<'_, AppState>,
    target: String,
    replace: bool,
) -> Result<CopyReport, Problem> {
    with_database(&state, move |store| {
        files::back_up(store, Path::new(&target), replace)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    info: DatabaseInfo,
    /// The database as it was before the restore.
    safety_copy: CopyReport,
}

/// What a database file holds, read without changing it: a backup about
/// to be restored.
#[tauri::command]
pub async fn database_inspect(path: String) -> Result<Inspection, Problem> {
    blocking(move || Store::inspect(Path::new(&path))).await
}

#[tauri::command]
pub async fn database_restore(
    state: State<'_, AppState>,
    backup: String,
) -> Result<Restored, Problem> {
    state.ensure_idle()?;
    let db = state.db.clone();
    blocking(move || {
        let mut slot = lock(&db);
        let safety_copy = files::restore(&mut slot, Path::new(&backup))?;
        let info = slot.as_ref().ok_or_else(no_database)?.info()?;
        Ok(Restored { info, safety_copy })
    })
    .await
}

/// The disks of the database and where each one is now.
#[tauri::command]
pub async fn database_volumes(state: State<'_, AppState>) -> Result<Vec<VolumeStatus>, Problem> {
    with_database(&state, |store| volumes::volumes(store)).await
}

#[tauri::command]
pub async fn database_reassign_preview(
    state: State<'_, AppState>,
    request: Reassign,
) -> Result<ReassignPreview, Problem> {
    with_database(&state, move |store| volumes::preview(store, &request)).await
}

#[tauri::command]
pub async fn database_reassign(
    state: State<'_, AppState>,
    request: Reassign,
) -> Result<ReassignReport, Problem> {
    with_database(&state, move |store| volumes::reassign(store, &request)).await
}

#[tauri::command]
pub async fn database_browse(
    state: State<'_, AppState>,
    query: String,
    listed: Listed,
) -> Result<ReferencePage, Problem> {
    with_database(&state, move |store| {
        store.browse(&query, BROWSE_LIMIT, listed)
    })
    .await
}

/// Every item a search finds, beyond those listed.
#[tauri::command]
pub async fn database_matching(
    state: State<'_, AppState>,
    query: String,
    listed: Listed,
) -> Result<Vec<i64>, Problem> {
    with_database(&state, move |store| store.matching_items(&query, listed)).await
}

#[tauri::command]
pub async fn database_remove(
    state: State<'_, AppState>,
    item_ids: Vec<i64>,
) -> Result<ItemsChanged, Problem> {
    with_database(&state, move |store| items::remove(store, &item_ids)).await
}

#[tauri::command]
pub async fn database_put_back(
    state: State<'_, AppState>,
    item_ids: Vec<i64>,
) -> Result<ItemsChanged, Problem> {
    with_database(&state, move |store| items::put_back(store, &item_ids)).await
}

#[tauri::command]
pub async fn database_history(
    state: State<'_, AppState>,
    item_id: i64,
) -> Result<ItemHistory, Problem> {
    with_database(&state, move |store| store.item_history(item_id)).await
}
