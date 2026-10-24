//! Matching rules: what a file found somewhere means for the database.
//! Pure functions (no file system, no database), so each rule of
//! docs/rapprochement.md is tested on its own.
//!
//! The principles:
//! - a path never proves identity, a hash does (R1, R2 only find the
//!   expected reference; the comparison decides);
//! - the same content in another item never makes a file "matching" a
//!   different expected reference (R4);
//! - a move is only "probable" when exactly one known copy of this content
//!   is missing from a volume that is there; a copy on a disk that is not
//!   plugged in is not missing, just out of reach (R5);
//! - a name or a size is a hint, never an association (R6, R7).

use serde::Serialize;
use ts_rs::TS;

/// Where a known item stands now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Presence {
    /// Its file is at its recorded location.
    Present,
    /// Its volume is there but its file is not: moved, renamed or deleted.
    Missing,
    /// Its volume cannot be reached (disk unplugged, share offline).
    VolumeUnavailable,
}

/// A known item that may be the file in hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Candidate {
    #[ts(type = "number")]
    pub item_id: i64,
    /// Its current recorded location.
    pub path: String,
    pub presence: Presence,
    #[ts(type = "number")]
    pub size: u64,
}

/// What the content of a file means for the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ContentOutcome {
    /// No current reference has this content.
    Unknown,
    /// The item recorded at this very location has this content.
    KnownHere,
    /// Exactly one item with this content is missing from its location:
    /// most likely this file, moved or renamed. To be confirmed.
    ProbablyMoved,
    /// Items with this content are where they should be, or out of reach:
    /// this file is another copy.
    CopyOfKnown,
    /// Several items with this content are missing: which one moved here
    /// cannot be told. The user chooses.
    Ambiguous,
}

/// R5: classifies the items whose current reference has the same content.
/// `here` is the item recorded at the file's own location, if any.
pub fn classify(here: Option<i64>, same_content: &[Candidate]) -> ContentOutcome {
    if here.is_some_and(|here| same_content.iter().any(|c| c.item_id == here)) {
        return ContentOutcome::KnownHere;
    }
    let missing = same_content
        .iter()
        .filter(|c| c.presence == Presence::Missing)
        .count();
    match missing {
        1 => ContentOutcome::ProbablyMoved,
        n if n > 1 => ContentOutcome::Ambiguous,
        _ if same_content.is_empty() => ContentOutcome::Unknown,
        _ => ContentOutcome::CopyOfKnown,
    }
}

/// The candidate a move would be attached to, when there is one obvious.
pub fn moved_from(same_content: &[Candidate]) -> Option<&Candidate> {
    let mut missing = same_content
        .iter()
        .filter(|c| c.presence == Presence::Missing);
    match (missing.next(), missing.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// R6, R7: items worth showing as hints for a file whose content is not
/// known: same name (whatever the folder), but a different content. Never
/// associated automatically; the user may attach one by hand.
pub fn hints<'a>(file_name: &str, by_name: &'a [Candidate]) -> Vec<&'a Candidate> {
    let wanted = file_name.to_lowercase();
    by_name
        .iter()
        .filter(|c| {
            c.path
                .rsplit(['\\', '/'])
                .next()
                .is_some_and(|name| name.to_lowercase() == wanted)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(item_id: i64, presence: Presence) -> Candidate {
        Candidate {
            item_id,
            path: format!(r"E:\Films\{item_id}.mkv"),
            presence,
            size: 10,
        }
    }

    #[test]
    fn r5_content_not_in_the_database() {
        assert_eq!(classify(None, &[]), ContentOutcome::Unknown);
        assert_eq!(classify(Some(3), &[]), ContentOutcome::Unknown);
    }

    #[test]
    fn r1_the_item_at_this_location_has_this_content() {
        let same = [
            candidate(1, Presence::Present),
            candidate(2, Presence::Missing),
        ];
        assert_eq!(classify(Some(1), &same), ContentOutcome::KnownHere);
    }

    #[test]
    fn r5_one_missing_copy_is_a_probable_move() {
        let same = [candidate(1, Presence::Missing)];
        assert_eq!(classify(None, &same), ContentOutcome::ProbablyMoved);
        assert_eq!(moved_from(&same).unwrap().item_id, 1);
        // Still a move when other copies exist where they should be.
        let same = [
            candidate(1, Presence::Present),
            candidate(2, Presence::Missing),
        ];
        assert_eq!(classify(None, &same), ContentOutcome::ProbablyMoved);
        assert_eq!(moved_from(&same).unwrap().item_id, 2);
    }

    #[test]
    fn r5_present_copies_make_another_copy() {
        let same = [
            candidate(1, Presence::Present),
            candidate(2, Presence::Present),
        ];
        assert_eq!(classify(None, &same), ContentOutcome::CopyOfKnown);
        assert!(moved_from(&same).is_none());
    }

    #[test]
    fn r5_a_copy_on_an_unplugged_disk_is_not_a_move() {
        // The backup disk is not connected: its copy is not "missing".
        let same = [candidate(1, Presence::VolumeUnavailable)];
        assert_eq!(classify(None, &same), ContentOutcome::CopyOfKnown);
        assert!(moved_from(&same).is_none());
    }

    #[test]
    fn r5_several_missing_copies_are_ambiguous() {
        let same = [
            candidate(1, Presence::Missing),
            candidate(2, Presence::Missing),
        ];
        assert_eq!(classify(None, &same), ContentOutcome::Ambiguous);
        assert!(moved_from(&same).is_none(), "the user chooses");
    }

    #[test]
    fn r6_r7_same_name_is_only_a_hint() {
        let known = [
            Candidate {
                path: r"E:\Films\Blade Runner.mkv".to_owned(),
                ..candidate(1, Presence::Missing)
            },
            Candidate {
                path: r"E:\Films\Alien.mkv".to_owned(),
                ..candidate(2, Presence::Missing)
            },
        ];
        let found = hints("blade runner.MKV", &known);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].item_id, 1);
        // A hint does not change the outcome: the content is unknown.
        assert_eq!(classify(None, &[]), ContentOutcome::Unknown);
    }
}
