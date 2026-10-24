//! Reference databases: one SQLite file (`.fileseal`) per collection, any
//! number of them, one open at a time per operation.
//!
//! The connection uses WAL with `locking_mode=EXCLUSIVE`: no `-shm` file,
//! and a second FileSeal (or any other program) gets "database in use"
//! instead of writing at the same time. One `Store` = one connection, used
//! by one thread at a time: the readers of a batch share it through a
//! `SharedStore`, locked for each short step.
//!
//! A database that cannot be written (read-only file, folder or medium)
//! still opens, for consultation and verification: nothing is recorded.

mod browse;
pub mod files;
mod schema;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use crate::algorithm::{Algorithm, Digest};
use crate::faults::{self, FaultPoint};
use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::timestamp;

pub use browse::{
    BrowseColumn, BrowseOrder, Counts, CurrentLocation, ItemHistory, Listed, LocationRecord,
    ReferencePage, ReferenceRow, Relocated, Restored, VerificationRecord,
};
pub use schema::{APPLICATION_ID, SCHEMA_VERSION};

/// A database shared by several threads (the readers of a batch). Each
/// lookup or write locks it briefly; nothing holds it while a file is read,
/// so a reader on one disk never waits for a reader on another.
pub struct SharedStore<'a>(Mutex<&'a mut Store>);

impl<'a> SharedStore<'a> {
    pub fn new(store: &'a mut Store) -> SharedStore<'a> {
        SharedStore(Mutex::new(store))
    }

    pub fn lock(&self) -> MutexGuard<'_, &'a mut Store> {
        // A panic in one reader must not make the database unusable.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct Store {
    conn: Connection,
    path: PathBuf,
    name: String,
    read_only: bool,
    on_network: bool,
    migration_backup: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DatabaseInfo {
    pub name: String,
    pub path: String,
    pub schema_version: u32,
    /// Items with a current reference.
    #[ts(type = "number")]
    pub references: u64,
    /// Opened for consultation only: nothing can be recorded.
    pub read_only: bool,
    /// On a network share, where SQLite's locking is not reliable.
    pub on_network: bool,
    /// Copy made before updating the schema when this database was opened.
    pub migration_backup: Option<String>,
}

/// The current reference of an item, found through one of its locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownReference {
    pub item_id: i64,
    pub reference_id: i64,
    pub size: u64,
    pub created_at: String,
    pub digests: Vec<Digest>,
    pub path: String,
}

/// Where a file was seen, as stored in `location`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLocation {
    pub path: String,
    pub path_key: String,
    pub file_name: String,
    /// Size and last write time seen there (hints, never proof).
    pub last_size: Option<u64>,
    pub last_mtime: Option<String>,
    /// R2: the place on its volume, independent of the drive letter.
    pub volume_serial: Option<String>,
    pub volume_relative_key: Option<String>,
}

/// One version of an item's expected content, for its history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReferenceVersion {
    #[ts(type = "number")]
    pub reference_id: i64,
    #[ts(type = "number")]
    pub size: u64,
    pub created_at: String,
    /// `created`, `accepted` or `imported`.
    pub origin: String,
    pub superseded_at: Option<String>,
    pub digests: Vec<Digest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Create,
    Verify,
    Attach,
}

impl OperationKind {
    fn code(self) -> &'static str {
        match self {
            OperationKind::Create => "create",
            OperationKind::Verify => "verify",
            OperationKind::Attach => "attach",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationStatus {
    Completed,
    Cancelled,
    Failed,
}

impl OperationStatus {
    fn code(self) -> &'static str {
        match self {
            OperationStatus::Completed => "completed",
            OperationStatus::Cancelled => "cancelled",
            OperationStatus::Failed => "failed",
        }
    }
}

/// A verification to record, with what was observed.
#[derive(Debug, Clone)]
pub struct NewVerification<'a> {
    pub operation_id: i64,
    pub item_id: Option<i64>,
    pub reference_id: Option<i64>,
    pub observed_path: &'a str,
    pub observed_size: Option<u64>,
    pub observed_mtime: Option<String>,
    pub started_at: &'a str,
    pub outcome: &'a str,
    pub detail: Option<&'a str>,
    pub digests: &'a [Digest],
}

impl Store {
    /// Creates a new database. Never touches an existing file.
    pub fn create(path: &Path, name: &str) -> Result<Store, Problem> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Problem::new(ProblemKind::Io, "a database needs a name"));
        }
        let path = &absolute(path)?;
        // `create_new` makes the "does it exist?" check and the creation one
        // atomic step: an existing file, FileSeal or not, is never reused.
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|err| Problem::from_io(&err))?;
        let result = (|| {
            let mut conn = schema::connect_new(path)?;
            conn.pragma_update(None, "application_id", APPLICATION_ID)
                .map_err(db_problem)?;
            schema::configure(&conn)?;
            schema::migrate(&mut conn, path, schema::MIGRATIONS)?;
            let now = timestamp::now();
            let uuid = uuid::Uuid::new_v4().to_string();
            let tx = conn.transaction().map_err(db_problem)?;
            for (key, value) in [
                ("name", name),
                ("uuid", uuid.as_str()),
                ("created_at", now.as_str()),
                ("created_by_version", crate::APP_VERSION),
            ] {
                tx.execute(
                    "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                    params![key, value],
                )
                .map_err(db_problem)?;
            }
            tx.commit().map_err(db_problem)?;
            Ok(conn)
        })();
        match result {
            Ok(conn) => {
                tracing::info!(path = %path.display(), "database created");
                Ok(Store {
                    conn,
                    path: path.to_path_buf(),
                    name: name.to_owned(),
                    read_only: false,
                    on_network: platform::is_network_path(path),
                    migration_backup: None,
                })
            }
            Err(problem) => {
                // Our own half-created file: remove it rather than leave a
                // database that is not one.
                let _ = std::fs::remove_file(path);
                Err(problem)
            }
        }
    }

    /// Opens an existing database. Checks, in this order, before changing
    /// anything: that it is a FileSeal database, not from a newer FileSeal,
    /// and not damaged. Then updates its schema if needed, after a copy.
    pub fn open(path: &Path) -> Result<Store, Problem> {
        let path = &absolute(path)?;
        if !path.is_file() {
            return Err(Problem::new(
                ProblemKind::NotFound,
                format!("{} does not exist", path.display()),
            ));
        }
        let (mut conn, read_only) = schema::connect_existing(path)?;
        // Read before anything is changed: a foreign SQLite file must be
        // left exactly as it was (not even switched to WAL).
        if schema::application_id(&conn)? != APPLICATION_ID {
            return Err(Problem::new(
                ProblemKind::NotAFileSealDatabase,
                format!("{} is not a FileSeal database", path.display()),
            ));
        }
        let version = schema::schema_version(&conn)?;
        if version > SCHEMA_VERSION {
            return Err(Problem::new(
                ProblemKind::DatabaseTooNew,
                format!(
                    "schema version {version}, this version of FileSeal supports up to {SCHEMA_VERSION}"
                ),
            ));
        }
        schema::quick_check(&conn)?;
        let migration_backup = if read_only {
            if version < SCHEMA_VERSION {
                return Err(Problem::new(
                    ProblemKind::DatabaseReadOnly,
                    format!(
                        "schema version {version} must be updated to {SCHEMA_VERSION}, which needs write access"
                    ),
                ));
            }
            schema::foreign_keys(&conn)?;
            None
        } else {
            schema::configure(&conn)?;
            schema::migrate(&mut conn, path, schema::MIGRATIONS)?
        };
        let name: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'name'", [], |row| {
                row.get(0)
            })
            .map_err(db_problem)?;
        tracing::info!(path = %path.display(), read_only, "database opened");
        Ok(Store {
            conn,
            path: path.to_path_buf(),
            name,
            read_only,
            on_network: platform::is_network_path(path),
            migration_backup,
        })
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Refuses writes to a read-only database with a clear reason, before
    /// SQLite would.
    fn writable(&self) -> Result<(), Problem> {
        if self.read_only {
            return Err(Problem::new(
                ProblemKind::DatabaseReadOnly,
                format!("{} is opened read-only", self.path.display()),
            ));
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn info(&self) -> Result<DatabaseInfo, Problem> {
        let references: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM reference r JOIN item i ON i.id = r.item_id \
                 WHERE r.superseded_at IS NULL AND i.retired_at IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(db_problem)?;
        Ok(DatabaseInfo {
            name: self.name.clone(),
            path: self.path.display().to_string(),
            schema_version: schema::schema_version(&self.conn)?,
            references: references as u64,
            read_only: self.read_only,
            on_network: self.on_network,
            migration_backup: self
                .migration_backup
                .as_ref()
                .map(|p| p.display().to_string()),
        })
    }

    /// During a batch (`true`), commits go to the journal without waiting
    /// for the disk each time (`synchronous=NORMAL`): a power cut could lose
    /// the last seconds of the batch, never damage the database (WAL), and a
    /// write that fails (disk full) still fails at once. At the end
    /// (`false`), everything is forced to the disk and each commit waits
    /// for it again.
    pub fn batch_durability(&mut self, batch: bool) -> Result<(), Problem> {
        self.writable()?;
        if batch {
            return self
                .conn
                .pragma_update(None, "synchronous", "NORMAL")
                .map_err(db_problem);
        }
        self.conn
            .pragma_update(None, "synchronous", "FULL")
            .map_err(db_problem)?;
        self.conn
            .query_row("PRAGMA wal_checkpoint(FULL)", [], |_| Ok(()))
            .map_err(db_problem)
    }

    /// `PRAGMA synchronous`: 2 when every commit waits for the disk.
    #[cfg(test)]
    pub(crate) fn synchronous(&self) -> i64 {
        self.conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .unwrap()
    }

    /// Starts an operation in the history. A read-only database keeps no
    /// history: the operation is not recorded and its id is 0.
    pub fn begin_operation(&mut self, kind: OperationKind) -> Result<i64, Problem> {
        if self.read_only {
            return Ok(0);
        }
        self.conn
            .execute(
                "INSERT INTO operation (kind, started_at, app_version) VALUES (?1, ?2, ?3)",
                params![kind.code(), timestamp::now(), crate::APP_VERSION],
            )
            .map_err(db_problem)?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn finish_operation(
        &mut self,
        id: i64,
        status: OperationStatus,
        summary_json: Option<&str>,
    ) -> Result<(), Problem> {
        if self.read_only {
            return Ok(());
        }
        self.conn
            .execute(
                "UPDATE operation SET finished_at = ?2, status = ?3, summary_json = ?4 WHERE id = ?1",
                params![id, timestamp::now(), status.code(), summary_json],
            )
            .map_err(db_problem)?;
        Ok(())
    }

    /// Current references (with their current location) of active items
    /// matching `lookup`, at most `limit`.
    fn current_where(
        &self,
        lookup: &Lookup,
        values: &[&dyn rusqlite::ToSql],
        limit: u32,
    ) -> Result<Vec<KnownReference>, Problem> {
        let mut statement = self
            .conn
            .prepare_cached(&current_sql(lookup, limit))
            .map_err(db_problem)?;
        let rows = statement
            .query_map(values, |row| {
                Ok(KnownReference {
                    item_id: row.get(0)?,
                    reference_id: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                    created_at: row.get(3)?,
                    digests: Vec::new(),
                    path: row.get(4)?,
                })
            })
            .map_err(db_problem)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_problem)?;
        let mut found = Vec::with_capacity(rows.len());
        for mut known in rows {
            known.digests = self.reference_digests(known.reference_id)?;
            found.push(known);
        }
        Ok(found)
    }

    /// R1: the current reference of the item currently located at `path_key`.
    pub fn current_by_path(&self, path_key: &str) -> Result<Option<KnownReference>, Problem> {
        Ok(self.current_where(&BY_PATH, &[&path_key], 1)?.pop())
    }

    /// R2: the item recorded at the same place of the same volume, whatever
    /// the drive letter it had then.
    pub fn current_by_volume(
        &self,
        serial: &str,
        relative_key: &str,
    ) -> Result<Option<KnownReference>, Problem> {
        Ok(self
            .current_where(&BY_VOLUME, &[&serial, &relative_key], 1)?
            .pop())
    }

    /// The current reference of an item, with its current location.
    pub fn current_of_item(&self, item_id: i64) -> Result<Option<KnownReference>, Problem> {
        Ok(self.current_where(&BY_ITEM, &[&item_id], 1)?.pop())
    }

    /// R5: items whose current reference has this digest. The caller still
    /// compares size and every other algorithm.
    /// At most 20: each one found is then checked on the disk (is it still
    /// there?), and a collection full of identical files would otherwise
    /// make every file of a batch slower.
    pub fn current_by_digest(&self, digest: &Digest) -> Result<Vec<KnownReference>, Problem> {
        self.current_where(&BY_DIGEST, &[&digest.algorithm.code(), &digest.hex], 20)
    }

    /// R6, R7: items currently located under this file name (any folder).
    pub fn current_by_name(&self, file_name: &str) -> Result<Vec<KnownReference>, Problem> {
        self.current_where(&BY_NAME, &[&file_name], 20)
    }

    /// Whether any current reference has this size: if none has, the
    /// content cannot be known, and nothing needs to be read to say so.
    pub fn has_reference_of_size(&self, size: u64) -> Result<bool, Problem> {
        self.conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM reference WHERE size = ?1 AND superseded_at IS NULL)",
                [size as i64],
                |row| row.get(0),
            )
            .map_err(db_problem)
    }

    /// Algorithms used by references, to know what to compute when searching
    /// by content. Asked for every file of a batch: one index lookup per
    /// algorithm FileSeal knows, never a scan of every hash (which made a
    /// batch slower with each file it added).
    pub fn algorithms_in_use(&self) -> Result<Vec<Algorithm>, Problem> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT EXISTS (SELECT 1 FROM reference_hash WHERE algorithm = ?1)")
            .map_err(db_problem)?;
        let mut algorithms = Vec::new();
        for algorithm in Algorithm::ALL {
            let used: bool = statement
                .query_row([algorithm.code()], |row| row.get(0))
                .map_err(db_problem)?;
            if used {
                algorithms.push(algorithm);
            }
        }
        algorithms.sort_unstable();
        Ok(algorithms)
    }

    fn reference_digests(&self, reference_id: i64) -> Result<Vec<Digest>, Problem> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT algorithm, hex FROM reference_hash WHERE reference_id = ?1")
            .map_err(db_problem)?;
        let rows = statement
            .query_map([reference_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(db_problem)?;
        let mut digests = Vec::new();
        for row in rows {
            let (code, hex) = row.map_err(db_problem)?;
            // An algorithm added by a newer FileSeal is ignored, not an error.
            if let Some(algorithm) = Algorithm::from_code(&code) {
                digests.push(Digest { algorithm, hex });
            }
        }
        digests.sort_by_key(|d| d.algorithm);
        Ok(digests)
    }

    /// Records a new item with its first reference and location, in one
    /// transaction: either everything is written or nothing.
    pub fn insert_reference(
        &mut self,
        location: &NewLocation,
        size: u64,
        digests: &[Digest],
    ) -> Result<KnownReference, Problem> {
        self.insert_item(location, size, digests, "created")
    }

    /// Same, for a reference taken from the file's hash files without
    /// reading the file: its history says so (origin `imported`).
    pub fn insert_imported_reference(
        &mut self,
        location: &NewLocation,
        size: u64,
        digests: &[Digest],
    ) -> Result<KnownReference, Problem> {
        self.insert_item(location, size, digests, "imported")
    }

    fn insert_item(
        &mut self,
        location: &NewLocation,
        size: u64,
        digests: &[Digest],
        origin: &str,
    ) -> Result<KnownReference, Problem> {
        self.writable()?;
        let now = timestamp::now();
        let tx = self.conn.transaction().map_err(db_problem)?;
        tx.execute(
            "INSERT INTO item (uuid, created_at) VALUES (?1, ?2)",
            params![uuid::Uuid::new_v4().to_string(), now],
        )
        .map_err(db_problem)?;
        let item_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO reference (item_id, size, created_at, origin) VALUES (?1, ?2, ?3, ?4)",
            params![item_id, size as i64, now, origin],
        )
        .map_err(db_problem)?;
        let reference_id = tx.last_insert_rowid();
        for digest in digests {
            tx.execute(
                "INSERT INTO reference_hash (reference_id, algorithm, hex) VALUES (?1, ?2, ?3)",
                params![reference_id, digest.algorithm.code(), digest.hex],
            )
            .map_err(db_problem)?;
        }
        insert_location(&tx, item_id, location, &now)?;
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)?;
        let mut digests = digests.to_vec();
        digests.sort_by_key(|d| d.algorithm);
        Ok(KnownReference {
            item_id,
            reference_id,
            size,
            created_at: now,
            digests,
            path: location.path.clone(),
        })
    }

    /// Adds digests of new algorithms to an existing reference. Only called
    /// once the content was checked against the digests it already had.
    pub fn add_digests(&mut self, reference_id: i64, digests: &[Digest]) -> Result<(), Problem> {
        self.writable()?;
        let tx = self.conn.transaction().map_err(db_problem)?;
        for digest in digests {
            tx.execute(
                "INSERT INTO reference_hash (reference_id, algorithm, hex) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (reference_id, algorithm) DO NOTHING",
                params![reference_id, digest.algorithm.code(), digest.hex],
            )
            .map_err(db_problem)?;
        }
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)
    }

    /// Marks the item's current location as seen now, with what was seen.
    pub fn touch_location(
        &mut self,
        item_id: i64,
        size: u64,
        mtime: Option<&str>,
    ) -> Result<(), Problem> {
        self.writable()?;
        self.conn
            .execute(
                "UPDATE location SET last_seen_at = ?2, missing_since = NULL, \
                 last_size = ?3, last_mtime = ?4 WHERE item_id = ?1 AND is_current = 1",
                params![item_id, timestamp::now(), size as i64, mtime],
            )
            .map_err(db_problem)?;
        Ok(())
    }

    /// R8: the item now lives at `location` (it was moved, renamed, or its
    /// drive got another letter). Its previous location is kept as history.
    /// Refused if another active item is already recorded there.
    pub fn relocate(&mut self, item_id: i64, location: &NewLocation) -> Result<(), Problem> {
        self.relocate_checked(item_id, None, location).map(|_| ())
    }

    /// Same, only if the item is still recorded at `from_key`: false if it
    /// was moved meanwhile (another file of the batch attached to it).
    pub fn relocate_from(
        &mut self,
        item_id: i64,
        from_key: &str,
        location: &NewLocation,
    ) -> Result<bool, Problem> {
        self.relocate_checked(item_id, Some(from_key), location)
    }

    fn relocate_checked(
        &mut self,
        item_id: i64,
        from_key: Option<&str>,
        location: &NewLocation,
    ) -> Result<bool, Problem> {
        self.writable()?;
        let now = timestamp::now();
        let tx = self.conn.transaction().map_err(db_problem)?;
        if let Some(from_key) = from_key {
            let current: Option<String> = tx
                .query_row(
                    "SELECT path_key FROM location WHERE item_id = ?1 AND is_current = 1",
                    [item_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_problem)?;
            if current.as_deref() != Some(from_key) {
                return Ok(false);
            }
        }
        let taken: Option<i64> = tx
            .query_row(
                "SELECT l.item_id FROM location l JOIN item i ON i.id = l.item_id \
                 WHERE l.path_key = ?1 AND l.is_current = 1 AND i.retired_at IS NULL",
                [&location.path_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_problem)?;
        if let Some(other) = taken.filter(|other| *other != item_id) {
            return Err(Problem::new(
                ProblemKind::LocationTaken,
                format!("item {other} is already recorded at {}", location.path),
            ));
        }
        tx.execute(
            "UPDATE location SET is_current = 0, missing_since = COALESCE(missing_since, ?2) \
             WHERE item_id = ?1 AND is_current = 1",
            params![item_id, now],
        )
        .map_err(db_problem)?;
        insert_location(&tx, item_id, location, &now)?;
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)?;
        Ok(true)
    }

    /// Accepts what a verification observed as the item's new reference:
    /// the current reference is superseded (kept as history), the new one
    /// records the verification it comes from. Refused if the reference
    /// changed since that verification, or if nothing was observed.
    pub fn accept_version(&mut self, verification_id: i64) -> Result<KnownReference, Problem> {
        self.writable()?;
        let now = timestamp::now();
        let tx = self.conn.transaction().map_err(db_problem)?;
        // item, compared reference, observed size, observed path
        type Verification = (Option<i64>, Option<i64>, Option<i64>, String);
        let found: Option<Verification> = tx
            .query_row(
                "SELECT item_id, reference_id, observed_size, observed_path FROM verification \
                 WHERE id = ?1",
                [verification_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(db_problem)?;
        let stale = |detail: String| Problem::new(ProblemKind::StaleVerification, detail);
        let Some((Some(item_id), Some(compared), Some(size), path)) = found else {
            return Err(stale(format!(
                "verification {verification_id} has no observed content to accept"
            )));
        };
        let current: Option<i64> = tx
            .query_row(
                "SELECT id FROM reference WHERE item_id = ?1 AND superseded_at IS NULL",
                [item_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_problem)?;
        if current != Some(compared) {
            return Err(stale(format!(
                "the reference of item {item_id} changed since verification {verification_id}"
            )));
        }
        let digests: Vec<(String, String)> = tx
            .prepare("SELECT algorithm, hex FROM verification_hash WHERE verification_id = ?1")
            .map_err(db_problem)?
            .query_map([verification_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(db_problem)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_problem)?;
        if digests.is_empty() {
            return Err(stale(format!(
                "verification {verification_id} recorded no hash"
            )));
        }
        tx.execute(
            "UPDATE reference SET superseded_at = ?2 WHERE id = ?1",
            params![compared, now],
        )
        .map_err(db_problem)?;
        tx.execute(
            "INSERT INTO reference (item_id, size, created_at, origin, accepted_from_verification_id) \
             VALUES (?1, ?2, ?3, 'accepted', ?4)",
            params![item_id, size, now, verification_id],
        )
        .map_err(db_problem)?;
        let reference_id = tx.last_insert_rowid();
        for (algorithm, hex) in &digests {
            tx.execute(
                "INSERT INTO reference_hash (reference_id, algorithm, hex) VALUES (?1, ?2, ?3)",
                params![reference_id, algorithm, hex],
            )
            .map_err(db_problem)?;
        }
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)?;
        tracing::info!(item_id, verification_id, "new version accepted");
        Ok(KnownReference {
            item_id,
            reference_id,
            size: size as u64,
            created_at: now,
            digests: self.reference_digests(reference_id)?,
            path,
        })
    }

    /// Every version of an item's reference, the current one first.
    pub fn references_of(&self, item_id: i64) -> Result<Vec<ReferenceVersion>, Problem> {
        let rows: Vec<(i64, i64, String, String, Option<String>)> = self
            .conn
            .prepare(
                "SELECT id, size, created_at, origin, superseded_at FROM reference \
                 WHERE item_id = ?1 ORDER BY superseded_at IS NOT NULL, id DESC",
            )
            .map_err(db_problem)?
            .query_map([item_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(db_problem)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db_problem)?;
        rows.into_iter()
            .map(|(reference_id, size, created_at, origin, superseded_at)| {
                Ok(ReferenceVersion {
                    reference_id,
                    size: size as u64,
                    created_at,
                    origin,
                    superseded_at,
                    digests: self.reference_digests(reference_id)?,
                })
            })
            .collect()
    }

    pub fn record_verification(&mut self, v: &NewVerification) -> Result<i64, Problem> {
        self.writable()?;
        let tx = self.conn.transaction().map_err(db_problem)?;
        tx.execute(
            "INSERT INTO verification (operation_id, item_id, reference_id, observed_path, \
             observed_size, observed_mtime, started_at, finished_at, outcome, detail) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                v.operation_id,
                v.item_id,
                v.reference_id,
                v.observed_path,
                v.observed_size.map(|s| s as i64),
                v.observed_mtime,
                v.started_at,
                timestamp::now(),
                v.outcome,
                v.detail
            ],
        )
        .map_err(db_problem)?;
        let id = tx.last_insert_rowid();
        for digest in v.digests {
            tx.execute(
                "INSERT INTO verification_hash (verification_id, algorithm, hex) VALUES (?1, ?2, ?3)",
                params![id, digest.algorithm.code(), digest.hex],
            )
            .map_err(db_problem)?;
        }
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)?;
        Ok(id)
    }

    /// Date, result and detail of the last verification of an item.
    pub fn last_verification(
        &self,
        item_id: i64,
    ) -> Result<Option<(String, String, Option<String>)>, Problem> {
        self.conn
            .query_row(
                "SELECT finished_at, outcome, detail FROM verification \
                 WHERE item_id = ?1 ORDER BY finished_at DESC, id DESC LIMIT 1",
                [item_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(db_problem)
    }

    /// Tests only: pretends the item was recorded at `path` (another drive
    /// letter, an unplugged disk…), keeping its volume and place.
    #[cfg(test)]
    pub(crate) fn set_location_for_test(&mut self, item_id: i64, path: &str) {
        self.conn
            .execute(
                "UPDATE location SET path = ?2, path_key = ?3 WHERE item_id = ?1 AND is_current = 1",
                params![item_id, path, platform::path_key(path)],
            )
            .unwrap();
    }

    /// Number of verifications recorded, for tests and statistics.
    pub fn verification_count(&self) -> Result<u64, Problem> {
        self.conn
            .query_row("SELECT count(*) FROM verification", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|n| n as u64)
            .map_err(db_problem)
    }
}

/// Inserts the current location of an item (in the caller's transaction).
fn insert_location(
    tx: &rusqlite::Transaction,
    item_id: i64,
    location: &NewLocation,
    now: &str,
) -> Result<(), Problem> {
    tx.execute(
        "INSERT INTO location (item_id, path, path_key, file_name, volume_serial, \
         volume_relative_path, last_size, last_mtime, first_seen_at, last_seen_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            item_id,
            location.path,
            location.path_key,
            location.file_name,
            location.volume_serial,
            location.volume_relative_key,
            location.last_size.map(|s| s as i64),
            location.last_mtime,
            now
        ],
    )
    .map_err(db_problem)?;
    Ok(())
}

/// Databases are known by their absolute path, shown as such to the user.
fn absolute(path: &Path) -> Result<PathBuf, Problem> {
    crate::platform::absolute(path).map_err(|err| Problem::from_io(&err))
}

fn db_problem(err: rusqlite::Error) -> Problem {
    use rusqlite::ErrorCode;
    let kind = match err.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => ProblemKind::DatabaseInUse,
        Some(ErrorCode::ReadOnly) => ProblemKind::DatabaseReadOnly,
        Some(ErrorCode::NotADatabase) => ProblemKind::NotAFileSealDatabase,
        Some(ErrorCode::DiskFull) => ProblemKind::DiskFull,
        // The file can no longer be read or written: disk unplugged, share
        // lost, failing medium.
        Some(ErrorCode::SystemIoFailure | ErrorCode::CannotOpen) => {
            ProblemKind::DatabaseUnavailable
        }
        Some(ErrorCode::DatabaseCorrupt) => ProblemKind::DatabaseCorrupt,
        Some(ErrorCode::PermissionDenied) => ProblemKind::PermissionDenied,
        _ => ProblemKind::Database,
    };
    Problem::new(kind, err.to_string())
}

/// A lookup made for each file of a batch: the condition, and the table it
/// needs besides locations and references. Each must run on an index (test
/// `per_file_lookups_use_an_index`): a scan, or a list built before the
/// first row, makes every file slower as the database grows.
struct Lookup {
    join: &'static str,
    condition: &'static str,
}

const BY_PATH: Lookup = Lookup {
    join: "",
    condition: "l.path_key = ?1",
};
const BY_VOLUME: Lookup = Lookup {
    join: "",
    condition: "l.volume_serial = ?1 AND l.volume_relative_path = ?2",
};
const BY_ITEM: Lookup = Lookup {
    join: "",
    condition: "l.item_id = ?1",
};
/// A join, not `r.id IN (SELECT …)`: SQLite would first list every
/// reference with this digest, tens of thousands for a common content.
const BY_DIGEST: Lookup = Lookup {
    join: "JOIN reference_hash h ON h.reference_id = r.id",
    condition: "h.algorithm = ?1 AND h.hex = ?2",
};
const BY_NAME: Lookup = Lookup {
    join: "",
    condition: "l.file_name = ?1 COLLATE NOCASE",
};

/// Not sorted: SQLite then stops at the limit. Sorted, it would first
/// gather every match.
fn current_sql(lookup: &Lookup, limit: u32) -> String {
    format!(
        "SELECT l.item_id, r.id, r.size, r.created_at, l.path \
         FROM location l \
         JOIN item i ON i.id = l.item_id AND i.retired_at IS NULL \
         JOIN reference r ON r.item_id = l.item_id AND r.superseded_at IS NULL {} \
         WHERE l.is_current = 1 AND ({}) LIMIT {limit}",
        lookup.join, lookup.condition
    )
}

#[cfg(test)]
mod admin_tests;
#[cfg(test)]
mod tests {
    use super::*;

    fn location(path: &str) -> NewLocation {
        NewLocation {
            path: path.to_owned(),
            path_key: crate::platform::path_key(path),
            file_name: path.rsplit(['/', '\\']).next().unwrap().to_owned(),
            last_size: Some(1),
            last_mtime: None,
            volume_serial: None,
            volume_relative_key: None,
        }
    }

    fn sha(hex_char: char) -> Digest {
        Digest {
            algorithm: Algorithm::Sha256,
            hex: hex_char.to_string().repeat(64),
        }
    }

    #[test]
    fn create_then_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("films.fileseal");
        {
            let mut store = Store::create(&path, " Films ").unwrap();
            assert_eq!(store.name(), "Films");
            store
                .insert_reference(&location(r"C:\Films\a.mkv"), 10, &[sha('a')])
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        let info = store.info().unwrap();
        assert_eq!(
            (info.name.as_str(), info.schema_version, info.references),
            ("Films", 1, 1)
        );
        let known = store
            .current_by_path(&crate::platform::path_key(r"C:\FILMS\A.MKV"))
            .unwrap();
        if cfg!(windows) {
            let known = known.expect("found whatever the case on Windows");
            assert_eq!((known.size, known.digests.clone()), (10, vec![sha('a')]));
        }
        assert!(Store::create(&path, "again").is_err(), "never overwrites");
    }

    #[test]
    fn per_file_lookups_use_an_index() {
        // FileSeal never runs ANALYZE: SQLite plans the same way whatever
        // the database holds, so an empty one shows the plan of any.
        let dir = tempfile::tempdir().unwrap();
        let store = Store::create(&dir.path().join("plans.fileseal"), "Plans").unwrap();
        for (label, lookup) in [
            ("path", BY_PATH),
            ("volume", BY_VOLUME),
            ("item", BY_ITEM),
            ("digest", BY_DIGEST),
            ("name", BY_NAME),
        ] {
            let sql = format!("EXPLAIN QUERY PLAN {}", current_sql(&lookup, 20));
            let mut statement = store.conn.prepare(&sql).unwrap();
            let unset = std::iter::repeat_n(rusqlite::types::Null, statement.parameter_count());
            let steps: Vec<String> = statement
                .query_map(rusqlite::params_from_iter(unset), |row| row.get(3))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert!(
                steps.iter().all(|step| step.starts_with("SEARCH ")),
                "{label}: {steps:?}"
            );
        }
    }

    #[test]
    fn refuses_what_is_not_a_fileseal_database() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        assert_eq!(
            Store::open(&other).err().unwrap().kind,
            ProblemKind::NotAFileSealDatabase
        );
        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "hello, this is not SQLite at all").unwrap();
        assert_eq!(
            Store::open(&text).err().unwrap().kind,
            ProblemKind::NotAFileSealDatabase
        );
        assert_eq!(
            Store::open(&dir.path().join("missing.fileseal"))
                .err()
                .unwrap()
                .kind,
            ProblemKind::NotFound
        );
    }

    #[test]
    fn refuses_a_database_from_a_newer_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("future.fileseal");
        drop(Store::create(&path, "Future").unwrap());
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        assert_eq!(
            Store::open(&path).err().unwrap().kind,
            ProblemKind::DatabaseTooNew
        );
    }

    #[test]
    fn a_second_connection_is_refused_while_one_is_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("busy.fileseal");
        let _first = Store::create(&path, "Busy").unwrap();
        assert_eq!(
            Store::open(&path).err().unwrap().kind,
            ProblemKind::DatabaseInUse
        );
    }

    #[test]
    fn a_failed_write_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::create(&dir.path().join("f.fileseal"), "F").unwrap();
        faults::arm(FaultPoint::DatabaseWrite);
        let result = store.insert_reference(&location(r"C:\a.mkv"), 1, &[sha('b')]);
        faults::clear();
        assert!(result.is_err());
        assert_eq!(
            store.info().unwrap().references,
            0,
            "transaction rolled back"
        );
    }

    fn set_read_only(path: &Path, read_only: bool) {
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(read_only);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn a_read_only_database_opens_for_consultation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("archive.fileseal");
        {
            let mut store = Store::create(&path, "Archive").unwrap();
            store
                .insert_reference(&location(r"C:\Films\a.mkv"), 10, &[sha('a')])
                .unwrap();
        }
        set_read_only(&path, true);
        let result = (|| {
            let mut store = Store::open(&path)?;
            let info = store.info()?;
            assert!(info.read_only);
            let key = crate::platform::path_key(r"C:\Films\a.mkv");
            assert!(store.current_by_path(&key)?.is_some(), "readable");
            assert_eq!(
                store.begin_operation(OperationKind::Verify)?,
                0,
                "no history"
            );
            let refused = store.insert_reference(&location(r"C:\b.mkv"), 1, &[sha('b')]);
            assert_eq!(refused.unwrap_err().kind, ProblemKind::DatabaseReadOnly);
            Ok::<_, Problem>(())
        })();
        set_read_only(&path, false);
        result.unwrap();
        assert_eq!(Store::open(&path).unwrap().info().unwrap().references, 1);
    }

    #[test]
    fn a_damaged_database_is_reported_when_opened() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("damaged.fileseal");
        {
            let mut store = Store::create(&path, "Damaged").unwrap();
            for i in 0..200 {
                store
                    .insert_reference(&location(&format!(r"C:\Films\{i}.mkv")), i, &[sha('c')])
                    .unwrap();
            }
        }
        // Scribble over a page in the middle, keeping the header intact.
        let mut bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 4 * 4096);
        bytes[3 * 4096..4 * 4096].fill(0xAB);
        std::fs::write(&path, bytes).unwrap();
        let kind = Store::open(&path).err().map(|p| p.kind);
        assert!(
            matches!(
                kind,
                Some(ProblemKind::DatabaseCorrupt | ProblemKind::Database)
            ),
            "{kind:?}"
        );
    }

    #[test]
    fn opening_a_foreign_sqlite_file_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let other = dir.path().join("other.db");
        Connection::open(&other)
            .unwrap()
            .execute_batch("CREATE TABLE t (x); INSERT INTO t VALUES (1);")
            .unwrap();
        let before = std::fs::read(&other).unwrap();
        assert!(Store::open(&other).is_err());
        assert_eq!(
            std::fs::read(&other).unwrap(),
            before,
            "not even switched to WAL"
        );
    }
}
