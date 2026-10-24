//! Application settings (`settings.json` in the user's configuration
//! folder): interface language, recent databases, the database to reopen,
//! the choices made for batches. A missing or damaged file gives the
//! defaults rather than an error: settings are a convenience, never a
//! reason not to start.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::algorithm::Algorithm;
use crate::ops::ExistingReferences;
use crate::platform;
use crate::problem::Problem;

/// How many databases the "recent" list keeps.
pub const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    /// Interface language (`"en"`, `"fr"`); `None` = default (English).
    pub language: Option<String>,
    /// Most recently used first.
    pub recent_databases: Vec<RecentDatabase>,
    /// The database that was active when FileSeal was last used, reopened
    /// at start-up. `None` once the user closes it.
    pub last_database: Option<String>,
    /// The choices of the next batch, as last made.
    pub run: RunPreferences,
}

/// The choices made in the interface for batches, kept from one session to
/// the next. Each one is optional: missing (an older file, a choice added
/// later), the interface's default applies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct RunPreferences {
    pub algorithms: Option<Vec<Algorithm>>,
    pub write_sidecar: Option<bool>,
    pub use_sidecar: Option<bool>,
    pub use_database: Option<bool>,
    pub follow_moves: Option<bool>,
    pub recursive: Option<bool>,
    pub include_hidden: Option<bool>,
    pub parallel_volumes: Option<u8>,
    pub existing: Option<ExistingReferences>,
    pub import_hash_files: Option<bool>,
    /// "Only these extensions", lowercase, without the dot.
    pub only_extensions: Option<Vec<String>>,
    /// The settings panel left open.
    pub settings_open: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentDatabase {
    pub path: String,
    pub name: String,
    pub last_opened: String,
}

/// A recent database as shown in the list: does its file still exist?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentDatabaseStatus {
    pub path: String,
    pub name: String,
    pub last_opened: String,
    /// False when the file is gone (moved, deleted, drive unplugged).
    pub exists: bool,
}

impl Settings {
    /// The recent list, each entry checked on disk.
    pub fn recent_status(&self) -> Vec<RecentDatabaseStatus> {
        self.recent_databases
            .iter()
            .map(|r| RecentDatabaseStatus {
                exists: Path::new(&r.path).is_file(),
                path: r.path.clone(),
                name: r.name.clone(),
                last_opened: r.last_opened.clone(),
            })
            .collect()
    }

    pub fn load(path: &Path) -> Settings {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
            Err(err) => {
                tracing::warn!(path = %path.display(), %err, "settings unreadable, defaults used");
                return Settings::default();
            }
        };
        // A BOM, as some editors add when the file is edited by hand, is
        // not JSON: skipped rather than costing all the settings.
        serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_else(|err| {
            tracing::warn!(path = %path.display(), %err, "settings damaged, defaults used");
            Settings::default()
        })
    }

    /// Written to a temporary file, then renamed over the old one: an
    /// interrupted save leaves the previous settings, not half a file.
    pub fn save(&self, path: &Path) -> Result<(), Problem> {
        let io_problem = |err: std::io::Error| Problem::from_io(&err);
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(io_problem)?;
        let mut temp = tempfile::Builder::new()
            .prefix(".settings-")
            .suffix(".tmp")
            .tempfile_in(platform::extended(dir))
            .map_err(io_problem)?;
        let json = serde_json::to_string_pretty(self).expect("settings always serialise");
        temp.write_all(json.as_bytes()).map_err(io_problem)?;
        temp.as_file().sync_all().map_err(io_problem)?;
        temp.persist(platform::extended(path))
            .map_err(|err| Problem::from_io(&err.error))?;
        Ok(())
    }

    /// A database was opened or created: first in the list (once), and the
    /// one to reopen next time.
    pub fn remember_database(&mut self, path: &str, name: &str) {
        self.forget_entry(path);
        self.recent_databases.insert(
            0,
            RecentDatabase {
                path: path.to_owned(),
                name: name.to_owned(),
                last_opened: crate::timestamp::now(),
            },
        );
        self.recent_databases.truncate(MAX_RECENT);
        self.last_database = Some(path.to_owned());
    }

    /// Removes a database from the list (the file itself is not touched).
    pub fn forget_database(&mut self, path: &str) {
        self.forget_entry(path);
        if self
            .last_database
            .as_deref()
            .is_some_and(|last| same_path(last, path))
        {
            self.last_database = None;
        }
    }

    fn forget_entry(&mut self, path: &str) {
        self.recent_databases.retain(|r| !same_path(&r.path, path));
    }

    /// Portable FileSeal, as written to its file: the databases on the
    /// drive it runs from (`E:`) are kept without that letter, so they are
    /// found again when the USB key gets another letter on another PC.
    pub fn without_drive(&self, drive: &str) -> Settings {
        self.clone().map_paths(|path| {
            let on_drive = path
                .get(..drive.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(drive))
                && path[drive.len()..].starts_with('\\');
            if on_drive {
                path[drive.len()..].to_owned()
            } else {
                path.to_owned()
            }
        })
    }

    /// As read back: a path without a drive (`\Bases\x.fileseal`, not a
    /// network path) is on the drive FileSeal runs from now.
    pub fn with_drive(self, drive: &str) -> Settings {
        self.map_paths(|path| {
            if path.starts_with('\\') && !path.starts_with(r"\\") {
                format!("{drive}{path}")
            } else {
                path.to_owned()
            }
        })
    }

    fn map_paths(mut self, map: impl Fn(&str) -> String) -> Settings {
        for recent in &mut self.recent_databases {
            recent.path = map(&recent.path);
        }
        self.last_database = self.last_database.as_deref().map(map);
        self
    }
}

/// What the interface shows about FileSeal itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    /// Run with a `portable.flag` beside the program: everything it writes
    /// is kept in a folder there, nothing in the user's profile.
    pub portable: bool,
    /// Where a new database is proposed: beside the program when portable
    /// (on the USB key), else `None` (the Documents folder).
    pub database_folder: Option<String>,
}

/// Paths compared the way the file system does (case-insensitive on Windows).
fn same_path(a: &str, b: &str) -> bool {
    platform::path_key(a) == platform::path_key(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_damaged_settings_give_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn saved_settings_are_read_back_and_unknown_fields_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config").join("settings.json");
        let mut settings = Settings {
            language: Some("fr".to_owned()),
            ..Settings::default()
        };
        settings.remember_database(r"C:\Bases\Films.fileseal", "Films");
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        // A newer version may have added fields: they must not break loading,
        // and an older file without the newer fields gets their defaults.
        std::fs::write(&path, r#"{"language":"en","futureField":42}"#).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.language.as_deref(), Some("en"));
        assert!(loaded.recent_databases.is_empty());
        // Edited by hand and saved with a BOM.
        std::fs::write(&path, "\u{feff}{\"language\":\"fr\"}").unwrap();
        assert_eq!(Settings::load(&path).language.as_deref(), Some("fr"));
    }

    #[test]
    fn the_choices_of_batches_are_kept_and_missing_ones_stay_unset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let settings = Settings {
            run: RunPreferences {
                algorithms: Some(vec![Algorithm::Md5]),
                existing: Some(ExistingReferences::SkipAny),
                only_extensions: Some(vec!["mkv".to_owned(), "iso".to_owned()]),
                settings_open: Some(true),
                ..RunPreferences::default()
            },
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        // A file from 2.2 or before has none: the interface's defaults apply.
        std::fs::write(&path, r#"{"language":"fr","run":{"writeSidecar":false}}"#).unwrap();
        let older = Settings::load(&path).run;
        assert_eq!(older.write_sidecar, Some(false));
        assert_eq!((older.algorithms, older.existing), (None, None));
    }

    #[test]
    fn recent_databases_most_recent_first_without_duplicates() {
        let mut settings = Settings::default();
        for i in 0..12 {
            settings.remember_database(&format!(r"C:\Bases\{i}.fileseal"), &i.to_string());
        }
        assert_eq!(settings.recent_databases.len(), MAX_RECENT);
        assert_eq!(settings.recent_databases[0].name, "11");
        settings.remember_database(r"c:\bases\5.FILESEAL", "5 again");
        assert_eq!(settings.recent_databases[0].name, "5 again");
        let fives = settings
            .recent_databases
            .iter()
            .filter(|r| r.name.starts_with('5'))
            .count();
        if cfg!(windows) {
            assert_eq!(fives, 1, "same file whatever the case");
        }
        assert_eq!(
            settings.last_database.as_deref(),
            Some(r"c:\bases\5.FILESEAL")
        );

        settings.forget_database(r"c:\bases\5.FILESEAL");
        assert!(settings.last_database.is_none());
        assert!(
            !settings
                .recent_databases
                .iter()
                .any(|r| r.name == "5 again")
        );
    }

    #[test]
    fn a_portable_fileseal_finds_its_databases_under_another_drive_letter() {
        let mut settings = Settings::default();
        settings.remember_database(r"\\nas\bases\Photos.fileseal", "Photos");
        settings.remember_database(r"C:\Bases\Films.fileseal", "Films");
        settings.remember_database(r"e:\FileSeal\Key.fileseal", "Key");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // Run from E:, saved without that letter.
        settings.without_drive("E:").save(&path).unwrap();
        let stored = Settings::load(&path);
        assert_eq!(
            stored.last_database.as_deref(),
            Some(r"\FileSeal\Key.fileseal")
        );
        // The key is F: on another PC: its database is there; the others,
        // on other drives or the network, are left as they were.
        let paths: Vec<String> = stored
            .with_drive("F:")
            .recent_databases
            .into_iter()
            .map(|r| r.path)
            .collect();
        assert_eq!(
            paths,
            [
                r"F:\FileSeal\Key.fileseal",
                r"C:\Bases\Films.fileseal",
                r"\\nas\bases\Photos.fileseal"
            ]
        );
    }
}
