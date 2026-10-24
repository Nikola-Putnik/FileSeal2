//! Looking a content up in the database: which items have exactly this
//! content (R4, R5), and which only share its name (R7). The decision
//! rules themselves are in `matching`.

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::algorithm;
use crate::matching::{self, Candidate, ContentOutcome, Presence};
use crate::platform;
use crate::problem::Problem;
use crate::reader::HashedFile;
use crate::store::{KnownReference, SharedStore};

/// What the database knows about a content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContentFinding {
    pub outcome: ContentOutcome,
    /// Items whose current reference is exactly this content.
    pub same_content: Vec<Candidate>,
    /// Items with the same file name but another content: hints only.
    pub hints: Vec<Candidate>,
}

/// True if the content read is the content of this reference: same size,
/// and the same digest for every algorithm both sides have (at least one).
pub(crate) fn agrees(known: &KnownReference, hashed: &HashedFile) -> bool {
    known.size == hashed.size
        && known
            .digests
            .iter()
            .any(|d| algorithm::find(&hashed.digests, d.algorithm).is_some())
        && known.digests.iter().all(|d| {
            algorithm::find(&hashed.digests, d.algorithm).is_none_or(|computed| computed == d)
        })
}

/// Where a recorded location stands now.
pub(crate) fn presence(path: &str) -> Presence {
    let path = Path::new(path);
    if path.exists() {
        Presence::Present
    } else if platform::volume_available(path) {
        Presence::Missing
    } else {
        Presence::VolumeUnavailable
    }
}

fn candidate(known: &KnownReference) -> Candidate {
    Candidate {
        item_id: known.item_id,
        path: known.path.clone(),
        presence: presence(&known.path),
        size: known.size,
    }
}

/// Items with exactly the content read, then the outcome for a file whose
/// own location is recorded for `here` (if any).
pub(crate) fn find(
    store: &SharedStore,
    file_name: &str,
    here: Option<i64>,
    hashed: &HashedFile,
) -> Result<ContentFinding, Problem> {
    // Every empty file has the same content: one being a copy of, or moved
    // from, another empty file would mean nothing.
    if hashed.size == 0 {
        return Ok(ContentFinding {
            outcome: ContentOutcome::Unknown,
            same_content: Vec::new(),
            hints: Vec::new(),
        });
    }
    let mut same: Vec<KnownReference> = Vec::new();
    {
        let store = store.lock();
        // The file's own item first: the lookups below stop at 20 items,
        // which may not include it when many share this content.
        if let Some(here) = here
            && let Some(known) = store.current_of_item(here)?
            && agrees(&known, hashed)
        {
            same.push(known);
        }
        for digest in &hashed.digests {
            for known in store.current_by_digest(digest)? {
                if agrees(&known, hashed) && !same.iter().any(|s| s.item_id == known.item_id) {
                    same.push(known);
                }
            }
        }
    }
    // Unlocked: telling whether a copy is present may wait on a slow or
    // unplugged drive.
    let same_content: Vec<Candidate> = same.iter().map(candidate).collect();
    let outcome = matching::classify(here, &same_content);
    let hints = if outcome == ContentOutcome::Unknown {
        name_hints(store, file_name, here)?
    } else {
        Vec::new()
    };
    Ok(ContentFinding {
        outcome,
        same_content,
        hints,
    })
}

/// R7 alone, for a file not even read (no reference has its size).
pub(crate) fn name_hints(
    store: &SharedStore,
    file_name: &str,
    here: Option<i64>,
) -> Result<Vec<Candidate>, Problem> {
    let known = store.lock().current_by_name(file_name)?;
    let by_name: Vec<Candidate> = known
        .iter()
        .filter(|known| Some(known.item_id) != here)
        .map(candidate)
        .collect();
    Ok(matching::hints(file_name, &by_name)
        .into_iter()
        .cloned()
        .collect())
}
