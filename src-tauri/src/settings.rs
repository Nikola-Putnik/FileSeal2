//! Settings commands: the interface reads and changes them here; the core
//! crate owns the file format.

use std::path::PathBuf;
use std::sync::Mutex;

use fileseal_core::Problem;
use fileseal_core::settings::{AppInfo, RunPreferences, Settings};
use tauri::State;

pub struct SettingsState {
    path: PathBuf,
    /// Portable FileSeal: the drive it runs from, whose letter is not
    /// written in the file (`Settings::without_drive`).
    drive: Option<String>,
    settings: Mutex<Settings>,
    info: AppInfo,
}

impl SettingsState {
    /// `FILESEAL_CONFIG_DIR` replaces the configuration folder, so a test
    /// instance never touches the real settings.
    pub fn load(dir: Option<PathBuf>, drive: Option<String>, info: AppInfo) -> SettingsState {
        let dir = std::env::var_os("FILESEAL_CONFIG_DIR")
            .map(PathBuf::from)
            .or(dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let path = dir.join("settings.json");
        tracing::info!(path = %path.display(), "settings");
        let mut settings = Settings::load(&path);
        if let Some(drive) = &drive {
            settings = settings.with_drive(drive);
        }
        SettingsState {
            settings: Mutex::new(settings),
            path,
            drive,
            info,
        }
    }

    /// Applies a change and saves it; in memory only if the file is saved.
    pub fn update(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, Problem> {
        let mut settings = self
            .settings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut changed = settings.clone();
        change(&mut changed);
        match &self.drive {
            Some(drive) => changed.without_drive(drive).save(&self.path)?,
            None => changed.save(&self.path)?,
        }
        *settings = changed.clone();
        Ok(changed)
    }

    /// Same, for changes that must not fail the action they follow (an
    /// unwritable settings file only costs the "recent" list).
    pub fn update_quietly(&self, change: impl FnOnce(&mut Settings)) {
        if let Err(problem) = self.update(change) {
            tracing::warn!(%problem, "settings not saved");
        }
    }

    pub fn snapshot(&self) -> Settings {
        self.settings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[tauri::command]
pub async fn settings_get(state: State<'_, SettingsState>) -> Result<Settings, Problem> {
    Ok(state.snapshot())
}

#[tauri::command]
pub async fn settings_set_language(
    state: State<'_, SettingsState>,
    language: Option<String>,
) -> Result<Settings, Problem> {
    state.update(|settings| settings.language = language)
}

/// The choices of the next batch, kept for the next session.
#[tauri::command]
pub async fn settings_set_run(
    state: State<'_, SettingsState>,
    run: RunPreferences,
) -> Result<Settings, Problem> {
    state.update(|settings| settings.run = run)
}

#[tauri::command]
pub async fn app_info(state: State<'_, SettingsState>) -> Result<AppInfo, Problem> {
    Ok(state.info.clone())
}
