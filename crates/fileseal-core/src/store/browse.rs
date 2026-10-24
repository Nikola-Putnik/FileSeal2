//! Reading a database as a whole: its references (to browse and search
//! them), the history of an item, its current locations (for the disks
//! overview), and the few changes made from the Databases screen: its
//! display name, many locations at once, items removed and put back.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{NewLocation, ReferenceVersion, Store, db_problem, insert_location};
use crate::algorithm::Algorithm;
use crate::faults::{self, FaultPoint};
use crate::problem::{Problem, ProblemKind};
use crate::timestamp;

/// A current reference, as listed on the Databases screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReferenceRow {
    #[ts(type = "number")]
    pub item_id: i64,
    pub path: String,
    #[ts(type = "number")]
    pub size: u64,
    pub created_at: String,
    pub algorithms: Vec<Algorithm>,
    /// Date and result code of the last verification, if any.
    pub last_checked_at: Option<String>,
    pub last_outcome: Option<String>,
    /// When it was removed from the database; then `path` is where it was.
    pub retired_at: Option<String>,
}

/// Which items a search of the references looks at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Listed {
    /// Those the database uses (to verify, to search).
    #[default]
    Active,
    /// Those removed from it: kept with their history, and can be put back.
    Retired,
}

/// The column references are listed by (the columns of the Databases
/// screen).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BrowseColumn {
    /// The file name, whatever the folder (case ignored).
    Name,
    /// The whole path: by folder, then by name within a folder.
    #[default]
    Folder,
    Size,
    /// When the current reference was recorded; for removed items, when
    /// they were removed.
    Recorded,
    /// The last verification. Never verified comes first in ascending order:
    /// what most needs verifying.
    LastCheck,
}

/// The order of a listing of references. Applied by the database to every
/// reference found, before keeping the first ones: the first of the whole
/// database, not of a page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BrowseOrder {
    pub column: BrowseColumn,
    pub descending: bool,
}

/// Items put back in the database.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Restored {
    pub restored: u32,
    /// Where they were, now held by another item: left removed.
    pub taken: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReferencePage {
    /// References matching the search, whether listed or not.
    #[ts(type = "number")]
    pub total: u64,
    /// At most the limit asked for, by path.
    pub rows: Vec<ReferenceRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LocationRecord {
    pub path: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub missing_since: Option<String>,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VerificationRecord {
    pub finished_at: String,
    pub outcome: String,
    pub observed_path: String,
    pub detail: Option<String>,
}

/// Everything recorded about one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemHistory {
    #[ts(type = "number")]
    pub item_id: i64,
    pub created_at: String,
    /// When it was removed from the database, if it was.
    pub retired_at: Option<String>,
    /// The current version first.
    pub versions: Vec<ReferenceVersion>,
    /// The current location first, then the former ones, latest first.
    pub locations: Vec<LocationRecord>,
    /// Latest first, at most `HISTORY_LIMIT`.
    pub verifications: Vec<VerificationRecord>,
    /// More verifications than listed.
    pub more_verifications: bool,
}

const HISTORY_LIMIT: u32 = 200;

/// The current location of an active item, as needed to tell where its
/// disk is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentLocation {
    pub item_id: i64,
    pub path: String,
    pub volume_serial: Option<String>,
    pub volume_relative_key: Option<String>,
    pub last_size: Option<u64>,
    pub last_mtime: Option<String>,
}

/// What a database holds, to check that a copy of it is complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Counts {
    #[ts(type = "number")]
    pub items: u64,
    #[ts(type = "number")]
    pub references: u64,
    #[ts(type = "number")]
    pub locations: u64,
    #[ts(type = "number")]
    pub verifications: u64,
}

/// Locations changed at once (reassigning a root).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relocated {
    pub moved: u32,
    /// Paths already held by another item: left as they were.
    pub taken: Vec<String>,
}

impl Store {
    /// Changes the name shown for the database (not its file name).
    pub fn rename(&mut self, name: &str) -> Result<(), Problem> {
        self.writable()?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Problem::new(ProblemKind::Io, "a database needs a name"));
        }
        self.conn
            .execute("UPDATE meta SET value = ?1 WHERE key = 'name'", [name])
            .map_err(db_problem)?;
        self.name = name.to_owned();
        Ok(())
    }

    pub fn counts(&self) -> Result<Counts, Problem> {
        let count = |sql: &str| -> Result<u64, Problem> {
            self.conn
                .query_row(sql, [], |row| row.get::<_, i64>(0))
                .map(|n| n as u64)
                .map_err(db_problem)
        };
        Ok(Counts {
            items: count("SELECT count(*) FROM item")?,
            references: count("SELECT count(*) FROM reference")?,
            locations: count("SELECT count(*) FROM location")?,
            verifications: count("SELECT count(*) FROM verification")?,
        })
    }

    /// Current references whose path contains `query` (case ignored), or
    /// whose hash starts with it when it looks like one (8 hexadecimal
    /// digits or more). An empty query lists everything. At most `limit`
    /// rows, the first in `order`; `total` says how many match. A removed
    /// item is listed at the place it was.
    pub fn browse(
        &self,
        query: &str,
        limit: u32,
        listed: Listed,
        order: BrowseOrder,
    ) -> Result<ReferencePage, Problem> {
        let (from, value) = matching(query, listed);
        let bind = |statement: &mut rusqlite::Statement| -> rusqlite::Result<()> {
            if !value.is_empty() {
                statement.raw_bind_parameter(1, &value)?;
            }
            Ok(())
        };
        let mut total = self
            .conn
            .prepare(&format!("SELECT count(*) {from}"))
            .map_err(db_problem)?;
        bind(&mut total).map_err(db_problem)?;
        let total = total
            .raw_query()
            .next()
            .map_err(db_problem)?
            .map(|row| row.get::<_, i64>(0))
            .transpose()
            .map_err(db_problem)?
            .unwrap_or(0) as u64;
        let sql = format!(
            "SELECT l.item_id, l.path, r.size, r.created_at, r.id, \
             (SELECT v.finished_at FROM verification v WHERE v.item_id = l.item_id \
              ORDER BY v.finished_at DESC, v.id DESC LIMIT 1) AS last_checked_at, \
             (SELECT v.outcome FROM verification v WHERE v.item_id = l.item_id \
              ORDER BY v.finished_at DESC, v.id DESC LIMIT 1), \
             i.retired_at \
             {from} ORDER BY {} LIMIT {limit}",
            order_by(order, listed)
        );
        let mut statement = self.conn.prepare(&sql).map_err(db_problem)?;
        bind(&mut statement).map_err(db_problem)?;
        let mut rows = Vec::new();
        let mut cursor = statement.raw_query();
        while let Some(row) = cursor.next().map_err(db_problem)? {
            let reference_id: i64 = row.get(4).map_err(db_problem)?;
            rows.push(ReferenceRow {
                item_id: row.get(0).map_err(db_problem)?,
                path: row.get(1).map_err(db_problem)?,
                size: row.get::<_, i64>(2).map_err(db_problem)? as u64,
                created_at: row.get(3).map_err(db_problem)?,
                algorithms: self
                    .reference_digests(reference_id)?
                    .into_iter()
                    .map(|d| d.algorithm)
                    .collect(),
                last_checked_at: row.get(5).map_err(db_problem)?,
                last_outcome: row.get(6).map_err(db_problem)?,
                retired_at: row.get(7).map_err(db_problem)?,
            });
        }
        Ok(ReferencePage { total, rows })
    }

    /// Every item `browse` would find, not only the first ones: what a
    /// change "for every result of the search" applies to.
    pub fn matching_items(&self, query: &str, listed: Listed) -> Result<Vec<i64>, Problem> {
        let (from, value) = matching(query, listed);
        let mut statement = self
            .conn
            .prepare(&format!("SELECT l.item_id {from} ORDER BY l.path_key"))
            .map_err(db_problem)?;
        if !value.is_empty() {
            statement
                .raw_bind_parameter(1, &value)
                .map_err(db_problem)?;
        }
        let mut ids = Vec::new();
        let mut cursor = statement.raw_query();
        while let Some(row) = cursor.next().map_err(db_problem)? {
            ids.push(row.get(0).map_err(db_problem)?);
        }
        Ok(ids)
    }

    /// Removes items from the database, in one transaction: they are no
    /// longer used to verify or to search, and their place is free for
    /// another file. Nothing is erased: their history stays, and they can be
    /// put back. Their files and hash files are not touched. Returns how
    /// many were removed (those already removed are left as they are).
    pub fn retire_items(&mut self, item_ids: &[i64]) -> Result<u32, Problem> {
        self.writable()?;
        let now = timestamp::now();
        let tx = self.conn.transaction().map_err(db_problem)?;
        let mut retired = 0;
        {
            let mut retire = tx
                .prepare("UPDATE item SET retired_at = ?2 WHERE id = ?1 AND retired_at IS NULL")
                .map_err(db_problem)?;
            let mut leave = tx
                .prepare("UPDATE location SET is_current = 0 WHERE item_id = ?1 AND is_current = 1")
                .map_err(db_problem)?;
            for item_id in item_ids {
                if retire.execute(params![item_id, now]).map_err(db_problem)? == 1 {
                    leave.execute([item_id]).map_err(db_problem)?;
                    retired += 1;
                }
            }
        }
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)?;
        Ok(retired)
    }

    /// Puts removed items back, at the place they were when removed. A place
    /// another item holds now is reported, and that item stays removed.
    pub fn restore_items(&mut self, item_ids: &[i64]) -> Result<Restored, Problem> {
        self.writable()?;
        let tx = self.conn.transaction().map_err(db_problem)?;
        let mut done = Restored::default();
        for item_id in item_ids {
            let last: Option<(i64, String, String)> = tx
                .query_row(
                    "SELECT l.id, l.path_key, l.path FROM location l \
                     JOIN item i ON i.id = l.item_id AND i.retired_at IS NOT NULL \
                     WHERE l.item_id = ?1 ORDER BY l.id DESC LIMIT 1",
                    [item_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(db_problem)?;
            let Some((location_id, path_key, path)) = last else {
                continue;
            };
            let taken: bool = tx
                .query_row(
                    "SELECT EXISTS (SELECT 1 FROM location WHERE path_key = ?1 AND is_current = 1)",
                    [&path_key],
                    |row| row.get(0),
                )
                .map_err(db_problem)?;
            if taken {
                done.taken.push(path);
                continue;
            }
            tx.execute(
                "UPDATE location SET is_current = 1 WHERE id = ?1",
                [location_id],
            )
            .map_err(db_problem)?;
            tx.execute("UPDATE item SET retired_at = NULL WHERE id = ?1", [item_id])
                .map_err(db_problem)?;
            done.restored += 1;
        }
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)?;
        Ok(done)
    }

    pub fn item_history(&self, item_id: i64) -> Result<ItemHistory, Problem> {
        let (created_at, retired_at): (String, Option<String>) = self
            .conn
            .query_row(
                "SELECT created_at, retired_at FROM item WHERE id = ?1",
                [item_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_problem)?
            .ok_or_else(|| Problem::new(ProblemKind::NotFound, format!("no item {item_id}")))?;
        let locations = self
            .conn
            .prepare(
                "SELECT path, first_seen_at, last_seen_at, missing_since, is_current \
                 FROM location WHERE item_id = ?1 ORDER BY is_current DESC, id DESC",
            )
            .map_err(db_problem)?
            .query_map([item_id], |row| {
                Ok(LocationRecord {
                    path: row.get(0)?,
                    first_seen_at: row.get(1)?,
                    last_seen_at: row.get(2)?,
                    missing_since: row.get(3)?,
                    current: row.get::<_, i64>(4)? == 1,
                })
            })
            .map_err(db_problem)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_problem)?;
        let mut verifications = self
            .conn
            .prepare(&format!(
                "SELECT finished_at, outcome, observed_path, detail FROM verification \
                 WHERE item_id = ?1 ORDER BY finished_at DESC, id DESC LIMIT {}",
                HISTORY_LIMIT + 1
            ))
            .map_err(db_problem)?
            .query_map([item_id], |row| {
                Ok(VerificationRecord {
                    finished_at: row.get(0)?,
                    outcome: row.get(1)?,
                    observed_path: row.get(2)?,
                    detail: row.get(3)?,
                })
            })
            .map_err(db_problem)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_problem)?;
        let more_verifications = verifications.len() > HISTORY_LIMIT as usize;
        verifications.truncate(HISTORY_LIMIT as usize);
        Ok(ItemHistory {
            item_id,
            created_at,
            retired_at,
            versions: self.references_of(item_id)?,
            locations,
            verifications,
            more_verifications,
        })
    }

    /// Every current location of an active item.
    pub fn current_locations(&self) -> Result<Vec<CurrentLocation>, Problem> {
        self.conn
            .prepare(
                "SELECT l.item_id, l.path, l.volume_serial, l.volume_relative_path, \
                 l.last_size, l.last_mtime FROM location l \
                 JOIN item i ON i.id = l.item_id AND i.retired_at IS NULL \
                 WHERE l.is_current = 1 ORDER BY l.path_key",
            )
            .map_err(db_problem)?
            .query_map([], |row| {
                Ok(CurrentLocation {
                    item_id: row.get(0)?,
                    path: row.get(1)?,
                    volume_serial: row.get(2)?,
                    volume_relative_key: row.get(3)?,
                    last_size: row.get::<_, Option<i64>>(4)?.map(|s| s as u64),
                    last_mtime: row.get(5)?,
                })
            })
            .map_err(db_problem)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_problem)
    }

    /// R8 for many items at once, in one transaction: each item's location
    /// becomes the new one, the former one is kept as history. A path
    /// already held by another item is left alone and reported.
    pub fn relocate_many(&mut self, moves: &[(i64, NewLocation)]) -> Result<Relocated, Problem> {
        self.writable()?;
        let now = timestamp::now();
        let tx = self.conn.transaction().map_err(db_problem)?;
        let mut done = Relocated::default();
        for (item_id, location) in moves {
            let taken: Option<i64> = tx
                .query_row(
                    "SELECT l.item_id FROM location l JOIN item i ON i.id = l.item_id \
                     WHERE l.path_key = ?1 AND l.is_current = 1 AND i.retired_at IS NULL",
                    [&location.path_key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db_problem)?;
            if taken.is_some_and(|other| other != *item_id) {
                done.taken.push(location.path.clone());
                continue;
            }
            tx.execute(
                "UPDATE location SET is_current = 0, missing_since = COALESCE(missing_since, ?2) \
                 WHERE item_id = ?1 AND is_current = 1",
                params![item_id, now],
            )
            .map_err(db_problem)?;
            insert_location(&tx, *item_id, location, &now)?;
            done.moved += 1;
        }
        faults::check(FaultPoint::DatabaseWrite, &self.path)
            .map_err(|err| Problem::from_io(&err))?;
        tx.commit().map_err(db_problem)?;
        Ok(done)
    }
}

/// The `ORDER BY` of a listing, the path breaking ties so that the order is
/// always the same. By path, the index of current locations gives the
/// first rows without sorting the others.
fn order_by(order: BrowseOrder, listed: Listed) -> String {
    let direction = if order.descending { "DESC" } else { "ASC" };
    let key = match order.column {
        BrowseColumn::Name => "l.file_name COLLATE NOCASE",
        BrowseColumn::Folder => return format!("l.path_key {direction}"),
        BrowseColumn::Size => "r.size",
        BrowseColumn::Recorded => match listed {
            Listed::Active => "r.created_at",
            Listed::Retired => "i.retired_at",
        },
        BrowseColumn::LastCheck => "last_checked_at",
    };
    format!("{key} {direction}, l.path_key")
}

/// The `FROM … WHERE …` of a search of the references, with the value to
/// bind as `?1` (none when empty). Rows give the item (`i`), its current
/// reference (`r`) and a location (`l`): the current one, or for a removed
/// item the last one it had.
fn matching(query: &str, listed: Listed) -> (String, String) {
    let query = query.trim().to_lowercase();
    let is_hash = query.len() >= 8 && query.chars().all(|c| c.is_ascii_hexdigit());
    let (condition, value) = if query.is_empty() {
        ("1", String::new())
    } else if is_hash {
        (
            "EXISTS (SELECT 1 FROM reference_hash h WHERE h.reference_id = r.id \
             AND h.hex >= ?1 AND h.hex < ?1 || 'g')",
            query,
        )
    } else {
        (
            "l.path_key LIKE '%' || ?1 || '%' ESCAPE '\\'",
            query
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_"),
        )
    };
    let from = match listed {
        Listed::Active => format!(
            "FROM location l \
             JOIN item i ON i.id = l.item_id AND i.retired_at IS NULL \
             JOIN reference r ON r.item_id = l.item_id AND r.superseded_at IS NULL \
             WHERE l.is_current = 1 AND {condition}"
        ),
        Listed::Retired => format!(
            "FROM item i \
             JOIN location l ON l.id = (SELECT max(id) FROM location WHERE item_id = i.id) \
             JOIN reference r ON r.item_id = i.id AND r.superseded_at IS NULL \
             WHERE i.retired_at IS NOT NULL AND {condition}"
        ),
    };
    (from, value)
}
