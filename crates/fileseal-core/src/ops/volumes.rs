//! The disks a database knows, and where they are now: a disk that came
//! back under another drive letter is recognised by its serial number and
//! its files' places on it, without reading a single file (cadrage, rule 7).
//! Its locations, or those of any folder whose files moved elsewhere, can
//! then be reassigned in one decision (R8: explicit, traced, history kept).

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::store::{CurrentLocation, NewLocation, OperationKind, OperationStatus, Store};
use crate::timestamp;

/// Files of one disk (or one root without a known disk) in the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeStatus {
    /// The disk's serial number as recorded; `None` when the files were
    /// recorded without one (some network shares).
    pub serial: Option<String>,
    /// Where the files were recorded: `E:\`, `\\nas\films\`…
    pub recorded_root: String,
    pub files: u32,
    pub state: VolumeState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum VolumeState {
    /// The disk is where it was recorded.
    Connected,
    /// The same disk is now elsewhere (drive letter changed): its locations
    /// can be updated.
    MovedTo { root: String },
    /// Not found anywhere now (unplugged, switched off).
    NotConnected,
    /// Recorded without a serial number: only its root can be looked at.
    Unidentified { present: bool },
}

/// Files whose places on their disk are checked to tell where it is now.
const SAMPLES: usize = 5;

/// The disks of the database and where each one is now.
pub fn volumes(store: &Store) -> Result<Vec<VolumeStatus>, Problem> {
    struct Group {
        serial: Option<String>,
        root: String,
        files: u32,
        samples: Vec<String>,
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut index: HashMap<(Option<String>, String), usize> = HashMap::new();
    for location in store.current_locations()? {
        let (serial, root, relative) = placed(&location);
        let key = (serial.clone(), platform::path_key(&root));
        let at = *index.entry(key).or_insert_with(|| {
            groups.push(Group {
                serial,
                root,
                files: 0,
                samples: Vec::new(),
            });
            groups.len() - 1
        });
        let group = &mut groups[at];
        group.files += 1;
        if let Some(relative) = relative
            && group.samples.len() < SAMPLES
        {
            group.samples.push(relative);
        }
    }
    // Asked once: what each drive letter holds now.
    let mounted: Vec<(PathBuf, Option<String>)> = platform::drive_roots()
        .into_iter()
        .map(|root| {
            let serial = platform::volume_serial(&root);
            (root, serial)
        })
        .collect();
    let mut statuses: Vec<VolumeStatus> = groups
        .into_iter()
        .map(|group| {
            let root = Path::new(&group.root);
            let state = match &group.serial {
                None => VolumeState::Unidentified {
                    present: root.exists(),
                },
                Some(serial) if platform::volume_serial(root).as_ref() == Some(serial) => {
                    VolumeState::Connected
                }
                Some(serial) => match where_now(serial, &group.samples, &mounted, root) {
                    Some(now) => VolumeState::MovedTo {
                        root: now.display().to_string(),
                    },
                    None => VolumeState::NotConnected,
                },
            };
            VolumeStatus {
                serial: group.serial,
                recorded_root: group.root,
                files: group.files,
                state,
            }
        })
        .collect();
    statuses.sort_by(|a, b| {
        platform::path_key(&a.recorded_root).cmp(&platform::path_key(&b.recorded_root))
    });
    Ok(statuses)
}

/// The disk, root and place on the disk of a location: its root is what
/// remains of its path once its place on the disk is taken off.
fn placed(location: &CurrentLocation) -> (Option<String>, String, Option<String>) {
    if let (Some(serial), Some(relative)) = (&location.volume_serial, &location.volume_relative_key)
    {
        let components: Vec<Component> = Path::new(&location.path).components().collect();
        let depth = Path::new(relative).components().count();
        if depth > 0 && depth < components.len() {
            let root: PathBuf = components[..components.len() - depth].iter().collect();
            return (
                Some(serial.clone()),
                root.display().to_string(),
                Some(relative.clone()),
            );
        }
    }
    (None, lexical_root(&location.path), None)
}

fn lexical_root(path: &str) -> String {
    let root: PathBuf = Path::new(path)
        .components()
        .take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
        .collect();
    root.display().to_string()
}

/// Among the roots holding the disk with this serial now, the one where its
/// files are: the serial alone is not enough (a `subst` drive shares the
/// serial of the disk it shows).
fn where_now(
    serial: &str,
    samples: &[String],
    mounted: &[(PathBuf, Option<String>)],
    recorded: &Path,
) -> Option<PathBuf> {
    mounted
        .iter()
        .filter(|(root, s)| {
            s.as_deref() == Some(serial)
                && platform::path_key(&root.to_string_lossy())
                    != platform::path_key(&recorded.to_string_lossy())
        })
        .map(|(root, _)| {
            let hits = samples.iter().filter(|r| root.join(r).exists()).count();
            (root, hits)
        })
        .filter(|(_, hits)| *hits > 0)
        .max_by_key(|(_, hits)| *hits)
        .map(|(root, _)| root.clone())
}

/// Moving every reference under one folder (or disk root) to another.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Reassign {
    pub from: String,
    pub to: String,
    /// Only the files of this disk: a disk that came back under another
    /// drive letter (their places on it do not change).
    #[serde(default)]
    pub serial: Option<String>,
    /// Move also the files not found at the new place (a disk not plugged in
    /// now, for instance).
    #[serde(default)]
    pub include_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlannedMove {
    pub from: String,
    pub to: String,
    /// A file is at the new place.
    pub found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReassignPreview {
    pub files: u32,
    pub found: u32,
    pub missing: u32,
    /// The first ones, to show.
    pub examples: Vec<PlannedMove>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReassignReport {
    pub moved: u32,
    /// Not found at the new place, and left where they were recorded.
    pub left: u32,
    /// New places already held by another file of the database: left alone.
    pub taken: Vec<String>,
}

const EXAMPLES: usize = 10;

struct Planned {
    location: CurrentLocation,
    to: PathBuf,
    found: bool,
}

/// What a reassignment would do, without doing it.
pub fn preview(store: &Store, request: &Reassign) -> Result<ReassignPreview, Problem> {
    let planned = plan(store, request)?;
    let found = planned.iter().filter(|p| p.found).count() as u32;
    Ok(ReassignPreview {
        files: planned.len() as u32,
        found,
        missing: planned.len() as u32 - found,
        examples: planned
            .iter()
            .take(EXAMPLES)
            .map(|p| PlannedMove {
                from: p.location.path.clone(),
                to: p.to.display().to_string(),
                found: p.found,
            })
            .collect(),
    })
}

/// Records the new location of every file under `from`, now under `to`.
/// Former locations stay as history; the change is one operation.
pub fn reassign(store: &mut Store, request: &Reassign) -> Result<ReassignReport, Problem> {
    let planned = plan(store, request)?;
    let mut left = 0;
    let mut moves = Vec::with_capacity(planned.len());
    for planned in planned {
        if !planned.found && !request.include_missing {
            left += 1;
            continue;
        }
        let Ok(text) = platform::path_text(&planned.to) else {
            left += 1;
            continue;
        };
        moves.push((
            planned.location.item_id,
            new_location(&planned, text, request),
        ));
    }
    let operation = store.begin_operation(OperationKind::Attach)?;
    let report = store.relocate_many(&moves).map(|done| ReassignReport {
        moved: done.moved,
        left,
        taken: done.taken,
    });
    let summary = serde_json::json!({
        "reassign": { "from": request.from, "to": request.to, "serial": request.serial },
        "moved": report.as_ref().map(|r| r.moved).unwrap_or(0),
        "left": left,
    });
    let status = if report.is_ok() {
        OperationStatus::Completed
    } else {
        OperationStatus::Failed
    };
    if let Err(problem) = store.finish_operation(operation, status, Some(&summary.to_string())) {
        tracing::warn!(%problem, "reassignment not closed in the database");
    }
    let report = report?;
    tracing::info!(
        from = request.from,
        to = request.to,
        moved = report.moved,
        left = report.left,
        "root reassigned"
    );
    Ok(report)
}

fn plan(store: &Store, request: &Reassign) -> Result<Vec<Planned>, Problem> {
    let io_problem = |err: std::io::Error| Problem::from_io(&err);
    let from = platform::absolute(Path::new(&request.from)).map_err(io_problem)?;
    let to = platform::absolute(Path::new(&request.to)).map_err(io_problem)?;
    let key = |path: &Path| platform::path_key(&path.to_string_lossy());
    if key(&from) == key(&to) {
        return Err(Problem::new(
            ProblemKind::InvalidName,
            "the new place is the same as the old one",
        ));
    }
    let depth = from.components().count();
    let mut planned = Vec::new();
    for location in store.current_locations()? {
        if request.serial.is_some() && location.volume_serial != request.serial {
            continue;
        }
        let path = Path::new(&location.path);
        let components: Vec<Component> = path.components().collect();
        if components.len() <= depth {
            continue;
        }
        let head: PathBuf = components[..depth].iter().collect();
        if key(&head) != key(&from) {
            continue;
        }
        let rest: PathBuf = components[depth..].iter().collect();
        let target = to.join(rest);
        let found = std::fs::metadata(platform::extended(&target)).is_ok_and(|m| m.is_file());
        planned.push(Planned {
            location,
            to: target,
            found,
        });
    }
    Ok(planned)
}

fn new_location(planned: &Planned, text: String, request: &Reassign) -> NewLocation {
    let old = &planned.location;
    let file_name = planned
        .to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let metadata = planned
        .found
        .then(|| std::fs::metadata(platform::extended(&planned.to)).ok())
        .flatten();
    // Same disk under another letter: same place on it. Elsewhere, the new
    // place is read from the file, if it is there.
    let (volume_serial, volume_relative_key) = if request.serial.is_some() {
        (old.volume_serial.clone(), old.volume_relative_key.clone())
    } else if planned.found {
        let place = platform::identify(&planned.to)
            .ok()
            .and_then(|state| platform::volume_place(&planned.to, state.id));
        (
            place.as_ref().map(|p| p.serial.clone()),
            place.map(|p| p.relative_key),
        )
    } else {
        (None, None)
    };
    NewLocation {
        path_key: platform::path_key(&text),
        path: text,
        file_name,
        last_size: metadata.as_ref().map(|m| m.len()).or(old.last_size),
        last_mtime: metadata
            .and_then(|m| m.modified().ok())
            .map(timestamp::from_system)
            .or_else(|| old.last_mtime.clone()),
        volume_serial,
        volume_relative_key,
    }
}

#[cfg(test)]
mod tests;
