//! Everything that depends on the operating system. Porting to macOS or
//! Linux means reviewing this module, not the rest of the crate.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::problem::{Problem, ProblemKind};

/// Identity of a file on its volume: survives renames and moves within the
/// volume, differs between two copies. Hard links share one identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId {
    pub volume: u64,
    pub file: u128,
}

/// What can be observed about an open file without reading it. Compared
/// before and after a read to detect a change made in between.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileState {
    pub len: u64,
    /// Last write time: tools can set it back, so it is not enough alone.
    pub modified: Option<SystemTime>,
    /// Windows ChangeTime / Unix ctime: updated by any write or metadata
    /// change, and not settable by ordinary programs.
    pub changed: Option<i128>,
    /// `None` when the file system gives no usable identifier (some NAS).
    pub id: Option<FileId>,
}

/// Reads the state through the open handle, not the path: a directory
/// listing (especially over SMB, which caches it) may be stale.
pub(crate) fn file_state(file: &File) -> io::Result<FileState> {
    let metadata = file.metadata()?;
    Ok(FileState {
        len: metadata.len(),
        modified: metadata.modified().ok(),
        changed: os::change_time(file, &metadata),
        id: os::file_id(file, &metadata),
    })
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod os {
    use std::fs::{File, Metadata};
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_BASIC_INFO, FILE_ID_INFO, FileBasicInfo, FileIdInfo,
        GetFileInformationByHandle, GetFileInformationByHandleEx,
    };

    use super::FileId;

    pub(super) fn change_time(file: &File, _metadata: &Metadata) -> Option<i128> {
        // SAFETY: FILE_BASIC_INFO is plain data (all-zero is valid), the
        // buffer size matches the information class, and the handle stays
        // valid while `file` is borrowed.
        let mut info: FILE_BASIC_INFO = unsafe { zeroed() };
        let ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileBasicInfo,
                (&raw mut info).cast(),
                size_of::<FILE_BASIC_INFO>() as u32,
            )
        };
        (ok != 0).then_some(i128::from(info.ChangeTime))
    }

    pub(super) fn file_id(file: &File, _metadata: &Metadata) -> Option<FileId> {
        // 128-bit identifier first: required on ReFS, where the 64-bit index
        // of the older call is not unique.
        // SAFETY: as above, for FILE_ID_INFO.
        let mut info: FILE_ID_INFO = unsafe { zeroed() };
        let ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileIdInfo,
                (&raw mut info).cast(),
                size_of::<FILE_ID_INFO>() as u32,
            )
        };
        if ok != 0 && info.FileId.Identifier != [0; 16] {
            return Some(FileId {
                volume: info.VolumeSerialNumber,
                file: u128::from_le_bytes(info.FileId.Identifier),
            });
        }
        // SAFETY: as above, for BY_HANDLE_FILE_INFORMATION.
        let mut legacy: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
        let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut legacy) };
        let index = (u64::from(legacy.nFileIndexHigh) << 32) | u64::from(legacy.nFileIndexLow);
        // Some network file systems answer 0: that identifies nothing.
        (ok != 0 && index != 0).then_some(FileId {
            volume: u64::from(legacy.dwVolumeSerialNumber),
            file: u128::from(index),
        })
    }

    fn wide(path: &std::path::Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().chain([0]).collect()
    }

    /// A drive letter mapped to a network share.
    pub(super) fn is_remote_drive(path: &std::path::Path) -> bool {
        use std::path::{Component, Prefix};
        let Some(Component::Prefix(prefix)) = path.components().next() else {
            return false;
        };
        let letter = match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return false,
        };
        let root = format!("{}:\\", char::from(letter));
        let root = wide(std::path::Path::new(&root));
        const DRIVE_REMOTE: u32 = 4;
        // SAFETY: `root` is a NUL-terminated UTF-16 string that outlives the call.
        unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE
        }
    }

    pub(super) fn available_space(dir: &std::path::Path) -> Option<u64> {
        let dir = wide(&super::extended(dir));
        let mut free = 0u64;
        // SAFETY: `dir` is NUL-terminated and outlives the call; the two
        // unused outputs may be null; `free` is a valid u64 to write to.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                dir.as_ptr(),
                &raw mut free,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        (ok != 0).then_some(free)
    }

    /// The mount point of the volume holding `path` (`E:\`, `\\nas\share\`,
    /// or a folder where a volume is mounted).
    pub(super) fn volume_root(path: &std::path::Path) -> Option<std::path::PathBuf> {
        use std::os::windows::ffi::OsStringExt;
        let path = wide(&super::extended(path));
        let mut buffer = vec![0u16; 32_768];
        // SAFETY: `path` is NUL-terminated; `buffer` is writable for the
        // length given, which the function never exceeds.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetVolumePathNameW(
                path.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        };
        if ok == 0 {
            return None;
        }
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        let root = std::ffi::OsString::from_wide(&buffer[..len]);
        let root = super::strip_verbatim(std::path::Path::new(&root));
        // On a `subst` drive, Windows answers a folder of it (`W:\Photos\`
        // for `W:\Photos\a.jpg`). A root below the drive's own is only
        // believed if it is a real mount point, which is a reparse point.
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        let below_drive = root.components().count() > 2;
        let mount_point = std::fs::symlink_metadata(super::extended(&root))
            .is_ok_and(|m| attributes(&m) & FILE_ATTRIBUTE_REPARSE_POINT != 0);
        (!below_drive || mount_point).then_some(root)
    }

    pub(super) fn attributes(metadata: &Metadata) -> u32 {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes()
    }

    pub(super) fn rename_noclobber(
        from: &std::path::Path,
        to: &std::path::Path,
    ) -> std::io::Result<()> {
        let from = wide(&super::extended(from));
        let to = wide(&super::extended(to));
        // SAFETY: both strings are NUL-terminated and outlive the call. No
        // flag: the call fails if `to` exists, or is on another volume.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(from.as_ptr(), to.as_ptr(), 0)
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn is_other_volume(err: &std::io::Error) -> bool {
        // ERROR_NOT_SAME_DEVICE
        err.raw_os_error() == Some(17)
    }

    pub(super) fn drive_roots() -> Vec<std::path::PathBuf> {
        // SAFETY: no argument; returns a bit mask of the drive letters.
        let mask = unsafe { windows_sys::Win32::Storage::FileSystem::GetLogicalDrives() };
        (0..26u8)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| std::path::PathBuf::from(format!("{}:\\", char::from(b'A' + bit))))
            .collect()
    }

    /// A handle on a folder, enough to read its identity.
    pub(super) fn open_folder(path: &std::path::Path) -> std::io::Result<File> {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_READ_ATTRIBUTES: u32 = 0x80;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        std::fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(super::extended(path))
    }

    pub(super) const SYSTEM_REQUIRED: u32 = 0x0000_0001;

    /// Asks Windows to stay awake (or no longer), for the calling thread;
    /// returns the previous request.
    pub(super) fn keep_awake(on: bool) -> u32 {
        use windows_sys::Win32::System::Power::{ES_CONTINUOUS, SetThreadExecutionState};
        let flags = if on {
            ES_CONTINUOUS | SYSTEM_REQUIRED
        } else {
            ES_CONTINUOUS
        };
        // SAFETY: plain flags in and out, no memory involved.
        unsafe { SetThreadExecutionState(flags) }
    }

    /// `\\server\share` behind a drive letter mapped to a network share.
    pub(super) fn network_share_of(letter: char) -> Option<String> {
        use windows_sys::Win32::NetworkManagement::WNet::WNetGetConnectionW;
        let local: Vec<u16> = format!("{letter}:").encode_utf16().chain([0]).collect();
        let mut buffer = vec![0u16; 1024];
        let mut len = buffer.len() as u32;
        // SAFETY: `local` is NUL-terminated; `buffer` holds `len` u16s and
        // the function writes at most that many, updating nothing else.
        let status =
            unsafe { WNetGetConnectionW(local.as_ptr(), buffer.as_mut_ptr(), &raw mut len) };
        if status != 0 {
            return None;
        }
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16(&buffer[..end]).ok()
    }
}

#[cfg(unix)]
mod os {
    use std::fs::{File, Metadata};
    use std::os::unix::fs::MetadataExt;

    use super::FileId;

    // To do when porting: statvfs for the free space, the mount table for
    // network file systems (nfs, cifs, smbfs…) and for mount points.
    pub(super) fn is_remote_drive(_path: &std::path::Path) -> bool {
        false
    }

    pub(super) fn volume_root(_path: &std::path::Path) -> Option<std::path::PathBuf> {
        None
    }

    pub(super) fn attributes(_metadata: &Metadata) -> u32 {
        0
    }

    pub(super) fn network_share_of(_letter: char) -> Option<String> {
        None
    }

    pub(super) fn keep_awake(_on: bool) -> u32 {
        0
    }

    pub(super) fn rename_noclobber(
        from: &std::path::Path,
        to: &std::path::Path,
    ) -> std::io::Result<()> {
        // A link fails if `to` exists; then the old name goes.
        std::fs::hard_link(from, to)?;
        std::fs::remove_file(from)
    }

    pub(super) fn is_other_volume(err: &std::io::Error) -> bool {
        // EXDEV
        err.raw_os_error() == Some(18)
    }

    // To do when porting: the mount table.
    pub(super) fn drive_roots() -> Vec<std::path::PathBuf> {
        Vec::new()
    }

    pub(super) fn open_folder(path: &std::path::Path) -> std::io::Result<File> {
        File::open(path)
    }

    pub(super) fn available_space(_dir: &std::path::Path) -> Option<u64> {
        None
    }

    pub(super) fn change_time(_file: &File, metadata: &Metadata) -> Option<i128> {
        Some(i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec()))
    }

    pub(super) fn file_id(_file: &File, metadata: &Metadata) -> Option<FileId> {
        (metadata.ino() != 0).then_some(FileId {
            volume: metadata.dev(),
            file: u128::from(metadata.ino()),
        })
    }
}

#[cfg(not(any(windows, unix)))]
mod os {
    use std::fs::{File, Metadata};

    use super::FileId;

    pub(super) fn is_remote_drive(_path: &std::path::Path) -> bool {
        false
    }

    pub(super) fn volume_root(_path: &std::path::Path) -> Option<std::path::PathBuf> {
        None
    }

    pub(super) fn attributes(_metadata: &Metadata) -> u32 {
        0
    }

    pub(super) fn network_share_of(_letter: char) -> Option<String> {
        None
    }

    pub(super) fn keep_awake(_on: bool) -> u32 {
        0
    }

    pub(super) fn rename_noclobber(
        _from: &std::path::Path,
        _to: &std::path::Path,
    ) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "not available on this system",
        ))
    }

    pub(super) fn is_other_volume(_err: &std::io::Error) -> bool {
        false
    }

    pub(super) fn drive_roots() -> Vec<std::path::PathBuf> {
        Vec::new()
    }

    pub(super) fn open_folder(path: &std::path::Path) -> std::io::Result<File> {
        File::open(path)
    }

    pub(super) fn available_space(_dir: &std::path::Path) -> Option<u64> {
        None
    }

    pub(super) fn change_time(_file: &File, _metadata: &Metadata) -> Option<i128> {
        None
    }

    pub(super) fn file_id(_file: &File, _metadata: &Metadata) -> Option<FileId> {
        None
    }
}

/// Opens a source file for hashing: read-only, and on Windows other
/// programs may read it but not write, rename or delete it until we are
/// done. A file someone is writing to fails with a sharing violation
/// (reported as "locked") instead of being hashed half-written.
pub(crate) fn open_for_hashing(path: &Path) -> io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_SEQUENTIAL_SCAN);
    }
    // Extended form: a file named `nul.mkv` or ending with a dot must be
    // that file, not the NUL device or another name Windows would make of it.
    options.open(extended(path))
}

/// Absolute, normalised path (`..` resolved) without the `\\?\` prefix
/// Windows APIs sometimes add. Symbolic links are not resolved: the path
/// stays the one the user chose.
pub fn absolute(path: &Path) -> io::Result<PathBuf> {
    use std::path::Component;
    // Already absolute: only `.` and repeated separators are tidied. Windows
    // would also drop a trailing dot or space from a name (`name.` becomes
    // `name`), which then designates another file.
    if path.is_absolute() && !path.components().any(|c| c == Component::ParentDir) {
        return Ok(strip_verbatim(&path.components().collect::<PathBuf>()));
    }
    let absolute = std::path::absolute(path)?;
    Ok(strip_verbatim(&absolute))
}

/// True for a path on a network share: a UNC path (`\\nas\films`) or a
/// drive letter mapped to a share. Used to warn about databases kept there.
pub fn is_network_path(path: &Path) -> bool {
    let path = strip_verbatim(path);
    let unc = path.to_str().is_some_and(|text| text.starts_with(r"\\"));
    (cfg!(windows) && unc) || os::is_remote_drive(&path)
}

/// Free space for the user on the volume holding `dir`, if the system says.
pub fn available_space(dir: &Path) -> Option<u64> {
    os::available_space(dir)
}

/// The root of the volume holding `path`: `E:\`, `\\nas\share\`, or the
/// folder a volume is mounted on. Falls back to the path's own root.
pub fn volume_root(path: &Path) -> Option<PathBuf> {
    os::volume_root(path).or_else(|| {
        let mut root = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    root.push(component);
                }
                _ => break,
            }
        }
        (!root.as_os_str().is_empty()).then_some(root)
    })
}

/// Whether the volume holding `path` can be reached now: a file there that
/// cannot be found is then really gone, not on a disk left unplugged.
pub fn volume_available(path: &Path) -> bool {
    volume_root_of_file(path).is_some_and(|root| root.exists())
}

/// How long the root of a folder is remembered.
const ROOTS_KEPT: std::time::Duration = std::time::Duration::from_secs(30);

type RootCache = std::collections::HashMap<String, (Option<PathBuf>, std::time::Instant)>;

static ROOTS: std::sync::LazyLock<std::sync::Mutex<RootCache>> =
    std::sync::LazyLock::new(Default::default);

/// The volume root of a file, which is its folder's: asked once per folder
/// and remembered a little while. Windows takes about half a millisecond to
/// answer for a deep folder (it looks for mount points all the way up), far
/// more than the rest of what a small file costs.
pub fn volume_root_of_file(path: &Path) -> Option<PathBuf> {
    let Some(folder) = path.parent().filter(|f| !f.as_os_str().is_empty()) else {
        return volume_root(path);
    };
    let key = path_key(&folder.to_string_lossy());
    let now = std::time::Instant::now();
    let mut roots = ROOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((root, at)) = roots.get(&key)
        && now.duration_since(*at) < ROOTS_KEPT
    {
        return root.clone();
    }
    let root = volume_root(folder);
    if roots.len() > 10_000 {
        roots.clear();
    }
    roots.insert(key, (root.clone(), now));
    root
}

/// What a directory listing says about a file or folder, without opening
/// it. Windows attributes only: elsewhere every flag is false (hidden files
/// are recognised by their leading dot by the caller).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Attributes(u32);

impl Attributes {
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x0004_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

    pub fn of(metadata: &std::fs::Metadata) -> Attributes {
        Attributes(os::attributes(metadata))
    }

    #[cfg(test)]
    pub fn from_bits(bits: u32) -> Attributes {
        Attributes(bits)
    }

    pub fn hidden(self) -> bool {
        self.0 & Self::HIDDEN != 0
    }

    pub fn system(self) -> bool {
        self.0 & Self::SYSTEM != 0
    }

    /// A cloud placeholder (OneDrive "online only"…): reading it would
    /// download it, or fail without a connection.
    pub fn online_only(self) -> bool {
        self.0 & (Self::OFFLINE | Self::RECALL_ON_OPEN | Self::RECALL_ON_DATA_ACCESS) != 0
    }
}

/// The path key, with a drive letter mapped to a network share replaced by
/// the share: `Z:\x` and `\\nas\share\x` are then the same file.
pub(crate) fn canonical_key(path_text: &str) -> String {
    let mut chars = path_text.chars();
    if let (Some(letter), Some(':')) = (chars.next(), chars.next())
        && letter.is_ascii_alphabetic()
        && os::is_remote_drive(Path::new(&format!("{letter}:\\")))
        && let Some(share) = os::network_share_of(letter)
    {
        return path_key(&format!("{share}{}", &path_text[2..]));
    }
    path_key(path_text)
}

/// Opens the system's file manager on the folder of `path`, with the file
/// selected. To do when porting: `open -R` on macOS, the folder through
/// `xdg-open` on Linux.
pub fn reveal(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Explorer wants `/select,"path"`: one argument, quoted inside. A
        // Windows path cannot contain a quote.
        std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "not available on this system yet",
        ))
    }
}

/// While alive, the computer does not go to sleep on its own (the screen
/// may still turn off). Windows ties the request to the thread that made
/// it, so this guard cannot be sent to another thread.
pub struct KeepAwake {
    _same_thread: std::marker::PhantomData<*const ()>,
}

/// To do when porting: an IOPMAssertion on macOS, a logind inhibitor on Linux.
pub fn keep_awake() -> KeepAwake {
    os::keep_awake(true);
    KeepAwake {
        _same_thread: std::marker::PhantomData,
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        os::keep_awake(false);
    }
}

/// Renames a file, never over an existing one. Fails across volumes
/// (`is_other_volume`): the caller copies instead.
pub fn rename_noclobber(from: &Path, to: &Path) -> io::Result<()> {
    os::rename_noclobber(from, to)
}

/// The error of a rename to another volume.
pub fn is_other_volume(err: &io::Error) -> bool {
    os::is_other_volume(err)
}

/// The roots of the drives present now (`C:\` to `Z:\`), network drives
/// included. Volumes mounted on a folder are not listed.
pub fn drive_roots() -> Vec<PathBuf> {
    os::drive_roots()
}

/// The serial number of the volume holding `root`, as stored with each
/// location (`VolumePlace::serial`), if the system gives one.
pub fn volume_serial(root: &Path) -> Option<String> {
    let folder = os::open_folder(root).ok()?;
    let state = file_state(&folder).ok()?;
    state.id.map(|id| format!("{:016x}", id.volume))
}

/// Identity and state of a file, without reading its content.
pub(crate) fn identify(path: &Path) -> io::Result<FileState> {
    file_state(&File::open(extended(path))?)
}

/// Where a file sits on its volume, independently of the drive letter: the
/// volume's serial number and the path from the volume's root (as a key).
/// An external disk mounted as F: instead of E: keeps both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VolumePlace {
    pub serial: String,
    pub relative_key: String,
}

pub(crate) fn volume_place(path: &Path, id: Option<FileId>) -> Option<VolumePlace> {
    let id = id?;
    let root = volume_root_of_file(path)?;
    let relative = path.strip_prefix(&root).ok()?;
    Some(VolumePlace {
        serial: format!("{:016x}", id.volume),
        relative_key: path_key(relative.to_str()?),
    })
}

/// The extended form (`\\?\C:\…`, `\\?\UNC\server\share\…`) of an absolute
/// path, for code that calls Windows directly instead of going through the
/// standard library (which adds it on its own): without it, paths longer
/// than 260 characters fail. The path is normalised first, since Windows
/// takes an extended path as it is: `C:/Films/a.mkv` (as typed in a
/// terminal) would otherwise name nothing. Relative paths are unchanged.
pub(crate) fn extended(path: &Path) -> PathBuf {
    if cfg!(windows)
        && path.is_absolute()
        && let Ok(clean) = absolute(path)
        && let Some(text) = clean.to_str()
        && !text.starts_with(r"\\?\")
    {
        return match text.strip_prefix(r"\\") {
            Some(unc) => PathBuf::from(format!(r"\\?\UNC\{unc}")),
            None => PathBuf::from(format!(r"\\?\{text}")),
        };
    }
    path.to_path_buf()
}

fn strip_verbatim(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path.to_path_buf()
    }
}

/// The path as text, for databases and reports. Windows names are UTF-16
/// and may (very rarely) not be valid Unicode: such a file cannot be
/// recorded faithfully, so it is reported instead of being stored garbled.
pub fn path_text(path: &Path) -> Result<String, Problem> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        Problem::new(
            ProblemKind::NameNotRepresentable,
            format!("{} is not valid Unicode", path.display()),
        )
    })
}

/// Key used to find a known location again. Windows and macOS file systems
/// ignore case by default, Linux does not. This key never decides identity
/// on its own: it only finds candidates, which are then checked by hash.
pub fn path_key(path_text: &str) -> String {
    if cfg!(any(windows, target_os = "macos")) {
        path_text.to_lowercase()
    } else {
        path_text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn verbatim_prefixes_are_removed() {
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\C:\Films\a.mkv")),
            PathBuf::from(r"C:\Films\a.mkv")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\nas\films\a.mkv")),
            PathBuf::from(r"\\nas\films\a.mkv")
        );
        assert_eq!(
            absolute(Path::new(r"C:\Films\x\..\a.mkv")).unwrap(),
            PathBuf::from(r"C:\Films\a.mkv")
        );
    }

    #[cfg(windows)]
    #[test]
    fn absolute_paths_keep_trailing_dots_and_spaces() {
        // `name.` and `name` are two different files (made with `\\?\`).
        assert_eq!(
            absolute(Path::new(r"C:\Films\name.")).unwrap(),
            PathBuf::from(r"C:\Films\name.")
        );
        assert_eq!(
            absolute(Path::new(r"C:\Films\space ")).unwrap(),
            PathBuf::from(r"C:\Films\space ")
        );
        assert_eq!(
            absolute(Path::new(r"C:\Films\.\a\\b.mkv\")).unwrap(),
            PathBuf::from(r"C:\Films\a\b.mkv")
        );
        assert_eq!(canonical_key(r"C:\Films\A.mkv"), r"c:\films\a.mkv");
    }

    #[cfg(windows)]
    #[test]
    fn extended_paths_for_direct_windows_calls() {
        assert_eq!(
            extended(Path::new(r"C:\Films\a.mkv")),
            PathBuf::from(r"\\?\C:\Films\a.mkv")
        );
        assert_eq!(
            extended(Path::new(r"\\nas\films\a.mkv")),
            PathBuf::from(r"\\?\UNC\nas\films\a.mkv")
        );
        let already = Path::new(r"\\?\C:\a.mkv");
        assert_eq!(extended(already), already, "not prefixed twice");
        // As typed in a terminal: the extended form takes only backslashes.
        assert_eq!(
            extended(Path::new("C:/Films/./SF/a.mkv")),
            PathBuf::from(r"\\?\C:\Films\SF\a.mkv")
        );
        assert_eq!(
            extended(Path::new(r"C:\Films\x\..\a.mkv")),
            PathBuf::from(r"\\?\C:\Films\a.mkv")
        );
        assert_eq!(
            extended(Path::new("relative.mkv")),
            Path::new("relative.mkv")
        );
    }

    #[cfg(windows)]
    #[test]
    fn staying_awake_is_asked_then_released() {
        let awake = keep_awake();
        // Each call returns the request in force before it.
        let during = os::keep_awake(true);
        assert_ne!(during & os::SYSTEM_REQUIRED, 0);
        drop(awake);
        let after = os::keep_awake(false);
        assert_eq!(after & os::SYSTEM_REQUIRED, 0);
    }

    #[test]
    fn the_root_remembered_for_a_file_is_its_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.bin");
        std::fs::write(&file, b"x").unwrap();
        let root = volume_root(dir.path()).unwrap();
        assert_eq!(volume_root_of_file(&file), Some(root.clone()));
        // Asked again: from memory, the same answer.
        assert_eq!(volume_root_of_file(&dir.path().join("b.bin")), Some(root));
        assert!(volume_available(&file));
    }

    #[cfg(windows)]
    #[test]
    fn path_keys_ignore_case_on_windows() {
        assert_eq!(path_key(r"C:\Films\Été.MKV"), r"c:\films\été.mkv");
    }

    fn state_of(path: &Path) -> FileState {
        file_state(&File::open(path).unwrap()).unwrap()
    }

    #[test]
    fn file_identity_follows_the_file_not_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.bin");
        std::fs::write(&a, b"same content").unwrap();
        let copy = dir.path().join("copy.bin");
        std::fs::copy(&a, &copy).unwrap();
        let first = state_of(&a);
        assert!(
            first.id.is_some(),
            "NTFS and Unix file systems give an identity"
        );
        assert!(first.changed.is_some());
        assert_ne!(first.id, state_of(&copy).id, "a copy is another file");

        let renamed = dir.path().join("renamed.bin");
        std::fs::rename(&a, &renamed).unwrap();
        assert_eq!(
            state_of(&renamed).id,
            first.id,
            "a rename keeps the identity"
        );

        let link = dir.path().join("hard link.bin");
        std::fs::hard_link(&renamed, &link).unwrap();
        assert_eq!(state_of(&link).id, first.id, "hard links are the same file");
    }

    #[test]
    fn a_write_changes_the_change_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        std::fs::write(&path, b"one").unwrap();
        let before = state_of(&path);
        // Same size, last write time put back: only the change time tells.
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::io::Write::write_all(&mut &file, b"two").unwrap();
        file.set_modified(before.modified.unwrap()).unwrap();
        drop(file);
        let after = state_of(&path);
        assert_eq!((after.len, after.modified), (before.len, before.modified));
        assert_ne!(after.changed, before.changed);
    }
}
