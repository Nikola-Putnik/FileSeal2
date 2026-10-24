//! Fault injection for tests: lets a test make a given step fail (disk
//! full, unplugged drive, network lost, file changed during the read…)
//! without real hardware. Compiled to nothing outside `cfg(test)`.
//!
//! Two ways to arm a fault:
//! - for the current thread (`arm`), when the test runs the code itself;
//! - for every path under a folder (`arm_under`), when the code runs on
//!   other threads (a batch's readers). Each test works in its own
//!   temporary folder, so tests running in parallel do not see each
//!   other's faults.

use std::io;
use std::path::Path;

use crate::platform::FileState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaultPoint {
    /// Just before a hash file is renamed into place.
    SidecarPublish,
    /// Inside the transaction that writes to the database.
    DatabaseWrite,
    /// After each block of a source file is read.
    Read,
    /// The file appears changed when its state is taken after the read
    /// (only `tamper` uses it, and only in tests).
    #[cfg_attr(not(test), allow(dead_code))]
    ChangeDuringRead,
    /// The volume holding the path looks gone (unplugged disk, lost share).
    VolumeGone,
    /// Each block takes 20 ms more to read: a slow disk, so that a test can
    /// act while a batch is under way.
    #[cfg_attr(not(test), allow(dead_code))]
    SlowRead,
}

#[cfg(test)]
type Armed = (FaultPoint, Option<i32>);

#[cfg(test)]
thread_local! {
    static ARMED: std::cell::RefCell<Vec<Armed>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
static ARMED_UNDER: std::sync::Mutex<Vec<(std::path::PathBuf, Armed)>> =
    std::sync::Mutex::new(Vec::new());

/// Makes every later `check(point)` on this thread fail, until `clear`.
#[cfg(test)]
pub(crate) fn arm(point: FaultPoint) {
    ARMED.with(|armed| armed.borrow_mut().push((point, None)));
}

/// Same, failing with this operating-system error code (e.g. 64 for a
/// network name that disappeared on Windows).
#[cfg(test)]
pub(crate) fn arm_os_error(point: FaultPoint, code: i32) {
    ARMED.with(|armed| armed.borrow_mut().push((point, Some(code))));
}

#[cfg(test)]
pub(crate) fn clear() {
    ARMED.with(|armed| armed.borrow_mut().clear());
}

/// Makes `check(point, path)` fail on every thread for the paths under
/// `folder`, with this operating-system error code if given.
#[cfg(test)]
pub(crate) fn arm_under(folder: &Path, point: FaultPoint, code: Option<i32>) {
    lock_under().push((folder.to_path_buf(), (point, code)));
}

/// Removes the faults armed under `folder`.
#[cfg(test)]
pub(crate) fn clear_under(folder: &Path) {
    lock_under().retain(|(armed, _)| armed != folder);
}

#[cfg(test)]
fn lock_under() -> std::sync::MutexGuard<'static, Vec<(std::path::PathBuf, Armed)>> {
    ARMED_UNDER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
fn armed(point: FaultPoint, path: &Path) -> Option<Option<i32>> {
    let here = ARMED.with(|armed| {
        armed
            .borrow()
            .iter()
            .find(|(p, _)| *p == point)
            .map(|(_, c)| *c)
    });
    here.or_else(|| {
        lock_under()
            .iter()
            .find(|(folder, (p, _))| *p == point && path.starts_with(folder))
            .map(|(_, (_, c))| *c)
    })
}

#[cfg(test)]
pub(crate) fn check(point: FaultPoint, path: &Path) -> io::Result<()> {
    match armed(point, path) {
        Some(Some(code)) => Err(io::Error::from_raw_os_error(code)),
        Some(None) => Err(io::Error::other(format!("injected fault at {point:?}"))),
        None => Ok(()),
    }
}

/// Simulates a write by another program during the read: one that kept the
/// size and the last write time, so only the change time reveals it.
#[cfg(test)]
pub(crate) fn tamper(state: &mut FileState, path: &Path) {
    if armed(FaultPoint::ChangeDuringRead, path).is_some() {
        state.changed = Some(state.changed.unwrap_or_default() + 1);
    }
}

/// Waits after a block read, if `SlowRead` is armed for this path.
#[cfg(test)]
pub(crate) fn slow_down(path: &Path) {
    if armed(FaultPoint::SlowRead, path).is_some() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(not(test))]
#[inline(always)]
pub(crate) fn slow_down(_path: &Path) {}

#[cfg(not(test))]
#[inline(always)]
pub(crate) fn check(_point: FaultPoint, _path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(not(test))]
#[inline(always)]
pub(crate) fn tamper(_state: &mut FileState, _path: &Path) {}
