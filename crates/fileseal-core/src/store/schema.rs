//! Opening a database file safely: locking, read-only fallback, checks, and
//! schema migrations with a backup copy first. Model and strategy:
//! docs/modele-donnees.md.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, MAIN_DB, OpenFlags};

use super::db_problem;
use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::timestamp;

/// `PRAGMA application_id` of FileSeal databases ("FSEL").
pub const APPLICATION_ID: i32 = 0x4653_454C;

/// Ordered migrations; `PRAGMA user_version` = number applied. A released
/// migration is never edited: changes go in a new file.
pub(super) const MIGRATIONS: &[&str] = &[include_str!("migrations/0001_initial.sql")];

pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// A connection that may write, opened for a new database.
pub(super) fn connect_new(path: &Path) -> Result<Connection, Problem> {
    connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
}

/// A connection to an existing database: read-write when possible, else
/// read-only (`true`). A read-only file, folder or medium (DVD, read-only
/// share) can still be consulted; writing is then refused by the store.
pub(super) fn connect_existing(path: &Path) -> Result<(Connection, bool), Problem> {
    let write_protected = std::fs::metadata(path)
        .map_err(|err| Problem::from_io(&err))?
        .permissions()
        .readonly();
    if write_protected {
        return connect_immutable(path);
    }
    let conn = connect(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    match probe(&conn) {
        Ok(()) => {
            let read_only = conn.is_readonly(MAIN_DB).map_err(db_problem)?;
            Ok((conn, read_only))
        }
        // A folder or share we cannot write to: SQLite cannot create the
        // -wal file it needs, and says so in one of these ways.
        Err(err) if cannot_write(&err) => {
            drop(conn);
            connect_immutable(path)
        }
        Err(err) => Err(db_problem(err)),
    }
}

/// Read-only access to a WAL database that cannot be written: `immutable`
/// tells SQLite nobody will change the file, so it needs no -wal file.
pub(super) fn connect_immutable(path: &Path) -> Result<(Connection, bool), Problem> {
    let uri = sqlite_uri(path, "immutable=1");
    let conn = connect(
        Path::new(&uri),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    probe(&conn).map_err(db_problem)?;
    Ok((conn, true))
}

fn connect(path: &Path, flags: OpenFlags) -> Result<Connection, Problem> {
    let conn = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(db_problem)?;
    conn.busy_timeout(Duration::from_millis(500))
        .map_err(db_problem)?;
    // Set before the first access: no shared-memory file is ever created in
    // WAL mode, and the lock is kept for the life of the connection, so a
    // database used elsewhere is reported as soon as it is read.
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")
        .map_err(db_problem)?;
    Ok(conn)
}

/// First read: takes the lock, and fails now rather than later if the file
/// cannot be read.
fn probe(conn: &Connection) -> rusqlite::Result<()> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|_| ())
}

fn cannot_write(err: &rusqlite::Error) -> bool {
    use rusqlite::ErrorCode;
    matches!(
        err.sqlite_error_code(),
        Some(
            ErrorCode::ReadOnly
                | ErrorCode::CannotOpen
                | ErrorCode::PermissionDenied
                | ErrorCode::SystemIoFailure
        )
    )
}

/// SQLite URI of a file path (`file:///C:/x%20y.fileseal?…`).
fn sqlite_uri(path: &Path, query: &str) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut encoded = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match c {
            '%' => encoded.push_str("%25"),
            '?' => encoded.push_str("%3f"),
            '#' => encoded.push_str("%23"),
            ' ' => encoded.push_str("%20"),
            c => encoded.push(c),
        }
    }
    // Empty authority, then the path: `file:///C:/…`, and for a UNC path
    // its two leading slashes too (`file:////server/share/…`).
    let prefix = if encoded.starts_with("//") {
        "file://"
    } else {
        "file:///"
    };
    format!("{prefix}{encoded}?{query}")
}

/// Settings that change the file: only applied to writable FileSeal databases.
pub(super) fn configure(conn: &Connection) -> Result<(), Problem> {
    // WAL: a crash in the middle of a write cannot corrupt the database.
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(db_problem)?;
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(db_problem)?;
    foreign_keys(conn)
}

pub(super) fn foreign_keys(conn: &Connection) -> Result<(), Problem> {
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(db_problem)
}

pub(super) fn application_id(conn: &Connection) -> Result<i32, Problem> {
    conn.pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(db_problem)
}

pub(super) fn schema_version(conn: &Connection) -> Result<u32, Problem> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(db_problem)
}

/// `PRAGMA quick_check`: structural damage (torn pages, broken indexes)
/// is reported when the database is opened, not discovered halfway
/// through a job.
pub(super) fn quick_check(conn: &Connection) -> Result<(), Problem> {
    let mut statement = conn.prepare("PRAGMA quick_check").map_err(db_problem)?;
    let messages: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .map_err(db_problem)?
        .collect::<rusqlite::Result<_>>()
        .map_err(db_problem)?;
    if messages == ["ok"] {
        return Ok(());
    }
    Err(Problem::new(
        ProblemKind::DatabaseCorrupt,
        messages.into_iter().take(5).collect::<Vec<_>>().join("; "),
    ))
}

/// Brings the schema to the last version. Before changing a database that
/// already has data, a copy is made next to it: a failed migration, or a
/// bug in a new version, must never cost a collection.
pub(super) fn migrate(
    conn: &mut Connection,
    path: &Path,
    migrations: &[&str],
) -> Result<Option<PathBuf>, Problem> {
    let current = schema_version(conn)?;
    let target = migrations.len() as u32;
    let backup = if current > 0 && current < target {
        Some(backup_before_migration(conn, path, current, target)?)
    } else {
        None
    };
    for (index, sql) in migrations.iter().enumerate().skip(current as usize) {
        let version = index as u32 + 1;
        let tx = conn.transaction().map_err(db_problem)?;
        tx.execute_batch(sql).map_err(db_problem)?;
        tx.pragma_update(None, "user_version", version)
            .map_err(db_problem)?;
        tx.commit().map_err(db_problem)?;
        tracing::info!(version, "database migration applied");
    }
    Ok(backup)
}

/// `name.before-v2-20261022-091503.fileseal` next to the database. Never
/// overwrites an earlier copy (the one from a failed attempt may be the
/// only good one).
fn backup_before_migration(
    conn: &Connection,
    path: &Path,
    from: u32,
    to: u32,
) -> Result<PathBuf, Problem> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let needed = database_bytes(path) + database_bytes(&wal_of(path)) + (16 << 20);
    if let Some(free) = platform::available_space(dir)
        && free < needed
    {
        return Err(Problem::new(
            ProblemKind::DiskFull,
            format!(
                "{free} bytes free, about {needed} needed to back the database up before updating it"
            ),
        ));
    }
    let stem = path
        .file_stem()
        .map_or_else(|| "database".into(), |s| s.to_string_lossy().into_owned());
    let stamp: String = timestamp::now()
        .chars()
        .filter(char::is_ascii_digit)
        .take(14)
        .collect();
    let copy = dir.join(format!(
        "{stem}.before-v{to}-{}-{}.fileseal",
        &stamp[..8],
        &stamp[8..]
    ));
    // Written under another name first: an interrupted copy never looks complete.
    let partial = copy.with_extension("fileseal.partial");
    let _ = std::fs::remove_file(&partial);
    conn.execute("VACUUM INTO ?1", [partial.to_string_lossy()])
        .map_err(db_problem)?;
    std::fs::rename(&partial, &copy).map_err(|err| Problem::from_io(&err))?;
    tracing::info!(copy = %copy.display(), from, to, "database copied before migration");
    Ok(copy)
}

pub(super) fn wal_of(path: &Path) -> PathBuf {
    let mut wal = path.as_os_str().to_os_string();
    wal.push("-wal");
    PathBuf::from(wal)
}

fn database_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_of_windows_and_unc_paths() {
        assert_eq!(
            sqlite_uri(Path::new(r"C:\Bases\Films 100%.fileseal"), "immutable=1"),
            "file:///C:/Bases/Films%20100%25.fileseal?immutable=1"
        );
        assert_eq!(
            sqlite_uri(Path::new(r"\\nas\bases\a#1?.fileseal"), "immutable=1"),
            "file:////nas/bases/a%231%3f.fileseal?immutable=1"
        );
    }

    #[test]
    fn a_migration_backs_up_the_database_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("films.fileseal");
        let mut store = super::super::Store::create(&path, "Films").unwrap();
        store
            .insert_reference(
                &super::super::NewLocation {
                    path: r"C:\a.mkv".to_owned(),
                    path_key: r"c:\a.mkv".to_owned(),
                    file_name: "a.mkv".to_owned(),
                    last_size: Some(3),
                    last_mtime: None,
                    volume_serial: None,
                    volume_relative_key: None,
                },
                3,
                &[],
            )
            .unwrap();
        drop(store);

        const V2: &str = "ALTER TABLE item ADD COLUMN label TEXT;";
        let mut conn = connect(&path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
        configure(&conn).unwrap();
        let backup = migrate(&mut conn, &path, &[MIGRATIONS[0], V2])
            .unwrap()
            .expect("a copy was made");
        assert_eq!(schema_version(&conn).unwrap(), 2);
        conn.execute("UPDATE item SET label = 'migrated'", [])
            .unwrap();
        drop(conn);

        let name = backup.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("films.before-v2-") && name.ends_with(".fileseal"),
            "{name}"
        );
        let copy = Connection::open(&backup).unwrap();
        assert_eq!(
            schema_version(&copy).unwrap(),
            1,
            "the copy is the old version"
        );
        let items: i64 = copy
            .query_row("SELECT count(*) FROM item", [], |row| row.get(0))
            .unwrap();
        assert_eq!(items, 1, "with its data");
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".partial")
            })
            .count();
        assert_eq!(leftovers, 0, "no partial copy left");

        // Up to date: nothing copied.
        let mut conn = connect(&path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
        assert_eq!(
            migrate(&mut conn, &path, &[MIGRATIONS[0], V2]).unwrap(),
            None
        );
    }

    #[test]
    fn a_failed_migration_leaves_the_database_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("films.fileseal");
        drop(super::super::Store::create(&path, "Films").unwrap());
        let mut conn = connect(&path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
        configure(&conn).unwrap();
        let broken = "ALTER TABLE item ADD COLUMN label TEXT; THIS IS NOT SQL;";
        assert!(migrate(&mut conn, &path, &[MIGRATIONS[0], broken]).is_err());
        assert_eq!(
            schema_version(&conn).unwrap(),
            1,
            "the transaction was rolled back"
        );
        let columns: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('item')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(!columns.contains(&"label".to_owned()));
    }
}
