//! Streaming read of a source file: constant memory whatever its size,
//! cancellable between blocks, and checked for changes made during the read.

use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use crate::algorithm::{Algorithm, Digest, MultiHasher};
use crate::faults::{self, FaultPoint};
use crate::platform::{self, FileId, FileState};
use crate::problem::{Problem, ProblemKind};

/// 1 MiB: large enough for disks and SMB shares, small enough for the
/// cancel button to answer at once.
const BLOCK: usize = 1 << 20;

/// Shared between the UI thread (which pauses, resumes or cancels) and the
/// workers (which read).
#[derive(Debug, Default)]
pub struct Control {
    cancelled: AtomicBool,
    paused: AtomicBool,
    /// Held to change `paused` or `cancelled` and to wait for them.
    gate: Mutex<()>,
    wake: Condvar,
}

impl Control {
    pub fn new() -> Control {
        Control::default()
    }

    pub fn cancel(&self) {
        let _gate = self.lock_gate();
        self.cancelled.store(true, Ordering::Relaxed);
        self.wake.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Readers stop at their next block, keeping their file open, until
    /// `resume` (or `cancel`).
    pub fn pause(&self) {
        let _gate = self.lock_gate();
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        let _gate = self.lock_gate();
        self.paused.store(false, Ordering::Relaxed);
        self.wake.notify_all();
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Called between two blocks, or two files: waits while paused, and
    /// fails once cancelled (also while waiting).
    pub fn checkpoint(&self) -> Result<(), Problem> {
        if self.is_paused() && !self.is_cancelled() {
            let mut gate = self.lock_gate();
            while self.is_paused() && !self.is_cancelled() {
                gate = self.wake.wait(gate).unwrap_or_else(PoisonError::into_inner);
            }
        }
        if self.is_cancelled() {
            return Err(Problem::cancelled());
        }
        Ok(())
    }

    fn lock_gate(&self) -> MutexGuard<'_, ()> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A file read completely and found unchanged from the first byte to the last.
#[derive(Debug, Clone)]
pub struct HashedFile {
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// Identity of the file read (volume + file number), if known.
    pub id: Option<FileId>,
    pub digests: Vec<Digest>,
}

/// Size of a file without reading it, for progress totals.
pub fn file_size(path: &Path) -> Result<u64, Problem> {
    let metadata =
        std::fs::metadata(platform::extended(path)).map_err(|err| Problem::from_io(&err))?;
    if metadata.is_dir() {
        return Err(not_a_file(path));
    }
    Ok(metadata.len())
}

/// Reads the whole file once and computes every requested digest.
///
/// `progress(done, total)` is called after each block. The digests are
/// only returned if the file looks exactly the same after the read as
/// before (size, last write time, change time, identity) and the number of
/// bytes read equals its size: otherwise it changed while we read it and the
/// digest describes no real state of the file.
///
/// Limit: this is not a consistent snapshot. Denying other programs write
/// access during the read is what makes a change in between unlikely; the
/// comparison catches what gets through (e.g. writes on a network share by
/// the server itself).
pub fn hash_file(
    path: &Path,
    algorithms: &[Algorithm],
    control: &Control,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<HashedFile, Problem> {
    let io_problem = |err: io::Error| Problem::from_io(&err);
    if std::fs::metadata(platform::extended(path)).is_ok_and(|m| m.is_dir()) {
        return Err(not_a_file(path));
    }
    let mut file = platform::open_for_hashing(path).map_err(io_problem)?;
    let before = platform::file_state(&file).map_err(io_problem)?;
    let mut hasher = MultiHasher::new(algorithms);
    let mut buffer = vec![0u8; BLOCK];
    let mut done = 0u64;
    progress(0, before.len);
    loop {
        control.checkpoint()?;
        let read = match file.read(&mut buffer) {
            Ok(read) => read,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(io_problem(err)),
        };
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        done += read as u64;
        faults::check(FaultPoint::Read, path).map_err(io_problem)?;
        faults::slow_down(path);
        progress(done, before.len);
    }
    let mut after = platform::file_state(&file).map_err(io_problem)?;
    faults::tamper(&mut after, path);
    let changes = changes(&before, &after, done);
    if !changes.is_empty() {
        return Err(Problem::new(
            ProblemKind::ModifiedDuringRead,
            changes.join("; "),
        ));
    }
    Ok(HashedFile {
        size: done,
        modified: before.modified,
        id: before.id,
        digests: hasher.finish(),
    })
}

/// Everything that differs between the states before and after the read,
/// in words for the technical detail. Empty when nothing changed.
fn changes(before: &FileState, after: &FileState, read: u64) -> Vec<String> {
    let mut changes = Vec::new();
    if read != before.len {
        changes.push(format!("read {read} bytes of {}", before.len));
    }
    if after.len != before.len {
        changes.push(format!("size {} → {} bytes", before.len, after.len));
    }
    if after.modified != before.modified {
        changes.push("last write time changed".to_owned());
    }
    // Compared only when known on both sides: some file systems give none.
    if before.changed.is_some() && after.changed.is_some() && after.changed != before.changed {
        changes.push("change time changed".to_owned());
    }
    if before.id.is_some() && after.id.is_some() && after.id != before.id {
        changes.push("file identity changed".to_owned());
    }
    changes
}

fn not_a_file(path: &Path) -> Problem {
    Problem::new(
        ProblemKind::NotAFile,
        format!("{} is a folder", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algorithm::find;

    fn sha256_of(data: &[u8]) -> String {
        let mut hasher = MultiHasher::new(&[Algorithm::Sha256]);
        hasher.update(data);
        hasher.finish().remove(0).hex
    }

    #[test]
    fn hashes_files_spanning_several_blocks() {
        let dir = tempfile::tempdir().unwrap();
        // Not a multiple of the block size, so the last block is partial.
        let data: Vec<u8> = (0..(BLOCK * 3 + 12_345))
            .map(|i| (i * 31 % 251) as u8)
            .collect();
        let path = dir.path().join("data.bin");
        std::fs::write(&path, &data).unwrap();
        let mut calls = Vec::new();
        let hashed = hash_file(&path, &[Algorithm::Sha256], &Control::new(), &mut |d, t| {
            calls.push((d, t))
        })
        .unwrap();
        assert_eq!(hashed.size, data.len() as u64);
        assert_eq!(
            find(&hashed.digests, Algorithm::Sha256).unwrap().hex,
            sha256_of(&data),
            "streaming gives the same digest as a one-shot hash"
        );
        assert_eq!(calls.first(), Some(&(0, data.len() as u64)));
        assert_eq!(calls.last(), Some(&(data.len() as u64, data.len() as u64)));
    }

    #[test]
    fn empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty");
        std::fs::write(&path, b"").unwrap();
        let hashed =
            hash_file(&path, &[Algorithm::Sha256], &Control::new(), &mut |_, _| {}).unwrap();
        assert_eq!(hashed.size, 0);
        assert_eq!(
            hashed.digests[0].hex,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn cancelling_stops_between_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        std::fs::write(&path, vec![7u8; BLOCK * 4]).unwrap();
        let control = Control::new();
        let err = hash_file(&path, &[Algorithm::Sha256], &control, &mut |done, _| {
            if done >= BLOCK as u64 {
                control.cancel();
            }
        })
        .unwrap_err();
        assert_eq!(err.kind, ProblemKind::Cancelled);
    }

    #[test]
    fn a_pause_holds_the_read_until_resumed_or_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        std::fs::write(&path, vec![7u8; BLOCK * 4]).unwrap();
        for cancel in [false, true] {
            let control = Control::new();
            let done_at_pause = std::sync::atomic::AtomicU64::new(0);
            let result = std::thread::scope(|scope| {
                let reader = scope.spawn(|| {
                    hash_file(&path, &[Algorithm::Sha256], &control, &mut |done, _| {
                        if done == BLOCK as u64 {
                            control.pause();
                            done_at_pause.store(done, Ordering::Relaxed);
                        }
                        // Nothing more is read while paused.
                        assert!(!control.is_paused() || done == BLOCK as u64);
                    })
                });
                while done_at_pause.load(Ordering::Relaxed) == 0 {
                    std::thread::yield_now();
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
                assert!(!reader.is_finished(), "waiting while paused");
                if cancel {
                    control.cancel();
                } else {
                    control.resume();
                }
                reader.join().unwrap()
            });
            match result {
                Ok(hashed) => assert!(!cancel && hashed.size == BLOCK as u64 * 4),
                Err(problem) => assert!(cancel && problem.kind == ProblemKind::Cancelled),
            }
        }
    }

    #[test]
    fn missing_file_and_folder() {
        let dir = tempfile::tempdir().unwrap();
        let missing = hash_file(
            &dir.path().join("nope"),
            &[Algorithm::Sha256],
            &Control::new(),
            &mut |_, _| {},
        )
        .unwrap_err();
        assert_eq!(missing.kind, ProblemKind::NotFound);
        let folder = hash_file(
            dir.path(),
            &[Algorithm::Sha256],
            &Control::new(),
            &mut |_, _| {},
        )
        .unwrap_err();
        assert_eq!(folder.kind, ProblemKind::NotAFile);
    }

    #[cfg(windows)]
    #[test]
    fn a_file_open_for_writing_elsewhere_is_reported_locked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("busy.bin");
        std::fs::write(&path, b"data").unwrap();
        let _writer = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let err =
            hash_file(&path, &[Algorithm::Sha256], &Control::new(), &mut |_, _| {}).unwrap_err();
        assert_eq!(err.kind, ProblemKind::Locked);
    }

    #[cfg(windows)]
    #[test]
    fn nobody_can_write_while_we_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guarded.bin");
        std::fs::write(&path, vec![1u8; BLOCK * 2]).unwrap();
        let mut write_attempt = None;
        hash_file(
            &path,
            &[Algorithm::Sha256],
            &Control::new(),
            &mut |done, _| {
                if done > 0 && write_attempt.is_none() {
                    write_attempt = Some(std::fs::OpenOptions::new().write(true).open(&path));
                }
            },
        )
        .unwrap();
        let err = write_attempt.unwrap().unwrap_err();
        assert_eq!(err.raw_os_error(), Some(32), "sharing violation");
    }

    #[cfg(windows)]
    #[test]
    fn nobody_can_rename_or_delete_while_we_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guarded.bin");
        std::fs::write(&path, vec![1u8; BLOCK * 2]).unwrap();
        let mut attempts = None;
        hash_file(
            &path,
            &[Algorithm::Sha256],
            &Control::new(),
            &mut |done, _| {
                if done > 0 && attempts.is_none() {
                    attempts = Some((
                        std::fs::rename(&path, dir.path().join("moved.bin")),
                        std::fs::remove_file(&path),
                    ));
                }
            },
        )
        .unwrap();
        let (rename, delete) = attempts.unwrap();
        assert!(rename.is_err() && delete.is_err());
        assert!(path.exists());
    }

    #[test]
    fn two_algorithms_are_computed_in_a_single_read() {
        let dir = tempfile::tempdir().unwrap();
        let data: Vec<u8> = (0..(BLOCK * 2 + 7)).map(|i| (i % 253) as u8).collect();
        let path = dir.path().join("both.bin");
        std::fs::write(&path, &data).unwrap();
        let mut seen = Vec::new();
        let hashed = hash_file(
            &path,
            &[Algorithm::Md5, Algorithm::Sha256],
            &Control::new(),
            &mut |done, _| seen.push(done),
        )
        .unwrap();
        assert!(
            seen.windows(2).all(|w| w[0] < w[1]),
            "one pass, never restarted"
        );
        assert_eq!(seen.last(), Some(&(data.len() as u64)));
        assert_eq!(
            find(&hashed.digests, Algorithm::Sha256).unwrap().hex,
            sha256_of(&data)
        );
        let mut md5 = MultiHasher::new(&[Algorithm::Md5]);
        md5.update(&data);
        assert_eq!(find(&hashed.digests, Algorithm::Md5), md5.finish().first());
    }

    fn state(len: u64) -> FileState {
        FileState {
            len,
            modified: Some(SystemTime::UNIX_EPOCH),
            changed: Some(100),
            id: Some(FileId { volume: 1, file: 2 }),
        }
    }

    #[test]
    fn every_kind_of_change_is_detected() {
        let before = state(10);
        assert!(changes(&before, &before, 10).is_empty());
        assert_eq!(changes(&before, &before, 9), ["read 9 bytes of 10"]);
        assert_eq!(changes(&before, &state(12), 10), ["size 10 → 12 bytes"]);
        let mut touched = state(10);
        touched.modified = Some(SystemTime::now());
        assert_eq!(changes(&before, &touched, 10), ["last write time changed"]);
        let mut changed = state(10);
        changed.changed = Some(101);
        assert_eq!(changes(&before, &changed, 10), ["change time changed"]);
        let mut replaced = state(10);
        replaced.id = Some(FileId { volume: 1, file: 3 });
        assert_eq!(changes(&before, &replaced, 10), ["file identity changed"]);
        let mut unknown = state(10);
        (unknown.changed, unknown.id) = (None, None);
        assert!(
            changes(&before, &unknown, 10).is_empty(),
            "what a file system does not give is not a change"
        );
    }

    #[test]
    fn a_change_during_the_read_gives_no_digest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        faults::arm(FaultPoint::ChangeDuringRead);
        let result = hash_file(&path, &[Algorithm::Sha256], &Control::new(), &mut |_, _| {});
        faults::clear();
        let err = result.unwrap_err();
        assert_eq!(err.kind, ProblemKind::ModifiedDuringRead);
        assert_eq!(err.detail, "change time changed");
    }

    #[cfg(windows)]
    #[test]
    fn read_errors_in_the_middle_of_a_file_are_classified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        std::fs::write(&path, vec![0u8; BLOCK * 2]).unwrap();
        for (code, kind) in [
            (21, ProblemKind::DeviceUnavailable), // ERROR_NOT_READY: drive unplugged
            (1167, ProblemKind::DeviceUnavailable), // ERROR_DEVICE_NOT_CONNECTED
            (64, ProblemKind::NetworkError),      // ERROR_NETNAME_DELETED: share lost
            (59, ProblemKind::NetworkError),      // ERROR_UNEXP_NET_ERR
            (23, ProblemKind::MediaError),        // ERROR_CRC: unreadable sector
            (1117, ProblemKind::MediaError),      // ERROR_IO_DEVICE
        ] {
            faults::arm_os_error(FaultPoint::Read, code);
            let result = hash_file(&path, &[Algorithm::Sha256], &Control::new(), &mut |_, _| {});
            faults::clear();
            assert_eq!(result.unwrap_err().kind, kind, "os error {code}");
        }
    }
}
