//! "Is this content already known?" (R1, R2, R5, R7). A search writes
//! nothing: it only tells what the database knows, and what could be done
//! (attach a moved file, accept a new version).

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use super::content::{self, ContentFinding, agrees};
use super::{locate, recorded_reference};
use crate::algorithm::Digest;
use crate::matching::ContentOutcome;
use crate::problem::{Problem, ProblemKind};
use crate::reader::{self, Control};
use crate::store::SharedStore;
use crate::timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SearchStatus {
    /// No current reference has this content.
    Unknown,
    /// Recorded at this location, with this content.
    KnownHere,
    /// Recorded at this location, but its content changed since.
    ChangedHere,
    /// Recorded at the same place of the same volume, under another drive
    /// letter, with this content (R2).
    LocationRecognized,
    /// A known item with this content is missing from its location: this
    /// file is most likely it, moved or renamed (R5).
    ProbablyMoved,
    /// Known items with this content are where they should be: another copy.
    CopyOfKnown,
    /// Several known items with this content are missing: choose.
    Ambiguous,
    NotFound,
    ReadError,
    Locked,
    ModifiedDuringRead,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchReport {
    pub path: String,
    pub status: SearchStatus,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    pub digests: Vec<Digest>,
    pub observed_mtime: Option<String>,
    /// The item recorded at this location (directly or through R2).
    #[ts(type = "number | null")]
    pub item_id: Option<i64>,
    /// R2: where that item was recorded, under another drive letter.
    pub location_change: Option<String>,
    pub finding: Option<ContentFinding>,
    pub problem: Option<Problem>,
}

/// Looks the content of one file up in the database.
pub fn search_file(
    path: &Path,
    store: &SharedStore,
    control: &Control,
    progress: &mut dyn FnMut(u64, u64),
) -> SearchReport {
    let mut report = SearchReport {
        path: path.display().to_string(),
        status: SearchStatus::Failed,
        size: None,
        digests: Vec::new(),
        observed_mtime: None,
        item_id: None,
        location_change: None,
        finding: None,
        problem: None,
    };
    let (path, location) = match locate(path) {
        Ok(found) => found,
        Err(problem) => return report.fail(problem),
    };
    report.path = location.path.clone();
    if !path.exists() {
        return report.fail(Problem::new(
            ProblemKind::NotFound,
            format!("{} does not exist", location.path),
        ));
    }
    let (here, moved_from) = match recorded_reference(store, &path, &location) {
        Ok(found) => found,
        Err(problem) => return report.fail(problem),
    };
    report.item_id = here.as_ref().map(|k| k.item_id);
    report.location_change = moved_from;

    let size = match reader::file_size(&path) {
        Ok(size) => size,
        Err(problem) => return report.fail(problem),
    };
    report.size = Some(size);
    // No current reference of this size: unknown, without reading a byte.
    if here.is_none() && !store.lock().has_reference_of_size(size).unwrap_or(true) {
        report.status = SearchStatus::Unknown;
        report.finding = content::name_hints(store, &location.file_name, None)
            .ok()
            .filter(|hints| !hints.is_empty())
            .map(|hints| ContentFinding {
                outcome: ContentOutcome::Unknown,
                same_content: Vec::new(),
                hints,
            });
        return report;
    }
    let in_use = store.lock().algorithms_in_use();
    let mut algorithms = match in_use {
        Ok(algorithms) => algorithms,
        Err(problem) => return report.fail(problem),
    };
    if let Some(here) = &here {
        algorithms.extend(here.digests.iter().map(|d| d.algorithm));
    }
    if algorithms.is_empty() {
        report.status = SearchStatus::Unknown;
        return report;
    }

    let hashed = match reader::hash_file(&path, &algorithms, control, progress) {
        Ok(hashed) => hashed,
        Err(problem) => return report.fail(problem),
    };
    report.size = Some(hashed.size);
    report.digests = hashed.digests.clone();
    report.observed_mtime = hashed.modified.map(timestamp::from_system);
    let finding = match content::find(
        store,
        &location.file_name,
        here.as_ref().map(|k| k.item_id),
        &hashed,
    ) {
        Ok(finding) => finding,
        Err(problem) => return report.fail(problem),
    };
    report.status = match &here {
        Some(here) if agrees(here, &hashed) && report.location_change.is_some() => {
            SearchStatus::LocationRecognized
        }
        Some(here) if agrees(here, &hashed) => SearchStatus::KnownHere,
        Some(_) => SearchStatus::ChangedHere,
        None => match finding.outcome {
            ContentOutcome::Unknown | ContentOutcome::KnownHere => SearchStatus::Unknown,
            ContentOutcome::ProbablyMoved => SearchStatus::ProbablyMoved,
            ContentOutcome::CopyOfKnown => SearchStatus::CopyOfKnown,
            ContentOutcome::Ambiguous => SearchStatus::Ambiguous,
        },
    };
    if !finding.same_content.is_empty() || !finding.hints.is_empty() {
        report.finding = Some(finding);
    }
    tracing::info!(path = %report.path, status = ?report.status, "search");
    report
}

impl SearchReport {
    fn fail(mut self, problem: Problem) -> SearchReport {
        self.status = match problem.kind {
            ProblemKind::Cancelled => SearchStatus::Cancelled,
            ProblemKind::NotFound => SearchStatus::NotFound,
            ProblemKind::Locked => SearchStatus::Locked,
            ProblemKind::ModifiedDuringRead => SearchStatus::ModifiedDuringRead,
            ProblemKind::PermissionDenied
            | ProblemKind::Io
            | ProblemKind::NotAFile
            | ProblemKind::DeviceUnavailable
            | ProblemKind::NetworkError
            | ProblemKind::MediaError => SearchStatus::ReadError,
            _ => SearchStatus::Failed,
        };
        tracing::warn!(path = %self.path, %problem, "search failed");
        self.problem = Some(problem);
        self
    }
}
