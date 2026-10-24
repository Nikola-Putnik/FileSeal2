//! What went wrong, in a form the UI can explain: a stable `kind` that the
//! interface turns into a sentence, plus the technical detail (often the
//! system message, already in the user's language) for the curious and for
//! the logs.

use std::io;

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProblemKind {
    NotFound,
    PermissionDenied,
    /// Another program has the file open for writing, or holds a lock.
    Locked,
    /// Size, date or identity changed while the file was being read.
    ModifiedDuringRead,
    Cancelled,
    DiskFull,
    AlreadyExists,
    /// The name cannot be written as UTF-8 (ill-formed UTF-16 on Windows).
    NameNotRepresentable,
    NameTooLong,
    /// Forbidden characters or syntax in a name or path.
    InvalidName,
    /// The path is a folder where a file was expected.
    NotAFile,
    /// The drive is gone or not ready (unplugged, switched off, ejected).
    DeviceUnavailable,
    /// The network share or connection was lost.
    NetworkError,
    /// The storage could not read the data (bad sector, CRC error): for an
    /// integrity tool, a warning sign about the medium itself.
    MediaError,
    DatabaseInUse,
    DatabaseReadOnly,
    DatabaseTooNew,
    NotAFileSealDatabase,
    /// `PRAGMA quick_check` found damage: restore a backup copy.
    DatabaseCorrupt,
    /// The database file can no longer be read or written while in use
    /// (disk unplugged, share lost, failing medium).
    DatabaseUnavailable,
    /// Another item is already recorded at this location.
    LocationTaken,
    /// The reference changed since the verification being accepted, or the
    /// file changed since it was verified: verify again first.
    StaleVerification,
    /// The content does not match the item it was to be attached to.
    ContentDiffers,
    /// A hash file that cannot be used: unreadable, empty, no valid line.
    InvalidHashFile,
    Database,
    /// A job is already running.
    Busy,
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Problem {
    pub kind: ProblemKind,
    pub detail: String,
}

impl Problem {
    pub fn new(kind: ProblemKind, detail: impl Into<String>) -> Problem {
        Problem {
            kind,
            detail: detail.into(),
        }
    }

    pub fn cancelled() -> Problem {
        Problem::new(ProblemKind::Cancelled, "cancelled by the user")
    }

    /// Classifies an I/O error. Windows codes are checked first because
    /// several of them share `ErrorKind::Other`.
    pub fn from_io(err: &io::Error) -> Problem {
        Problem::new(classify(err), err.to_string())
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for Problem {}

fn classify(err: &io::Error) -> ProblemKind {
    if cfg!(windows) {
        match err.raw_os_error() {
            // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION
            Some(32 | 33) => return ProblemKind::Locked,
            // ERROR_HANDLE_DISK_FULL, ERROR_DISK_FULL
            Some(39 | 112) => return ProblemKind::DiskFull,
            // ERROR_FILENAME_EXCED_RANGE
            Some(206) => return ProblemKind::NameTooLong,
            // ERROR_INVALID_NAME
            Some(123) => return ProblemKind::InvalidName,
            // ERROR_NOT_READY, ERROR_DEV_NOT_EXIST, ERROR_DEVICE_NOT_CONNECTED,
            // ERROR_NO_SUCH_DEVICE
            Some(21 | 55 | 1167 | 433) => return ProblemKind::DeviceUnavailable,
            // ERROR_BAD_NETPATH, ERROR_UNEXP_NET_ERR, ERROR_NETNAME_DELETED,
            // ERROR_BAD_NET_NAME, ERROR_SEM_TIMEOUT, ERROR_VC_DISCONNECTED,
            // ERROR_NETWORK_UNREACHABLE, ERROR_CONNECTION_ABORTED
            Some(53 | 59 | 64 | 67 | 121 | 240 | 1231 | 1236) => {
                return ProblemKind::NetworkError;
            }
            // ERROR_CRC, ERROR_SECTOR_NOT_FOUND, ERROR_READ_FAULT,
            // ERROR_DEVICE_HARDWARE_ERROR, ERROR_IO_DEVICE
            Some(23 | 27 | 30 | 483 | 1117) => return ProblemKind::MediaError,
            _ => {}
        }
    }
    match err.kind() {
        io::ErrorKind::NotFound => ProblemKind::NotFound,
        io::ErrorKind::PermissionDenied => ProblemKind::PermissionDenied,
        io::ErrorKind::AlreadyExists => ProblemKind::AlreadyExists,
        io::ErrorKind::StorageFull => ProblemKind::DiskFull,
        io::ErrorKind::IsADirectory => ProblemKind::NotAFile,
        // Covers both "too long" and "forbidden characters": only the
        // Windows codes above can tell them apart.
        io::ErrorKind::InvalidFilename => ProblemKind::InvalidName,
        _ => ProblemKind::Io,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_codes_are_classified() {
        let kind = |code| Problem::from_io(&io::Error::from_raw_os_error(code)).kind;
        assert_eq!(kind(32), ProblemKind::Locked);
        assert_eq!(kind(112), ProblemKind::DiskFull);
        assert_eq!(kind(123), ProblemKind::InvalidName);
        assert_eq!(kind(206), ProblemKind::NameTooLong);
        assert_eq!(kind(2), ProblemKind::NotFound);
        assert_eq!(kind(5), ProblemKind::PermissionDenied);
    }
}
