//! The database file itself: moving or renaming it, backing it up and
//! restoring a backup. Each step that could lose data comes after a check:
//! the original is deleted only once its copy opened with the same
//! contents, and the database being replaced by a restore is copied first.
//!
//! Functions take the open database from the caller's slot and always put
//! one back: the new one, or the original if anything failed.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use super::{Counts, Store, db_problem, schema};
use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::timestamp;

/// What a backup or a restore did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CopyReport {
    /// The backup written, or the copy of the database made before it was
    /// replaced by a restore.
    pub path: String,
    #[ts(type = "number")]
    pub bytes: u64,
    pub counts: Counts,
}

/// A database file looked at without changing it (a backup to restore).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Inspection {
    pub name: String,
    pub schema_version: u32,
    pub counts: Counts,
}

impl Store {
    /// A consistent, compact copy of the database at `target` (SQLite
    /// `VACUUM INTO`). Written under a temporary name first: an interrupted
    /// copy never looks complete. An existing `target` is replaced only if
    /// `replace` (the user confirmed it in the save dialog).
    pub fn copy_to(&self, target: &Path, replace: bool) -> Result<u64, Problem> {
        let target = platform::absolute(target).map_err(|err| Problem::from_io(&err))?;
        if target == self.path {
            return Err(Problem::new(
                ProblemKind::AlreadyExists,
                "this is the database itself",
            ));
        }
        if !replace && target.exists() {
            return Err(Problem::new(
                ProblemKind::AlreadyExists,
                format!("{} already exists", target.display()),
            ));
        }
        let dir = target.parent().unwrap_or(Path::new("."));
        let needed = file_bytes(&self.path) + file_bytes(&schema::wal_of(&self.path)) + (16 << 20);
        if let Some(free) = platform::available_space(dir)
            && free < needed
        {
            return Err(Problem::new(
                ProblemKind::DiskFull,
                format!("{free} bytes free, about {needed} needed for the copy"),
            ));
        }
        let partial = partial_name(&target);
        let _ = std::fs::remove_file(&partial);
        let copied = self
            .conn
            .execute("VACUUM INTO ?1", [partial.to_string_lossy()])
            .map_err(db_problem)
            .and_then(|_| {
                let moved = if replace {
                    std::fs::rename(&partial, &target)
                } else {
                    platform::rename_noclobber(&partial, &target)
                };
                moved.map_err(|err| Problem::from_io(&err))
            });
        if let Err(problem) = copied {
            let _ = std::fs::remove_file(&partial);
            return Err(problem);
        }
        Ok(file_bytes(&target))
    }

    /// Opens a database file to look at it, without changing anything in
    /// it: not even a migration. Refuses what `open` would refuse.
    pub fn inspect(path: &Path) -> Result<Inspection, Problem> {
        let path = platform::absolute(path).map_err(|err| Problem::from_io(&err))?;
        if !path.is_file() {
            return Err(Problem::new(
                ProblemKind::NotFound,
                format!("{} does not exist", path.display()),
            ));
        }
        let (conn, _) = schema::connect_immutable(&path)?;
        if schema::application_id(&conn)? != super::APPLICATION_ID {
            return Err(Problem::new(
                ProblemKind::NotAFileSealDatabase,
                format!("{} is not a FileSeal database", path.display()),
            ));
        }
        let schema_version = schema::schema_version(&conn)?;
        if schema_version > super::SCHEMA_VERSION {
            return Err(Problem::new(
                ProblemKind::DatabaseTooNew,
                format!("schema version {schema_version}"),
            ));
        }
        schema::quick_check(&conn)?;
        let name: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'name'", [], |row| {
                row.get(0)
            })
            .map_err(db_problem)?;
        let count = |sql: &str| -> Result<u64, Problem> {
            conn.query_row(sql, [], |row| row.get::<_, i64>(0))
                .map(|n| n as u64)
                .map_err(db_problem)
        };
        let counts = Counts {
            items: count("SELECT count(*) FROM item")?,
            references: count("SELECT count(*) FROM reference")?,
            locations: count("SELECT count(*) FROM location")?,
            verifications: count("SELECT count(*) FROM verification")?,
        };
        Ok(Inspection {
            name,
            schema_version,
            counts,
        })
    }
}

/// Backs the database up to `target`, then checks the copy: it must open
/// and hold exactly what the database holds.
pub fn back_up(store: &Store, target: &Path, replace: bool) -> Result<CopyReport, Problem> {
    let counts = store.counts()?;
    let bytes = store.copy_to(target, replace)?;
    let target = platform::absolute(target).map_err(|err| Problem::from_io(&err))?;
    let copy = Store::inspect(&target)?;
    if copy.counts != counts {
        return Err(Problem::new(
            ProblemKind::Database,
            format!("the copy holds {:?}, the database {counts:?}", copy.counts),
        ));
    }
    tracing::info!(copy = %target.display(), "database backed up");
    Ok(CopyReport {
        path: target.display().to_string(),
        bytes,
        counts,
    })
}

/// Moves (or renames) the database file to `target`, never over another
/// file. On the same disk, the file is renamed; elsewhere it is copied,
/// the copy checked, and only then the original deleted.
pub fn move_to(slot: &mut Option<Store>, target: &Path) -> Result<(), Problem> {
    let store = writable(slot)?;
    let source = store.path.clone();
    let target = platform::absolute(target).map_err(|err| Problem::from_io(&err))?;
    if target.exists() {
        return Err(Problem::new(
            ProblemKind::AlreadyExists,
            format!("{} already exists", target.display()),
        ));
    }
    let counts = store.counts()?;
    // Closed first: its journal is merged back into the file.
    *slot = None;
    let result = (|| {
        if schema::wal_of(&source).exists() {
            return Err(Problem::new(
                ProblemKind::Database,
                "the database journal could not be merged back; nothing was moved",
            ));
        }
        match platform::rename_noclobber(&source, &target) {
            Ok(()) => Store::open(&target),
            Err(err) if platform::is_other_volume(&err) => {
                copy_then_delete(&source, &target, counts)
            }
            Err(err) => Err(Problem::from_io(&err)),
        }
    })();
    match result {
        Ok(moved) => {
            tracing::info!(from = %source.display(), to = %target.display(), "database moved");
            *slot = Some(moved);
            Ok(())
        }
        Err(problem) => {
            reopen(slot, &source);
            Err(problem)
        }
    }
}

/// The original is only deleted once the copy opened with the same counts.
pub(super) fn copy_then_delete(
    source: &Path,
    target: &Path,
    counts: Counts,
) -> Result<Store, Problem> {
    let io_problem = |err: std::io::Error| Problem::from_io(&err);
    let partial = partial_name(target);
    let _ = std::fs::remove_file(&partial);
    let copied = std::fs::copy(source, &partial)
        .and_then(|_| platform::rename_noclobber(&partial, target))
        .map_err(io_problem);
    if let Err(problem) = copied {
        let _ = std::fs::remove_file(&partial);
        return Err(problem);
    }
    let copy = match Store::open(target) {
        Ok(copy) if copy.counts().ok() == Some(counts) => copy,
        Ok(copy) => {
            drop(copy);
            let _ = std::fs::remove_file(target);
            return Err(Problem::new(
                ProblemKind::Database,
                "the copy does not hold what the database holds; nothing was moved",
            ));
        }
        Err(problem) => {
            let _ = std::fs::remove_file(target);
            return Err(problem);
        }
    };
    if let Err(err) = std::fs::remove_file(source) {
        // Both files exist: the copy is in use from now on, the original
        // stays where it was; better than losing either.
        tracing::warn!(%err, "database copied, but the original could not be deleted");
    }
    Ok(copy)
}

/// Replaces the content of the open database with the backup at `backup`,
/// after copying the current content next to it. Returns that copy.
pub fn restore(slot: &mut Option<Store>, backup: &Path) -> Result<CopyReport, Problem> {
    let store = writable(slot)?;
    let current = store.path.clone();
    let backup = platform::absolute(backup).map_err(|err| Problem::from_io(&err))?;
    if backup == current {
        return Err(Problem::new(
            ProblemKind::AlreadyExists,
            "this is the database itself",
        ));
    }
    let inspected = Store::inspect(&backup)?;
    let safety = safety_copy_name(&current, "before-restore");
    let report = back_up(store, &safety, false)?;
    *slot = None;
    let result = (|| {
        if schema::wal_of(&current).exists() {
            return Err(Problem::new(
                ProblemKind::Database,
                "the database journal could not be merged back; nothing was restored",
            ));
        }
        // Next to the database first, then put in place in one rename.
        let partial = partial_name(&current);
        let _ = std::fs::remove_file(&partial);
        let placed = std::fs::copy(&backup, &partial)
            .and_then(|_| std::fs::rename(&partial, &current))
            .map_err(|err| Problem::from_io(&err));
        if let Err(problem) = placed {
            let _ = std::fs::remove_file(&partial);
            return Err(problem);
        }
        // An older schema is updated here, with its own copy first.
        Store::open(&current)
    })();
    match result {
        Ok(restored) => {
            tracing::info!(
                backup = %backup.display(),
                references = inspected.counts.references,
                "database restored"
            );
            *slot = Some(restored);
            Ok(report)
        }
        Err(problem) => {
            reopen(slot, &current);
            Err(problem)
        }
    }
}

fn writable(slot: &mut Option<Store>) -> Result<&Store, Problem> {
    let store = slot
        .as_ref()
        .ok_or_else(|| Problem::new(ProblemKind::NotFound, "no database is open"))?;
    store.writable()?;
    Ok(store)
}

fn reopen(slot: &mut Option<Store>, path: &Path) {
    match Store::open(path) {
        Ok(store) => *slot = Some(store),
        Err(problem) => tracing::error!(%problem, path = %path.display(), "database not reopened"),
    }
}

/// `name.before-restore-20261022-091503.fileseal`, next to the database.
pub(crate) fn safety_copy_name(path: &Path, what: &str) -> PathBuf {
    let dir = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .map_or_else(|| "database".into(), |s| s.to_string_lossy().into_owned());
    let stamp: String = timestamp::now()
        .chars()
        .filter(char::is_ascii_digit)
        .take(14)
        .collect();
    dir.join(format!(
        "{stem}.{what}-{}-{}.fileseal",
        &stamp[..8],
        &stamp[8..]
    ))
}

/// `name.fileseal.partial`: excluded from selections, never taken for a
/// database.
fn partial_name(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_os_string();
    name.push(".partial");
    PathBuf::from(name)
}

fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
