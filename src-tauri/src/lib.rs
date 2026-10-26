//! Tauri shell: exposes `fileseal-core` to the interface and nothing more.
//! Every command is `async` (a synchronous command would run on the main
//! thread and freeze the window); long work runs on a blocking thread and
//! reports progress through a `Channel`.

mod commands;
mod databases;
mod portable;
mod settings;

use std::path::PathBuf;

use fileseal_core::settings::AppInfo;
use tauri::{Emitter, Manager, RunEvent, WebviewWindowBuilder, WindowEvent};
use tracing_appender::non_blocking::WorkerGuard;

pub fn run() {
    let mode = portable::Mode::detect();
    tauri::Builder::default()
        // Registered first, as the plugin requires: a second launch brings
        // the open window forward instead of opening the database twice.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        // New versions: the latest release on GitHub, its installer signed
        // with the project's key (tauri.conf.json, plugins.updater).
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::AppState::default())
        // Closing during an operation is confirmed in the page first.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.state::<commands::AppState>().is_busy()
            {
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .setup(move |app| {
            let portable = mode.portable();
            let log_dir = match portable {
                Some(portable) => Some(portable.data_dir.join("logs")),
                None => app.path().app_log_dir().ok(),
            };
            if let Some(guard) = log_dir.and_then(init_logging) {
                app.manage(LogGuard(guard));
            }
            tracing::info!(version = fileseal_core::APP_VERSION, "FileSeal started");
            match &mode {
                portable::Mode::Portable(portable) => {
                    tracing::info!(data = %portable.data_dir.display(), "portable");
                }
                portable::Mode::ReadOnly(dir) => {
                    tracing::warn!(data = %dir.display(), "portable, but not writable: profile used");
                }
                portable::Mode::Installed => {}
            }
            let info = AppInfo {
                version: fileseal_core::APP_VERSION.to_owned(),
                portable: portable.is_some(),
                database_folder: portable.map(|p| p.program_dir.display().to_string()),
            };
            app.manage(settings::SettingsState::load(
                match portable {
                    Some(portable) => Some(portable.data_dir.clone()),
                    None => app.path().app_config_dir().ok(),
                },
                portable.and_then(|p| p.drive.clone()),
                info,
            ));
            // Created here rather than from the configuration: a portable
            // FileSeal gives WebView2 a data folder beside the program.
            let config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .expect("a window in tauri.conf.json");
            let theme = app.state::<settings::SettingsState>().snapshot().theme;
            let mut window = WebviewWindowBuilder::from_config(app.handle(), &config)?
                .theme(settings::native_theme(theme.as_deref()));
            if let Some(portable) = portable {
                window = window.data_directory(portable.data_dir.join("webview"));
            }
            window.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            settings::settings_get,
            settings::settings_set_language,
            settings::settings_set_theme,
            settings::settings_set_check_updates,
            settings::settings_set_run,
            settings::app_info,
            commands::database_create,
            commands::database_open,
            commands::database_close,
            commands::database_current,
            commands::database_reopen_last,
            commands::database_recent,
            commands::database_forget,
            databases::database_rename,
            databases::database_move,
            databases::database_backup,
            databases::database_inspect,
            databases::database_restore,
            databases::database_volumes,
            databases::database_reassign_preview,
            databases::database_reassign,
            databases::database_browse,
            databases::database_matching,
            databases::database_remove,
            databases::database_put_back,
            databases::database_history,
            commands::create_reference,
            commands::discover_selection,
            commands::run_batch,
            commands::expand_hash_file,
            commands::reveal_in_folder,
            commands::open_release_page,
            commands::pause_job,
            commands::resume_job,
            commands::close_confirmed,
            commands::attach_file,
            commands::accept_version,
            commands::export_report,
            commands::cancel_job,
        ])
        .build(tauri::generate_context!())
        .expect("FileSeal could not start")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                app.state::<commands::AppState>().close_database();
            }
        });
}

/// Keeps the log writer alive (and flushed on exit).
struct LogGuard(#[allow(dead_code)] WorkerGuard);

/// Daily log files, 14 kept. Paths and errors only, never file content.
fn init_logging(dir: PathBuf) -> Option<WorkerGuard> {
    use tracing_appender::rolling::{Builder, Rotation};
    std::fs::create_dir_all(&dir).ok()?;
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("fileseal")
        .filename_suffix("log")
        .max_log_files(14)
        .build(&dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_env("FILESEAL_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(filter)
        .try_init()
        .ok()?;
    Some(guard)
}
