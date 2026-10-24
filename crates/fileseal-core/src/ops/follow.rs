//! Locations followed automatically (`follow_moves`), for whoever only
//! wants references created and verified, wherever the files are: a file
//! found elsewhere than recorded is attached to its item without asking,
//! when its content or its place on its disk says which item it is, and the
//! database keeps its last known location. The former ones stay in the
//! item's history.
//!
//! The choice made beforehand in the settings stands for the confirmation
//! of R8. The rules that find the item (R2, R5) are unchanged, and a copy
//! (its item still where it was, or on a disk that is not plugged in) is
//! never attached.

use std::path::Path;

use super::content::{self, ContentFinding};
use super::observed_location;
use crate::matching;
use crate::platform;
use crate::problem::Problem;
use crate::reader::HashedFile;
use crate::store::{KnownReference, NewLocation, SharedStore};

/// R2 followed: the item recorded at the same place of the same volume,
/// under another drive letter, is now at `path`.
pub(crate) fn volume(
    store: &SharedStore,
    path: &Path,
    location: &NewLocation,
    item_id: i64,
) -> Result<(), Problem> {
    // Opened for its size, date and identity; its content is not read.
    let state = platform::identify(path).map_err(|err| Problem::from_io(&err))?;
    let seen = HashedFile {
        size: state.len,
        modified: state.modified,
        id: state.id,
        digests: Vec::new(),
    };
    store
        .lock()
        .relocate(item_id, &observed_location(path, location, &seen))
}

/// R5 followed: the known item with exactly the content read, missing from
/// its recorded location, is now at `path`. Gives its current reference and
/// its former location; `None` if no item is missing (an unknown content,
/// or a copy), or if the move could not be recorded (logged: the user then
/// decides, as without the option). `finding` is looked up again when
/// another file of the batch was attached to the chosen item meanwhile.
pub(crate) fn content(
    store: &SharedStore,
    path: &Path,
    location: &NewLocation,
    hashed: &HashedFile,
    finding: &mut ContentFinding,
) -> Option<(KnownReference, String)> {
    // Each try fails only if another file took the item chosen.
    for _ in 0..3 {
        let chosen = matching::automatic_choice(&location.file_name, &finding.same_content)?;
        let (item_id, from) = (chosen.item_id, chosen.path.clone());
        let observed = observed_location(path, location, hashed);
        let moved = store
            .lock()
            .relocate_from(item_id, &platform::path_key(&from), &observed);
        match moved {
            Ok(true) => {
                let current = store.lock().current_of_item(item_id);
                return match current {
                    Ok(Some(known)) => {
                        tracing::info!(item_id, path = %location.path, %from, "location followed");
                        Some((known, from))
                    }
                    Ok(None) => None,
                    Err(problem) => {
                        tracing::warn!(%problem, "followed item not read back");
                        None
                    }
                };
            }
            Ok(false) => match content::find(store, &location.file_name, None, hashed) {
                Ok(found) => *finding = found,
                Err(problem) => {
                    tracing::warn!(%problem, "content not looked up again");
                    return None;
                }
            },
            Err(problem) => {
                tracing::warn!(%problem, path = %location.path, "location not followed");
                return None;
            }
        }
    }
    None
}
