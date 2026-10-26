//! The actions on one file: create a reference, verify a file against its
//! references. Batches (step 7 of the plan) call these once per file.
//!
//! Rules applied here (numbers refer to `docs/cadrage.md`, §2):
//! - creating never overwrites: an existing reference that agrees is kept,
//!   one that disagrees (or cannot be read) blocks every write for that
//!   file, so modified content is never blessed silently (rules 2–4);
//! - verifying never writes a reference; the file is read in full and
//!   compared with every reference found, and references that disagree with
//!   each other give "conflict", not a choice (rules 8–9);
//! - the database reference of a file is found by its path (R1), else by
//!   its place on its volume (R2: drive letter changed); otherwise its
//!   content is looked up (R4, R5) and a probable move is proposed, never
//!   applied (docs/rapprochement.md), unless the user chose to follow
//!   locations automatically (`follow`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::algorithm::{self, Algorithm, Digest};
use crate::matching::ContentOutcome;
use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::reader::{self, Control, HashedFile};
use crate::sidecar::{self, SidecarLookup, SidecarNote};
use crate::store::{KnownReference, NewLocation, NewVerification, SharedStore};
use crate::sums;
use crate::timestamp;

pub mod content;
pub mod decide;
mod follow;
pub mod items;
pub mod manifest;
pub mod search;
pub mod volumes;

use content::{ContentFinding, agrees};

/// The database an action writes to, and the operation it belongs to. It is
/// locked for each lookup or write, never while the file is read.
#[derive(Clone, Copy)]
pub struct DbTarget<'a> {
    pub store: &'a SharedStore<'a>,
    pub operation_id: i64,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateOptions {
    pub algorithms: Vec<Algorithm>,
    /// Write `file.<algo>` next to the file.
    pub write_sidecar: bool,
    /// Add the file's line to its folder's hash list (`SHA256SUMS`,
    /// `MD5SUMS`), written once every file of the folder is done.
    #[serde(default)]
    pub write_sums: bool,
    /// Create a new item even when the content looks like a known item that
    /// moved here (the user decided it is another copy).
    #[serde(default)]
    pub new_item: bool,
    /// A file that already has references: read again, or skipped.
    #[serde(default)]
    pub existing: ExistingReferences,
    /// A skipped file unknown to the database: its hash files are recorded
    /// there as an imported reference, still without reading it.
    #[serde(default)]
    pub import_hash_files: bool,
    /// A file found elsewhere than recorded is attached to its item without
    /// asking (`follow`).
    #[serde(default)]
    pub follow_moves: bool,
}

/// What creating does with a file that already has references.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExistingReferences {
    /// Read: its references are checked against its content, and completed
    /// (new algorithm, new destination) only if they agree.
    #[default]
    Read,
    /// Not read when every destination asked has a reference for every
    /// algorithm asked.
    SkipComplete,
    /// Not read when every destination asked has a reference, whatever its
    /// algorithm: an old `.md5` is enough, even with SHA-256 asked.
    SkipAny,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VerifyOptions {
    /// Look for `file.<algo>` next to the file, and for the file's line in
    /// its folder's hash list (`SHA256SUMS`).
    pub use_sidecar: bool,
    /// A file found elsewhere than recorded is attached to its item without
    /// asking (`follow`).
    #[serde(default)]
    pub follow_moves: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CreateStatus {
    /// New references written everywhere requested.
    Created,
    /// An existing reference was confirmed and completed (new algorithm or
    /// new destination).
    Completed,
    /// Everything requested already existed, identical.
    AlreadyReferenced,
    /// Already referenced, and not read (`ExistingReferences::Skip…`): its
    /// references were not checked.
    Skipped,
    /// Not read; its hash files were recorded in the database.
    Imported,
    /// An existing reference disagrees or is unreadable: nothing written.
    Conflict,
    /// At least one destination failed and at least one succeeded.
    PartialSuccess,
    /// The content is a known item that seems to have moved here: the user
    /// decides between attaching it and creating a new item.
    NeedsDecision,
    /// Provisional: its line waits for the folder's hash list, written once
    /// every file of the folder is done. Replaced then by the final status.
    Pending,
    Failed,
    Cancelled,
}

/// What happened to one destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WriteOutcome {
    Written,
    /// Database: digests of new algorithms added to the existing reference.
    Completed,
    /// Identical reference already there; left as it was.
    AlreadyPresent,
    /// Existing reference with another value; left as it was.
    Conflict,
    /// Existing hash file that cannot be used; left as it was.
    Invalid,
    /// Not attempted because another reference of this file is in conflict.
    Skipped,
    /// Database: not written, waiting for the user's decision.
    NeedsDecision,
    /// Reference already there, kept without reading the file (so without
    /// checking it).
    NotRead,
    /// Database: recorded from the file's hash files, without reading it.
    Imported,
    /// Folder's hash list: written once every file of the folder is done.
    Pending,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SidecarResult {
    pub algorithm: Algorithm,
    pub path: String,
    /// False for a hash file that was not asked for but was checked because
    /// it exists (an existing `.md5` when creating SHA-256).
    pub requested: bool,
    /// A line of the folder's hash list (`SHA256SUMS`), not a hash file of
    /// its own.
    pub list: bool,
    pub outcome: WriteOutcome,
    /// Value found in an existing hash file.
    pub existing: Option<String>,
    pub notes: Vec<SidecarNote>,
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DatabaseResult {
    pub outcome: WriteOutcome,
    /// Digests of the reference that already existed.
    pub existing: Vec<Digest>,
    #[ts(type = "number | null")]
    pub existing_size: Option<u64>,
    pub added: Vec<Algorithm>,
    pub problem: Option<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateReport {
    pub path: String,
    pub status: CreateStatus,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    pub digests: Vec<Digest>,
    /// Its hash files, and its lines in its folder's hash lists.
    pub sidecars: Vec<SidecarResult>,
    pub database: Option<DatabaseResult>,
    /// Why the file could not be read, if it could not.
    pub problem: Option<Problem>,
    /// The database item of this file, once known or created.
    #[ts(type = "number | null")]
    pub item_id: Option<i64>,
    /// R2: the item was recorded at this other path (same place on the same
    /// volume, under another drive letter).
    pub location_change: Option<String>,
    /// What the database knows about this content, when the file's own
    /// location was not recorded.
    pub finding: Option<ContentFinding>,
    /// Locations followed: the item was recorded at this other path, and
    /// now is at this one.
    pub relocated_from: Option<String>,
}

/// Creates the references of one file in the requested destinations.
pub fn create_reference(
    path: &Path,
    options: &CreateOptions,
    db: Option<DbTarget>,
    control: &Control,
    progress: &mut dyn FnMut(u64, u64),
) -> CreateReport {
    let mut report = CreateReport {
        path: path.display().to_string(),
        status: CreateStatus::Failed,
        size: None,
        digests: Vec::new(),
        sidecars: Vec::new(),
        database: None,
        problem: None,
        item_id: None,
        location_change: None,
        finding: None,
        relocated_from: None,
    };
    let (path, location) = match locate(path) {
        Ok(found) => found,
        Err(problem) => return report.fail(problem),
    };
    report.path = location.path.clone();
    let mut selected = options.algorithms.clone();
    selected.sort_unstable();
    selected.dedup();
    if selected.is_empty() || (!options.write_sidecar && !options.write_sums && db.is_none()) {
        return report.fail(Problem::new(
            ProblemKind::Io,
            "no algorithm or no destination selected",
        ));
    }

    // Existing references: every hash file and folder list line present
    // (requested or not), and the database entry for this path.
    let mut sidecars: Vec<HashRef> = Vec::new();
    for algorithm in Algorithm::ALL {
        for list in [false, true] {
            let asked = if list {
                options.write_sums
            } else {
                options.write_sidecar
            };
            let requested = asked && selected.contains(&algorithm);
            let (lookup, target) = if list {
                (
                    sums::lookup(&path, algorithm),
                    sums::path_for(&path, algorithm),
                )
            } else {
                (
                    sidecar::lookup(&path, algorithm),
                    sidecar::path_for(&path, algorithm),
                )
            };
            if requested || lookup != SidecarLookup::Absent {
                sidecars.push(HashRef {
                    algorithm,
                    path: target.unwrap_or_default(),
                    lookup,
                    requested,
                    list,
                });
            }
        }
    }
    let mut database = db.as_ref().map(|_| DatabaseResult {
        outcome: WriteOutcome::Skipped,
        existing: Vec::new(),
        existing_size: None,
        added: Vec::new(),
        problem: None,
    });
    let mut known: Option<KnownReference> = None;
    if let (Some(target), Some(result)) = (db, database.as_mut()) {
        match recorded_reference(target.store, &path, &location) {
            Ok((found, moved_from)) => {
                if let Some(found) = &found {
                    result.existing = found.digests.clone();
                    result.existing_size = Some(found.size);
                    report.item_id = Some(found.item_id);
                }
                report.location_change = moved_from;
                known = found;
            }
            Err(problem) => {
                result.outcome = WriteOutcome::Failed;
                result.problem = Some(problem);
            }
        }
    }
    if options.follow_moves
        && let (Some(target), Some(known)) = (db, &known)
    {
        report.relocated_from = follow_volume(
            &mut report.location_change,
            target,
            &path,
            &location,
            known.item_id,
        );
    }
    if options.existing != ExistingReferences::Read {
        let unread = without_reading(
            options,
            &selected,
            &sidecars,
            database.as_ref(),
            known.as_ref(),
            report.location_change.is_some(),
        );
        match unread {
            Some(Unread::Skip) => {
                return skipped(report, &path, sidecars, database, known.as_ref());
            }
            Some(Unread::Import(digests)) => {
                let target = db.expect("a database result means a database");
                if let Some(done) = imported(
                    report.clone(),
                    &path,
                    &location,
                    options,
                    target,
                    &sidecars,
                    digests,
                ) {
                    return done;
                }
            }
            None => {}
        }
    }
    // Unknown location: hash with the database's algorithms too, so that
    // the content can be looked up (a file moved here is not a new item).
    let look_up_content =
        db.is_some() && known.is_none() && database.as_ref().is_some_and(|d| d.problem.is_none());

    // Read once, with every algorithm needed to check the existing references.
    let mut needed = selected.clone();
    for existing in &sidecars {
        if let SidecarLookup::Found { digest, .. } = &existing.lookup {
            needed.push(digest.algorithm);
        }
    }
    if let Some(known) = &known {
        needed.extend(known.digests.iter().map(|d| d.algorithm));
    }
    if look_up_content && let Some(target) = db {
        let in_use = target.store.lock().algorithms_in_use();
        needed.extend(in_use.unwrap_or_default());
    }
    let hashed = match reader::hash_file(&path, &needed, control, progress) {
        Ok(hashed) => hashed,
        Err(problem) => {
            report.sidecars = sidecars
                .iter()
                .map(|existing| sidecar_result(existing, WriteOutcome::Skipped))
                .collect();
            report.database = database;
            return report.fail(problem);
        }
    };
    report.size = Some(hashed.size);

    // A content the database already has, at a location it does not know:
    // probably the same file moved here. Not written until the user says
    // whether it moved (attach) or is another copy (new item), unless
    // locations are followed: then it is attached at once.
    let mut deferred = false;
    if look_up_content && let (Some(target), Some(result)) = (db, database.as_mut()) {
        match content::find(target.store, &location.file_name, None, &hashed) {
            Ok(mut finding) => {
                let followed = (options.follow_moves && !options.new_item)
                    .then(|| follow::content(target.store, &path, &location, &hashed, &mut finding))
                    .flatten();
                if let Some((attached, from)) = followed {
                    result.existing = attached.digests.clone();
                    result.existing_size = Some(attached.size);
                    report.item_id = Some(attached.item_id);
                    report.relocated_from = Some(from);
                    known = Some(attached);
                } else {
                    deferred = !options.new_item
                        && matches!(
                            finding.outcome,
                            ContentOutcome::ProbablyMoved | ContentOutcome::Ambiguous
                        );
                    if finding.outcome != ContentOutcome::Unknown || !finding.hints.is_empty() {
                        report.finding = Some(finding);
                    }
                }
            }
            Err(problem) => tracing::warn!(%problem, "content not looked up"),
        }
    }
    report.digests = hashed
        .digests
        .iter()
        .filter(|d| needed_for_report(d.algorithm, &selected, &sidecars, known.as_ref()))
        .cloned()
        .collect();

    // Check the existing references against what was just read.
    let mut blocked = false;
    let mut results: Vec<SidecarResult> = Vec::new();
    for existing in &sidecars {
        let outcome = match &existing.lookup {
            SidecarLookup::Absent => WriteOutcome::Skipped, // decided below
            SidecarLookup::Found { digest, .. }
                if computed(&hashed, existing.algorithm) == Some(digest) =>
            {
                WriteOutcome::AlreadyPresent
            }
            SidecarLookup::Found { .. } => WriteOutcome::Conflict,
            SidecarLookup::Invalid { .. } => WriteOutcome::Invalid,
        };
        blocked |= matches!(outcome, WriteOutcome::Conflict | WriteOutcome::Invalid);
        results.push(sidecar_result(existing, outcome));
    }
    if let (Some(known), Some(result)) = (&known, database.as_mut()) {
        if agrees(known, &hashed) {
            result.outcome = WriteOutcome::AlreadyPresent;
        } else {
            result.outcome = WriteOutcome::Conflict;
            blocked = true;
        }
    }

    if !blocked {
        for (result, existing) in results.iter_mut().zip(&sidecars) {
            if existing.requested && existing.lookup == SidecarLookup::Absent {
                // A folder's list is written once for all its files, by
                // `finish_sums`: the line waits until then.
                if existing.list {
                    result.outcome = WriteOutcome::Pending;
                    continue;
                }
                let digest =
                    computed(&hashed, result.algorithm).expect("selected algorithm hashed");
                match sidecar::write_new(&path, digest) {
                    Ok(_) => result.outcome = WriteOutcome::Written,
                    Err(problem) => {
                        result.outcome = WriteOutcome::Failed;
                        result.problem = Some(problem);
                    }
                }
            }
        }
        if let (Some(target), Some(result)) = (db, database.as_mut())
            && result.problem.is_none()
        {
            let wanted: Vec<Digest> = selected
                .iter()
                .filter_map(|a| computed(&hashed, *a).cloned())
                .collect();
            let written = match &known {
                Some(known) => {
                    let missing: Vec<Digest> = wanted
                        .into_iter()
                        .filter(|d| algorithm::find(&known.digests, d.algorithm).is_none())
                        .collect();
                    let added: Vec<Algorithm> = missing.iter().map(|d| d.algorithm).collect();
                    let write = if missing.is_empty() {
                        Ok(())
                    } else {
                        target
                            .store
                            .lock()
                            .add_digests(known.reference_id, &missing)
                    };
                    write.map(|()| {
                        touch(target.store, known.item_id, &hashed);
                        if !added.is_empty() {
                            result.outcome = WriteOutcome::Completed;
                            result.added = added;
                        }
                    })
                }
                None if deferred => {
                    result.outcome = WriteOutcome::NeedsDecision;
                    Ok(())
                }
                None => {
                    let observed = observed_location(&path, &location, &hashed);
                    let inserted =
                        target
                            .store
                            .lock()
                            .insert_reference(&observed, hashed.size, &wanted);
                    inserted.map(|inserted| {
                        result.outcome = WriteOutcome::Written;
                        result.added = selected.clone();
                        report.item_id = Some(inserted.item_id);
                    })
                }
            };
            if let Err(problem) = written {
                result.outcome = WriteOutcome::Failed;
                result.problem = Some(problem);
            }
        }
    }

    report.status = create_status(blocked, &results, database.as_ref());
    report.sidecars = results;
    report.database = database;
    tracing::info!(path = %report.path, status = ?report.status, "create reference");
    report
}

/// What to do with a file without reading it.
enum Unread {
    /// Every destination asked already has what was asked.
    Skip,
    /// Its hash files say enough: recorded in the database as they are.
    Import(Vec<Digest>),
}

/// Whether a file can be left unread (`ExistingReferences::Skip…`). Never
/// when a reference would have something to report: an unusable hash file,
/// references that disagree with each other, a new drive letter. Reading
/// it then says what is wrong, as without the option.
fn without_reading(
    options: &CreateOptions,
    selected: &[Algorithm],
    sidecars: &[HashRef],
    database: Option<&DatabaseResult>,
    known: Option<&KnownReference>,
    moved: bool,
) -> Option<Unread> {
    if moved
        || sidecars
            .iter()
            .any(|s| matches!(s.lookup, SidecarLookup::Invalid { .. }))
    {
        return None;
    }
    let found_in = |list: bool| -> Vec<&Digest> {
        sidecars
            .iter()
            .filter(|s| s.list == list)
            .filter_map(|s| match &s.lookup {
                SidecarLookup::Found { digest, .. } => Some(digest),
                _ => None,
            })
            .collect()
    };
    let mut found: Vec<&Digest> = found_in(false);
    found.extend(found_in(true));
    // References that disagree with each other: reading says which is right.
    if found
        .iter()
        .any(|a| found.iter().any(|b| a.algorithm == b.algorithm && a != b))
    {
        return None;
    }
    if let Some(known) = known
        && found.iter().any(|d| {
            algorithm::find(&known.digests, d.algorithm).is_some_and(|recorded| recorded != *d)
        })
    {
        return None;
    }
    let covers = |digests: &[&Digest]| match options.existing {
        ExistingReferences::SkipAny => !digests.is_empty(),
        _ => selected
            .iter()
            .all(|a| digests.iter().any(|d| d.algorithm == *a)),
    };
    if (options.write_sidecar && !covers(&found_in(false)))
        || (options.write_sums && !covers(&found_in(true)))
    {
        return None;
    }
    let Some(database) = database else {
        return Some(Unread::Skip);
    };
    // The database could not be looked up: the usual way reports it.
    if database.problem.is_some() {
        return None;
    }
    match known {
        Some(known) => covers(&known.digests.iter().collect::<Vec<_>>()).then_some(Unread::Skip),
        None if options.import_hash_files && covers(&found) => {
            let mut digests: Vec<Digest> = Vec::new();
            for digest in found {
                if algorithm::find(&digests, digest.algorithm).is_none() {
                    digests.push(digest.clone());
                }
            }
            digests.sort_by_key(|d| d.algorithm);
            Some(Unread::Import(digests))
        }
        None => None,
    }
}

/// A file left unread: what it already has, as it is.
fn skipped(
    mut report: CreateReport,
    path: &Path,
    sidecars: Vec<HashRef>,
    mut database: Option<DatabaseResult>,
    known: Option<&KnownReference>,
) -> CreateReport {
    report.size = reader::file_size(path).ok();
    report.digests = existing_digests(&sidecars, known);
    report.sidecars = unread_sidecars(&sidecars);
    if let (Some(result), Some(_)) = (database.as_mut(), known) {
        result.outcome = WriteOutcome::NotRead;
    }
    report.database = database;
    report.status = CreateStatus::Skipped;
    tracing::info!(path = %report.path, "create reference: already referenced, not read");
    report
}

/// A file left unread, unknown to the database: its hash files are recorded
/// there. `None` when its content is a known item that seems to have moved
/// here: reading it leads to the usual decision (attach or new copy).
fn imported(
    mut report: CreateReport,
    path: &Path,
    location: &NewLocation,
    options: &CreateOptions,
    target: DbTarget,
    sidecars: &[HashRef],
    digests: Vec<Digest>,
) -> Option<CreateReport> {
    // Opened for its size, date and identity; its content is not read.
    let state = match platform::identify(path) {
        Ok(state) => state,
        Err(err) => return Some(report.fail(Problem::from_io(&err))),
    };
    let seen = HashedFile {
        size: state.len,
        modified: state.modified,
        id: state.id,
        digests,
    };
    match content::find(target.store, &location.file_name, None, &seen) {
        Ok(finding) => {
            if !options.new_item
                && matches!(
                    finding.outcome,
                    ContentOutcome::ProbablyMoved | ContentOutcome::Ambiguous
                )
            {
                return None;
            }
            if finding.outcome != ContentOutcome::Unknown {
                report.finding = Some(finding);
            }
        }
        Err(problem) => tracing::warn!(%problem, "content not looked up"),
    }
    report.size = Some(seen.size);
    report.digests = seen.digests.clone();
    report.sidecars = unread_sidecars(sidecars);
    let observed = observed_location(path, location, &seen);
    let inserted =
        target
            .store
            .lock()
            .insert_imported_reference(&observed, seen.size, &seen.digests);
    let mut result = DatabaseResult {
        outcome: WriteOutcome::Imported,
        existing: Vec::new(),
        existing_size: None,
        added: seen.digests.iter().map(|d| d.algorithm).collect(),
        problem: None,
    };
    match inserted {
        Ok(inserted) => {
            report.item_id = Some(inserted.item_id);
            report.status = CreateStatus::Imported;
        }
        Err(problem) => {
            result.outcome = WriteOutcome::Failed;
            result.added.clear();
            result.problem = Some(problem);
            report.status = CreateStatus::Failed;
        }
    }
    report.database = Some(result);
    tracing::info!(path = %report.path, status = ?report.status, "create reference, not read");
    Some(report)
}

/// The hash files of a file left unread: those present kept unchecked, the
/// others not written.
fn unread_sidecars(sidecars: &[HashRef]) -> Vec<SidecarResult> {
    sidecars
        .iter()
        .map(|existing| {
            let outcome = match existing.lookup {
                SidecarLookup::Found { .. } => WriteOutcome::NotRead,
                _ => WriteOutcome::Skipped,
            };
            sidecar_result(existing, outcome)
        })
        .collect()
}

/// Every digest a file's references already give, one per algorithm.
fn existing_digests(sidecars: &[HashRef], known: Option<&KnownReference>) -> Vec<Digest> {
    let mut digests: Vec<Digest> = known.map(|k| k.digests.clone()).unwrap_or_default();
    for existing in sidecars {
        if let SidecarLookup::Found { digest, .. } = &existing.lookup
            && algorithm::find(&digests, digest.algorithm).is_none()
        {
            digests.push(digest.clone());
        }
    }
    digests.sort_by_key(|d| d.algorithm);
    digests
}

fn create_status(
    blocked: bool,
    sidecars: &[SidecarResult],
    database: Option<&DatabaseResult>,
) -> CreateStatus {
    if blocked {
        return CreateStatus::Conflict;
    }
    if database.is_some_and(|d| d.outcome == WriteOutcome::NeedsDecision) {
        return CreateStatus::NeedsDecision;
    }
    // Only the destinations the user asked for decide the status.
    let outcomes: Vec<WriteOutcome> = sidecars
        .iter()
        .filter(|s| s.requested)
        .map(|s| s.outcome)
        .chain(database.map(|d| d.outcome))
        .collect();
    let failed = outcomes.contains(&WriteOutcome::Failed);
    let succeeded = outcomes.iter().any(|o| *o != WriteOutcome::Failed);
    // A line waiting for the folder's list: nothing failed so far, but
    // nothing is said done before it is written.
    if !failed && outcomes.contains(&WriteOutcome::Pending) {
        return CreateStatus::Pending;
    }
    let new = |o: &WriteOutcome| matches!(o, WriteOutcome::Written | WriteOutcome::Completed);
    let existed =
        |o: &WriteOutcome| matches!(o, WriteOutcome::AlreadyPresent | WriteOutcome::Completed);
    match (failed, succeeded) {
        (true, true) => CreateStatus::PartialSuccess,
        (true, false) => CreateStatus::Failed,
        _ if !outcomes.iter().any(new) => CreateStatus::AlreadyReferenced,
        _ if outcomes.iter().any(existed) => CreateStatus::Completed,
        _ => CreateStatus::Created,
    }
}

/// Writes the folder's hash lists of these creations (the lines left
/// pending), one publication per folder and algorithm, then gives each its
/// final status. What a list could not take (another value already there,
/// a list that cannot be read or written) fails for that file only.
pub fn finish_sums(reports: &mut [CreateReport]) {
    type Line = (usize, usize); // report, then its result
    let mut groups: Vec<((PathBuf, Algorithm), Vec<Line>)> = Vec::new();
    for (r, report) in reports.iter().enumerate() {
        for (s, result) in report.sidecars.iter().enumerate() {
            if result.outcome != WriteOutcome::Pending {
                continue;
            }
            let dir = Path::new(&result.path)
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf();
            let key = (dir, result.algorithm);
            match groups.iter_mut().find(|(k, _)| *k == key) {
                Some((_, lines)) => lines.push((r, s)),
                None => groups.push((key, vec![(r, s)])),
            }
        }
    }
    for ((dir, algorithm), lines) in groups {
        let mut asked = Vec::with_capacity(lines.len());
        let mut kept = Vec::with_capacity(lines.len());
        for (r, s) in lines {
            let report = &reports[r];
            let name = Path::new(&report.path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            match (name, algorithm::find(&report.digests, algorithm)) {
                (Some(name), Some(digest)) => {
                    asked.push((name, digest.clone()));
                    kept.push((r, s));
                }
                _ => {
                    let result = &mut reports[r].sidecars[s];
                    result.outcome = WriteOutcome::Failed;
                    result.problem = Some(Problem::new(ProblemKind::Io, "no line to write"));
                }
            }
        }
        let written = sums::add(&dir, algorithm, &asked);
        for (i, (r, s)) in kept.into_iter().enumerate() {
            let result = &mut reports[r].sidecars[s];
            match &written {
                Ok((_, outcomes)) => match &outcomes[i] {
                    sums::Added::Written => result.outcome = WriteOutcome::Written,
                    sums::Added::AlreadyPresent => result.outcome = WriteOutcome::AlreadyPresent,
                    sums::Added::Conflict { existing } => {
                        result.outcome = WriteOutcome::Failed;
                        result.existing = existing.clone();
                        result.problem = Some(Problem::new(
                            ProblemKind::AlreadyExists,
                            format!(
                                "{} already has another value for this file: left as it is",
                                sums::name_for(algorithm)
                            ),
                        ));
                    }
                },
                Err(problem) => {
                    result.outcome = WriteOutcome::Failed;
                    result.problem = Some(problem.clone());
                }
            }
        }
    }
    for report in reports.iter_mut() {
        if report.status == CreateStatus::Pending {
            report.status = create_status(false, &report.sidecars, report.database.as_ref());
            tracing::info!(path = %report.path, status = ?report.status, "folder hash list written");
        }
    }
}

/// The database reference of the file at `path`: by its path (R1), else by
/// its place on its volume (R2), with the path recorded then. R2 only
/// applies when that recorded path no longer exists: the same volume now
/// has another drive letter (or mount point).
pub(crate) fn recorded_reference(
    store: &SharedStore,
    path: &Path,
    location: &NewLocation,
) -> Result<(Option<KnownReference>, Option<String>), Problem> {
    let by_path = store.lock().current_by_path(&location.path_key)?;
    if let Some(known) = by_path {
        return Ok((Some(known), None));
    }
    let Some(place) = platform::identify(path)
        .ok()
        .and_then(|state| platform::volume_place(path, state.id))
    else {
        return Ok((None, None));
    };
    let by_volume = store
        .lock()
        .current_by_volume(&place.serial, &place.relative_key)?;
    match by_volume {
        Some(known) if !Path::new(&known.path).exists() => {
            let from = known.path.clone();
            Ok((Some(known), Some(from)))
        }
        _ => Ok((None, None)),
    }
}

/// R2 followed (`follow_moves`): the item found at the same place of the
/// same disk, under another drive letter, is recorded at `path` now. Its
/// former path moves from `location_change` (a change to propose) to the
/// value returned (a change made). A failure is logged and leaves the
/// change to propose, as without the option.
fn follow_volume(
    location_change: &mut Option<String>,
    target: DbTarget,
    path: &Path,
    location: &NewLocation,
    item_id: i64,
) -> Option<String> {
    location_change.as_ref()?;
    match follow::volume(target.store, path, location, item_id) {
        Ok(()) => {
            tracing::info!(item_id, path = %location.path, "location followed (drive letter)");
            location_change.take()
        }
        Err(problem) => {
            tracing::warn!(%problem, path = %location.path, "location not followed");
            None
        }
    }
}

/// The location record of a file just read: its path, its place on its
/// volume, and the size and date seen there.
pub(crate) fn observed_location(
    path: &Path,
    location: &NewLocation,
    hashed: &HashedFile,
) -> NewLocation {
    let place = platform::volume_place(path, hashed.id);
    NewLocation {
        last_size: Some(hashed.size),
        last_mtime: hashed.modified.map(timestamp::from_system),
        volume_serial: place.as_ref().map(|p| p.serial.clone()),
        volume_relative_key: place.map(|p| p.relative_key),
        ..location.clone()
    }
}

/// Digests worth showing for a creation: the requested algorithms and those
/// of existing references, not the extra ones computed to search.
fn needed_for_report(
    algorithm: Algorithm,
    selected: &[Algorithm],
    sidecars: &[HashRef],
    known: Option<&KnownReference>,
) -> bool {
    selected.contains(&algorithm)
        || sidecars
            .iter()
            .any(|s| s.algorithm == algorithm && s.lookup != SidecarLookup::Absent)
        || known.is_some_and(|k| algorithm::find(&k.digests, algorithm).is_some())
}

/// A reference next to a file, found or wanted: a hash file of its own
/// (`file.sha256`), or its line in the folder's list (`SHA256SUMS`).
struct HashRef {
    algorithm: Algorithm,
    path: PathBuf,
    lookup: SidecarLookup,
    /// A destination asked for, not only one found.
    requested: bool,
    /// A line of the folder's list.
    list: bool,
}

fn sidecar_result(existing: &HashRef, outcome: WriteOutcome) -> SidecarResult {
    let (existing_hex, notes, problem) = match &existing.lookup {
        SidecarLookup::Absent => (None, Vec::new(), None),
        SidecarLookup::Found { digest, notes, .. } => {
            (Some(digest.hex.clone()), notes.clone(), None)
        }
        SidecarLookup::Invalid { reason, .. } => (
            None,
            Vec::new(),
            Some(Problem::new(ProblemKind::Io, reason.clone())),
        ),
    };
    SidecarResult {
        algorithm: existing.algorithm,
        path: existing.path.display().to_string(),
        requested: existing.requested,
        list: existing.list,
        outcome,
        existing: existing_hex,
        notes,
        problem,
    }
}

impl CreateReport {
    fn fail(mut self, problem: Problem) -> CreateReport {
        self.status = if problem.kind == ProblemKind::Cancelled {
            CreateStatus::Cancelled
        } else {
            CreateStatus::Failed
        };
        tracing::warn!(path = %self.path, %problem, "create reference failed");
        self.problem = Some(problem);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum VerifyStatus {
    /// Every reference found matches the content read.
    Match,
    /// Every reference found disagrees with the content read.
    Mismatch,
    NoReference,
    /// References exist, but none in an algorithm FileSeal can compute.
    NoCompatibleReference,
    /// Some references match and others do not: shown, never settled.
    Conflict,
    /// Only unusable hash files were found.
    InvalidHashFile,
    NotFound,
    ReadError,
    Locked,
    ModifiedDuringRead,
    Cancelled,
    /// Not reached: the batch was cancelled before this file.
    NotProcessed,
    /// Something else failed (database error…).
    Failed,
}

impl VerifyStatus {
    /// Value stored in `verification.outcome`.
    pub fn code(self) -> &'static str {
        match self {
            VerifyStatus::Match => "match",
            VerifyStatus::Mismatch => "mismatch",
            VerifyStatus::NoReference => "noReference",
            VerifyStatus::NoCompatibleReference => "noCompatibleReference",
            VerifyStatus::Conflict => "conflict",
            VerifyStatus::InvalidHashFile => "invalidHashFile",
            VerifyStatus::NotFound => "notFound",
            VerifyStatus::ReadError => "readError",
            VerifyStatus::Locked => "locked",
            VerifyStatus::ModifiedDuringRead => "modifiedDuringRead",
            VerifyStatus::Cancelled => "cancelled",
            VerifyStatus::NotProcessed => "notProcessed",
            VerifyStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceKind {
    Sidecar,
    /// The file's line in its folder's hash list (`SHA256SUMS`).
    HashList,
    Database,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceOutcome {
    Match,
    Mismatch,
    Invalid,
    /// Found, but not compared (the file could not be read).
    NotCompared,
}

/// The comparison with one reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceResult {
    pub source: SourceKind,
    /// Hash file path, or the location recorded in the database.
    pub path: String,
    pub expected: Vec<Digest>,
    #[ts(type = "number | null")]
    pub expected_size: Option<u64>,
    pub reference_created_at: Option<String>,
    pub outcome: SourceOutcome,
    pub detail: Option<String>,
    pub notes: Vec<SidecarNote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VerifyReport {
    pub path: String,
    pub status: VerifyStatus,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    /// What was read: kept so a "different" file can be accepted later.
    pub digests: Vec<Digest>,
    /// Last write time of the file read, to tell later whether it changed
    /// again before a new version is accepted.
    pub observed_mtime: Option<String>,
    pub sources: Vec<SourceResult>,
    pub problem: Option<Problem>,
    /// The verification could not be added to the database history.
    pub record_problem: Option<Problem>,
    /// The database item this file was compared with.
    #[ts(type = "number | null")]
    pub item_id: Option<i64>,
    /// This verification in the database history (to accept it later).
    #[ts(type = "number | null")]
    pub verification_id: Option<i64>,
    /// R2: the item was recorded at this other path (same place on the same
    /// volume, under another drive letter).
    pub location_change: Option<String>,
    /// What the database knows about this content, when its location is
    /// not recorded (R5) or when it differs from its reference (R4).
    pub finding: Option<ContentFinding>,
    /// Locations followed: the item was recorded at this other path, and
    /// now is at this one.
    pub relocated_from: Option<String>,
}

/// Verifies one file against every reference found for it.
pub fn verify_file(
    path: &Path,
    options: &VerifyOptions,
    db: Option<DbTarget>,
    control: &Control,
    progress: &mut dyn FnMut(u64, u64),
) -> VerifyReport {
    verify_against(path, options, Vec::new(), db, control, progress)
}

/// Same, with references `given` by the caller (the lines of a hash file
/// the user opened), checked alongside the ones found for the file.
pub(crate) fn verify_against(
    path: &Path,
    options: &VerifyOptions,
    given: Vec<SourceResult>,
    db: Option<DbTarget>,
    control: &Control,
    progress: &mut dyn FnMut(u64, u64),
) -> VerifyReport {
    let started_at = timestamp::now();
    let mut report = VerifyReport {
        path: path.display().to_string(),
        status: VerifyStatus::Failed,
        size: None,
        digests: Vec::new(),
        observed_mtime: None,
        sources: Vec::new(),
        problem: None,
        record_problem: None,
        item_id: None,
        verification_id: None,
        location_change: None,
        finding: None,
        relocated_from: None,
    };
    let (path, location) = match locate(path) {
        Ok(found) => found,
        Err(problem) => return report.fail(problem),
    };
    report.path = location.path.clone();
    let mut sources = given;
    if !path.exists() {
        report.sources = sources;
        return report.fail(Problem::new(
            ProblemKind::NotFound,
            format!("{} does not exist", location.path),
        ));
    }

    if options.use_sidecar {
        let lookups = Algorithm::ALL.into_iter().flat_map(|algorithm| {
            [
                (SourceKind::Sidecar, sidecar::lookup(&path, algorithm)),
                (SourceKind::HashList, sums::lookup(&path, algorithm)),
            ]
        });
        for (kind, lookup) in lookups {
            match lookup {
                SidecarLookup::Absent => {}
                SidecarLookup::Found {
                    path: file,
                    digest,
                    notes,
                } => sources.push(SourceResult {
                    source: kind,
                    path: file.display().to_string(),
                    expected: vec![digest],
                    expected_size: None,
                    reference_created_at: None,
                    outcome: SourceOutcome::NotCompared,
                    detail: None,
                    notes,
                }),
                SidecarLookup::Invalid { path: file, reason } => sources.push(SourceResult {
                    source: kind,
                    path: file.display().to_string(),
                    expected: Vec::new(),
                    expected_size: None,
                    reference_created_at: None,
                    outcome: SourceOutcome::Invalid,
                    detail: Some(reason),
                    notes: Vec::new(),
                }),
            }
        }
    }
    let mut known: Option<KnownReference> = None;
    if let Some(target) = db {
        match recorded_reference(target.store, &path, &location) {
            Ok((found, moved_from)) => {
                known = found;
                report.location_change = moved_from;
            }
            Err(problem) => return report.fail(problem),
        }
        if options.follow_moves
            && let Some(known) = known.as_mut()
        {
            report.relocated_from = follow_volume(
                &mut report.location_change,
                target,
                &path,
                &location,
                known.item_id,
            );
            if report.relocated_from.is_some() {
                known.path = location.path.clone();
            }
        }
    }
    if let Some(known) = &known {
        report.item_id = Some(known.item_id);
        sources.push(database_source(known));
    }
    // Without a status of its own in the database, a file is looked up by
    // content there: it may be a known file that moved (R5).
    let mut look_up_content = known.is_none() && db.is_some();
    let without_reference = |sources: &[SourceResult]| {
        if sources.is_empty() {
            VerifyStatus::NoReference
        } else {
            VerifyStatus::InvalidHashFile
        }
    };

    let usable = sources
        .iter()
        .filter(|s| s.outcome != SourceOutcome::Invalid)
        .count();
    if usable == 0 {
        report.status = without_reference(&sources);
        report.sources = sources;
        let Some(target) = db.filter(|_| look_up_content) else {
            return report;
        };
        // R5: no current reference of this size means an unknown content,
        // without reading a byte. Only name hints are worth showing.
        let size = match reader::file_size(&path) {
            Ok(size) => size,
            Err(problem) => return report.fail(problem),
        };
        let size_known = target.store.lock().has_reference_of_size(size);
        if !size_known.unwrap_or(true) {
            report.size = Some(size);
            if let Ok(hints) = content::name_hints(target.store, &location.file_name, None)
                && !hints.is_empty()
            {
                report.finding = Some(ContentFinding {
                    outcome: ContentOutcome::Unknown,
                    same_content: Vec::new(),
                    hints,
                });
            }
            return report;
        }
        sources = std::mem::take(&mut report.sources);
    }
    let mut needed: Vec<Algorithm> = sources
        .iter()
        .filter(|s| s.outcome != SourceOutcome::Invalid)
        .flat_map(|s| s.expected.iter().map(|d| d.algorithm))
        .collect();
    if look_up_content && let Some(target) = db {
        let in_use = target.store.lock().algorithms_in_use();
        needed.extend(in_use.unwrap_or_default());
    }
    needed.sort_unstable();
    needed.dedup();
    if needed.is_empty() {
        report.status = if usable == 0 {
            without_reference(&sources)
        } else {
            VerifyStatus::NoCompatibleReference
        };
        report.sources = sources;
        return report;
    }

    let hashed = match reader::hash_file(&path, &needed, control, progress) {
        Ok(hashed) => hashed,
        Err(problem) => {
            report.sources = sources;
            let mut report = report.fail(problem);
            // A failed read is history too (a disk that starts failing shows
            // up there), but a cancellation says nothing about the file.
            if report.status != VerifyStatus::Cancelled
                && let (Some(target), Some(known)) = (db, &known)
            {
                let detail = report.problem.as_ref().map(ToString::to_string);
                let recorded = target.store.lock().record_verification(&NewVerification {
                    operation_id: target.operation_id,
                    item_id: Some(known.item_id),
                    reference_id: Some(known.reference_id),
                    observed_path: &location.path,
                    observed_size: None,
                    observed_mtime: None,
                    started_at: &started_at,
                    outcome: report.status.code(),
                    detail: detail.as_deref(),
                    digests: &[],
                });
                match recorded {
                    Ok(id) => report.verification_id = Some(id),
                    Err(problem) => {
                        tracing::warn!(path = %report.path, %problem, "verification not recorded");
                        report.record_problem = Some(problem);
                    }
                }
            }
            return report;
        }
    };
    report.size = Some(hashed.size);
    report.digests = hashed.digests.clone();
    report.observed_mtime = hashed.modified.map(timestamp::from_system);

    // Locations followed: a known content missing from its place is that
    // item, moved here. It is attached before the comparison, which then
    // says what it would have said at its former place.
    let mut found_before: Option<ContentFinding> = None;
    if look_up_content
        && options.follow_moves
        && let Some(target) = db
    {
        match content::find(target.store, &location.file_name, None, &hashed) {
            Ok(mut finding) => {
                match follow::content(target.store, &path, &location, &hashed, &mut finding) {
                    Some((attached, from)) => {
                        report.item_id = Some(attached.item_id);
                        report.relocated_from = Some(from);
                        sources.push(database_source(&attached));
                        known = Some(attached);
                        look_up_content = false;
                    }
                    None => found_before = Some(finding),
                }
            }
            Err(problem) => tracing::warn!(%problem, "content not looked up"),
        }
    }

    for source in sources
        .iter_mut()
        .filter(|s| s.outcome != SourceOutcome::Invalid)
    {
        let same_hashes = source
            .expected
            .iter()
            .all(|expected| computed(&hashed, expected.algorithm) == Some(expected));
        let same_size = source.expected_size.is_none_or(|size| size == hashed.size);
        source.outcome = if same_hashes && same_size {
            SourceOutcome::Match
        } else {
            SourceOutcome::Mismatch
        };
        if !same_size {
            source.detail = Some(format!(
                "size changed: {} → {} bytes",
                source.expected_size.unwrap_or_default(),
                hashed.size
            ));
        }
    }
    let matches = sources.iter().any(|s| s.outcome == SourceOutcome::Match);
    let mismatches = sources.iter().any(|s| s.outcome == SourceOutcome::Mismatch);
    report.status = match (matches, mismatches) {
        (true, true) => VerifyStatus::Conflict,
        (false, true) => VerifyStatus::Mismatch,
        (true, false) => VerifyStatus::Match,
        (false, false) => without_reference(&sources),
    };
    report.sources = sources;

    // What the database knows of this content: where else it is (R5, for
    // an unknown location), or which other item it equals (R4, for a file
    // that differs from its own reference: never a match for that).
    if let Some(target) = db
        && (look_up_content
            || matches!(
                report.status,
                VerifyStatus::Mismatch | VerifyStatus::Conflict
            ))
    {
        let found = match found_before {
            Some(finding) => Ok(finding),
            None => content::find(
                target.store,
                &location.file_name,
                known.as_ref().map(|k| k.item_id),
                &hashed,
            ),
        };
        match found {
            Ok(finding) => {
                if !finding.same_content.is_empty() || !finding.hints.is_empty() {
                    report.finding = Some(finding);
                }
            }
            Err(problem) => tracing::warn!(%problem, "content not looked up"),
        }
    }

    // History, only for a file the database knows.
    if let (Some(target), Some(known)) = (db, &known) {
        let followed = report
            .relocated_from
            .as_ref()
            .map(|from| format!("location followed; previously at {from}"));
        let detail: Vec<String> = report
            .sources
            .iter()
            .find_map(|s| s.detail.clone())
            .into_iter()
            .chain(followed)
            .collect();
        let detail = (!detail.is_empty()).then(|| detail.join("; "));
        let recorded = target.store.lock().record_verification(&NewVerification {
            operation_id: target.operation_id,
            item_id: Some(known.item_id),
            reference_id: Some(known.reference_id),
            observed_path: &location.path,
            observed_size: Some(hashed.size),
            observed_mtime: report.observed_mtime.clone(),
            started_at: &started_at,
            outcome: report.status.code(),
            detail: detail.as_deref(),
            digests: &hashed.digests,
        });
        match recorded {
            Ok(id) => {
                report.verification_id = Some(id);
                // Seen at its recorded location (not at a new drive letter:
                // updating that is the user's decision).
                if report.location_change.is_none() {
                    touch(target.store, known.item_id, &hashed);
                }
            }
            Err(problem) => {
                tracing::warn!(path = %report.path, %problem, "verification not recorded");
                report.record_problem = Some(problem);
            }
        }
    }
    tracing::info!(path = %report.path, status = ?report.status, "verify");
    report
}

impl VerifyReport {
    fn fail(mut self, problem: Problem) -> VerifyReport {
        self.status = match problem.kind {
            ProblemKind::Cancelled => VerifyStatus::Cancelled,
            ProblemKind::NotFound => VerifyStatus::NotFound,
            ProblemKind::Locked => VerifyStatus::Locked,
            ProblemKind::ModifiedDuringRead => VerifyStatus::ModifiedDuringRead,
            ProblemKind::PermissionDenied
            | ProblemKind::Io
            | ProblemKind::NotAFile
            | ProblemKind::DeviceUnavailable
            | ProblemKind::NetworkError
            | ProblemKind::MediaError => VerifyStatus::ReadError,
            _ => VerifyStatus::Failed,
        };
        tracing::warn!(path = %self.path, %problem, "verification failed");
        self.problem = Some(problem);
        self
    }
}

/// The database reference of a file, to be compared with what is read.
fn database_source(known: &KnownReference) -> SourceResult {
    SourceResult {
        source: SourceKind::Database,
        path: known.path.clone(),
        expected: known.digests.clone(),
        expected_size: Some(known.size),
        reference_created_at: Some(known.created_at.clone()),
        outcome: SourceOutcome::NotCompared,
        detail: None,
        notes: Vec::new(),
    }
}

fn computed(hashed: &HashedFile, algorithm: Algorithm) -> Option<&Digest> {
    algorithm::find(&hashed.digests, algorithm)
}

/// Notes that the item was just seen at its location, with its size and
/// date. Only a hint for later: a failure is logged, not reported.
fn touch(store: &SharedStore, item_id: i64, hashed: &HashedFile) {
    let mtime = hashed.modified.map(timestamp::from_system);
    let touched = store
        .lock()
        .touch_location(item_id, hashed.size, mtime.as_deref());
    if let Err(problem) = touched {
        tracing::debug!(%problem, "location not updated");
    }
}

#[cfg(test)]
mod follow_tests;
#[cfg(test)]
mod rules_tests;
#[cfg(test)]
mod sums_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod unread_tests;

/// Absolute path of the file and the location record describing it.
pub(crate) fn locate(path: &Path) -> Result<(PathBuf, NewLocation), Problem> {
    let absolute = platform::absolute(path).map_err(|err| Problem::from_io(&err))?;
    let text = platform::path_text(&absolute)?;
    let file_name = absolute
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let location = NewLocation {
        path_key: platform::path_key(&text),
        path: text,
        file_name,
        last_size: None,
        last_mtime: None,
        volume_serial: None,
        volume_relative_key: None,
    };
    Ok((absolute, location))
}
