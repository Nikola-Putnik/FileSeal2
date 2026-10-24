//! Timestamps are stored as UTC text (`2026-10-22T09:15:03.250Z`): readable
//! in any SQLite browser, sortable as text, and turned into local time only
//! when displayed.

use std::time::SystemTime;

use time::OffsetDateTime;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;

const FORMAT: &[BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub(crate) fn now() -> String {
    format(OffsetDateTime::now_utc())
}

pub(crate) fn from_system(time: SystemTime) -> String {
    format(OffsetDateTime::from(time))
}

fn format(time: OffsetDateTime) -> String {
    time.to_offset(time::UtcOffset::UTC)
        .format(FORMAT)
        .expect("a UTC date always formats")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_in_utc_with_milliseconds() {
        let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_761_124_503_250);
        assert_eq!(from_system(time), "2025-10-22T09:15:03.250Z");
        assert_eq!(now().len(), "2025-10-22T09:15:03.250Z".len());
    }
}
