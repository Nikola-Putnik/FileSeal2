//! Decisions that only the user makes, each one explicit and recorded (R8):
//! attaching a file to a known item (it moved, was renamed, or its drive
//! got another letter), and accepting a file's current content as the new
//! reference.
//!
//! Both act on what a verification or a search observed, without reading
//! the file again; they refuse if the file looks changed since (size or
//! last write time): it must then be verified again.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::content::agrees;
use super::{SidecarResult, WriteOutcome, locate};
use crate::algorithm::{Algorithm, Digest};
use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::reader::HashedFile;
use crate::sidecar::{self, SidecarLookup};
use crate::store::{NewLocation, NewVerification, OperationKind, OperationStatus, Store};
use crate::timestamp;

/// What was read from a file, as a verification or a search reported it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Observation {
    #[ts(type = "number")]
    pub size: u64,
    pub modified: Option<String>,
    pub digests: Vec<Digest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AttachReport {
    pub path: String,
    #[ts(type = "number")]
    pub item_id: i64,
    /// Where the item was recorded before.
    pub previous_path: String,
    /// False when attached despite a different content (renamed and
    /// modified): its new version can then be accepted.
    pub same_content: bool,
    #[ts(type = "number")]
    pub verification_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AcceptReport {
    pub path: String,
    /// The database's new reference: written, failed, or `None` if not asked.
    pub database: Option<WriteOutcome>,
    pub database_problem: Option<Problem>,
    /// Hash files next to the file, replaced with the accepted content.
    pub sidecars: Vec<SidecarResult>,
}

/// The file must look as it did when it was read: same size, same last
/// write time. Not proof that the content is the same (only reading is),
/// but enough to catch a file changed since.
fn unchanged_since(path: &Path, observed: &Observation) -> Result<(), Problem> {
    let metadata = std::fs::metadata(path).map_err(|err| Problem::from_io(&err))?;
    let modified = metadata.modified().ok().map(timestamp::from_system);
    if metadata.len() != observed.size
        || (observed.modified.is_some() && modified != observed.modified)
    {
        return Err(Problem::new(
            ProblemKind::StaleVerification,
            format!(
                "{} changed since it was read: verify it again",
                path.display()
            ),
        ));
    }
    Ok(())
}

/// R8: records that `item_id` is now the file at `path`. Its content must
/// be the item's reference, unless `allow_different` (the user associates a
/// renamed and modified file by hand). The previous location stays in the
/// history; a verification records the decision.
pub fn attach(
    path: &Path,
    item_id: i64,
    observed: &Observation,
    allow_different: bool,
    store: &mut Store,
) -> Result<AttachReport, Problem> {
    let (path, location) = locate(path)?;
    unchanged_since(&path, observed)?;
    let current = store.current_of_item(item_id)?.ok_or_else(|| {
        Problem::new(
            ProblemKind::NotFound,
            format!("item {item_id} has no current reference"),
        )
    })?;
    let read = HashedFile {
        size: observed.size,
        modified: None,
        id: None,
        digests: observed.digests.clone(),
    };
    let same_content = agrees(&current, &read);
    if !same_content && !allow_different {
        return Err(Problem::new(
            ProblemKind::ContentDiffers,
            format!(
                "{} is not the content recorded for item {item_id}",
                location.path
            ),
        ));
    }
    let id = platform::identify(&path).ok().and_then(|state| state.id);
    let place = platform::volume_place(&path, id);
    let new_location = NewLocation {
        last_size: Some(observed.size),
        last_mtime: observed.modified.clone(),
        volume_serial: place.as_ref().map(|p| p.serial.clone()),
        volume_relative_key: place.map(|p| p.relative_key),
        ..location.clone()
    };
    store.relocate(item_id, &new_location)?;
    let operation_id = store.begin_operation(OperationKind::Attach)?;
    let detail = format!("attached; previously at {}", current.path);
    let started_at = timestamp::now();
    let verification_id = store.record_verification(&NewVerification {
        operation_id,
        item_id: Some(item_id),
        reference_id: Some(current.reference_id),
        observed_path: &location.path,
        observed_size: Some(observed.size),
        observed_mtime: observed.modified.clone(),
        started_at: &started_at,
        outcome: if same_content { "match" } else { "mismatch" },
        detail: Some(&detail),
        digests: &observed.digests,
    })?;
    store.finish_operation(operation_id, OperationStatus::Completed, None)?;
    tracing::info!(item_id, path = %location.path, from = %current.path, "attached");
    Ok(AttachReport {
        path: location.path,
        item_id,
        previous_path: current.path,
        same_content,
        verification_id,
    })
}

/// Accepts the content observed by a verification as the new reference:
/// in the database (if `verification_id`, the previous reference stays in
/// the history) and in the hash files next to the file (if
/// `update_sidecars`: existing ones are replaced, none is created).
pub fn accept(
    path: &Path,
    observed: &Observation,
    verification_id: Option<i64>,
    update_sidecars: bool,
    store: Option<&mut Store>,
) -> Result<AcceptReport, Problem> {
    let (path, location) = locate(path)?;
    unchanged_since(&path, observed)?;
    let mut report = AcceptReport {
        path: location.path,
        database: None,
        database_problem: None,
        sidecars: Vec::new(),
    };
    if let (Some(store), Some(verification_id)) = (store, verification_id) {
        match store.accept_version(verification_id) {
            Ok(_) => report.database = Some(WriteOutcome::Written),
            Err(problem) => {
                report.database = Some(WriteOutcome::Failed);
                report.database_problem = Some(problem);
            }
        }
    }
    if update_sidecars {
        for algorithm in Algorithm::ALL {
            let Some(accepted) = observed.digests.iter().find(|d| d.algorithm == algorithm) else {
                continue;
            };
            let target = sidecar::path_for(&path, algorithm).unwrap_or_default();
            let (existing, outcome, problem) = match sidecar::lookup(&path, algorithm) {
                SidecarLookup::Absent => continue,
                SidecarLookup::Found { digest, .. } if digest == *accepted => {
                    (Some(digest.hex), WriteOutcome::AlreadyPresent, None)
                }
                found => {
                    let existing = match found {
                        SidecarLookup::Found { digest, .. } => Some(digest.hex),
                        _ => None,
                    };
                    match sidecar::replace(&path, accepted) {
                        Ok(_) => (existing, WriteOutcome::Written, None),
                        Err(problem) => (existing, WriteOutcome::Failed, Some(problem)),
                    }
                }
            };
            report.sidecars.push(SidecarResult {
                algorithm,
                path: target.display().to_string(),
                requested: true,
                outcome,
                existing,
                notes: Vec::new(),
                problem,
            });
        }
    }
    tracing::info!(path = %report.path, ?verification_id, "new version accepted");
    Ok(report)
}
