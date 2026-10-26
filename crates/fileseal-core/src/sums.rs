//! Hash lists of a folder (`SHA256SUMS`, `MD5SUMS`): one line per file of
//! the folder, in the format of the hash files (`sidecar`), checkable with
//! `sha256sum -c SHA256SUMS` from that folder. One list per folder and per
//! algorithm, naming the files of that folder only (no subfolder): it stays
//! right when the folder is moved or renamed. Specification:
//! `docs/formats-hash.md`.
//!
//! Writing adds lines and never changes an existing one: the lines already
//! there are kept byte for byte, the new ones follow, and the whole list is
//! published atomically (a temporary file, then a rename over the old list).
//! A list that cannot be read is never replaced. Reading goes through a
//! small cache: a batch looks up every file of a folder in the same list.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use crate::algorithm::{Algorithm, Digest};
use crate::faults::{self, FaultPoint};
use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::sidecar::{self, Entry, Parsed, SidecarLookup, SidecarNote};

/// The name of a folder's list for `algorithm`: `SHA256SUMS`, `MD5SUMS`.
pub fn name_for(algorithm: Algorithm) -> String {
    format!("{}SUMS", algorithm.code().to_uppercase())
}

/// The list of the folder of `file` for `algorithm`.
pub fn path_for(file: &Path, algorithm: Algorithm) -> Option<PathBuf> {
    Some(file.parent()?.join(name_for(algorithm)))
}

/// The line of `file` in its folder's list for `algorithm`: absent when
/// there is no list or no line for this name.
pub fn lookup(file: &Path, algorithm: Algorithm) -> SidecarLookup {
    let (Some(path), Some(name)) = (
        path_for(file, algorithm),
        file.file_name().map(|n| n.to_string_lossy().into_owned()),
    ) else {
        return SidecarLookup::Absent;
    };
    let parsed = match read(&path) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return SidecarLookup::Absent,
        Err(reason) => return SidecarLookup::Invalid { path, reason },
    };
    match find(&parsed, &name, algorithm) {
        Found::None => SidecarLookup::Absent,
        Found::Line(digest, notes) => SidecarLookup::Found {
            path,
            digest,
            notes,
        },
        Found::Unusable(reason) => SidecarLookup::Invalid { path, reason },
    }
}

/// What a list says about one name.
enum Found {
    None,
    Line(Digest, Vec<SidecarNote>),
    Unusable(String),
}

/// The line of `name`: the same name, else the same name but for case on a
/// file system that ignores case. Unlike a hash file of its own, a list's
/// only line is never taken for another name.
fn find(parsed: &Parsed, name: &str, algorithm: Algorithm) -> Found {
    let bare = |entry: &Entry| {
        let written = entry.name.as_str();
        written
            .strip_prefix("./")
            .or_else(|| written.strip_prefix(".\\"))
            .unwrap_or(written)
            .to_owned()
    };
    let mut lines: Vec<&Entry> = parsed.entries.iter().filter(|e| bare(e) == name).collect();
    let mut notes = Vec::new();
    if lines.is_empty() && cfg!(any(windows, target_os = "macos")) {
        let lower = name.to_lowercase();
        lines = parsed
            .entries
            .iter()
            .filter(|e| bare(e).to_lowercase() == lower)
            .collect();
        if let Some(first) = lines.first() {
            notes.push(SidecarNote::NameCaseDiffers {
                written: first.name.clone(),
            });
        }
    }
    let Some(first) = lines.first() else {
        return Found::None;
    };
    if lines
        .iter()
        .any(|e| !e.hex.eq_ignore_ascii_case(&first.hex))
    {
        return Found::Unusable(format!("its lines for {name} do not agree"));
    }
    if first.tagged.is_some_and(|tagged| tagged != algorithm) {
        return Found::Unusable(format!(
            "the line for {name} is tagged {:?} in a {} list",
            first.tagged.unwrap(),
            name_for(algorithm)
        ));
    }
    let Some(digest) = Digest::parse(algorithm, &first.hex) else {
        return Found::Unusable(format!(
            "\"{}\" is not a {} hash ({} hexadecimal digits expected)",
            first.hex,
            algorithm.code(),
            algorithm.hex_len()
        ));
    };
    if first.non_standard {
        notes.insert(0, SidecarNote::NonStandardFormat);
    }
    if parsed.legacy_encoding {
        notes.insert(0, SidecarNote::LegacyEncoding);
    }
    Found::Line(digest, notes)
}

/// What became of one line asked to be added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    Written,
    /// The list already had this line.
    AlreadyPresent,
    /// The list has another value for this name, or lines that disagree:
    /// left as it was. `existing` is the value found, if one.
    Conflict {
        existing: Option<String>,
    },
}

/// Adds the lines of files of `dir` (name, digest) to its list for
/// `algorithm`, creating it if needed. Lines already there stay as they
/// are; a name with another value is a conflict, never changed. One
/// publication for all the lines given. Fails, leaving the list as it was,
/// if it cannot be read or written; read again if another program changed
/// it meanwhile.
pub fn add(
    dir: &Path,
    algorithm: Algorithm,
    lines: &[(String, Digest)],
) -> Result<(PathBuf, Vec<Added>), Problem> {
    let target = dir.join(name_for(algorithm));
    for _ in 0..3 {
        let before = state(&target).map_err(|err| Problem::from_io(&err))?;
        let bytes = match &before {
            Some(_) => sidecar::read_limited(&target)
                .map_err(|reason| unusable(&target, &reason))?
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let parsed = sidecar::parse(&bytes);
        if before.is_some() && parsed.entries.is_empty() && !parsed.invalid_lines.is_empty() {
            return Err(unusable(&target, "no valid line"));
        }
        let mut outcomes = Vec::with_capacity(lines.len());
        let mut new: Vec<(&str, &Digest)> = Vec::new();
        for (name, digest) in lines {
            outcomes.push(match find(&parsed, name, algorithm) {
                Found::Line(existing, _) if existing == *digest => Added::AlreadyPresent,
                Found::Line(existing, _) => Added::Conflict {
                    existing: Some(existing.hex),
                },
                Found::Unusable(_) => Added::Conflict { existing: None },
                // The same name asked twice: once is enough, if they agree.
                Found::None => match new.iter().find(|(n, _)| *n == name.as_str()) {
                    Some((_, first)) if *first == digest => Added::AlreadyPresent,
                    Some((_, first)) => Added::Conflict {
                        existing: Some(first.hex.clone()),
                    },
                    None => {
                        new.push((name, digest));
                        Added::Written
                    }
                },
            });
        }
        if new.is_empty() {
            return Ok((target, outcomes));
        }
        new.sort_by(|a, b| {
            a.0.to_lowercase()
                .cmp(&b.0.to_lowercase())
                .then(a.0.cmp(b.0))
        });
        // The lines already there, as they are; the new ones in the same
        // line ending (a list written with CRLF stays so).
        let newline: &[u8] = if bytes.windows(2).any(|w| w == b"\r\n") {
            b"\r\n"
        } else {
            b"\n"
        };
        let mut content = bytes.clone();
        if !content.is_empty() && !content.ends_with(b"\n") && !content.ends_with(b"\r") {
            content.extend_from_slice(newline);
        }
        for (name, digest) in &new {
            let line = sidecar::format_line(&digest.hex, name);
            content.extend_from_slice(line.trim_end_matches('\n').as_bytes());
            content.extend_from_slice(newline);
        }
        match publish(dir, &target, &content, &before)? {
            true => {
                forget(&target);
                return Ok((target, outcomes));
            }
            // Changed by another program since it was read: read it again.
            false => continue,
        }
    }
    Err(Problem::new(
        ProblemKind::Io,
        format!(
            "{} kept changing while it was written: try again",
            target.display()
        ),
    ))
}

/// Accepting a new version: the line of `file` in its folder's list takes
/// the accepted value, every other byte of the list as it was. `None` when
/// the list has no line for it (none is added: accepting changes what
/// exists). The only change ever made to an existing line, and only on the
/// user's decision.
pub fn replace(file: &Path, digest: &Digest) -> Result<Option<PathBuf>, Problem> {
    let algorithm = digest.algorithm;
    let (Some(target), Some(name)) = (
        path_for(file, algorithm),
        file.file_name().map(|n| n.to_string_lossy().into_owned()),
    ) else {
        return Ok(None);
    };
    let dir = target.parent().unwrap_or(Path::new("")).to_path_buf();
    for _ in 0..3 {
        let before = state(&target).map_err(|err| Problem::from_io(&err))?;
        if before.is_none() {
            return Ok(None);
        }
        let bytes = sidecar::read_limited(&target)
            .map_err(|reason| unusable(&target, &reason))?
            .unwrap_or_default();
        let parsed = sidecar::parse(&bytes);
        let lower = name.to_lowercase();
        let insensitive = cfg!(any(windows, target_os = "macos"));
        let lines: Vec<&Entry> = parsed
            .entries
            .iter()
            .filter(|e| {
                let written = e.name.strip_prefix("./").unwrap_or(&e.name);
                written == name || (insensitive && written.to_lowercase() == lower)
            })
            .collect();
        if lines.is_empty() {
            return Ok(None);
        }
        // Lines are counted as the parser counts them; with lone CRs (very
        // old Mac files) the raw lines would not match: left as it is.
        if bytes
            .iter()
            .enumerate()
            .any(|(i, b)| *b == b'\r' && bytes.get(i + 1) != Some(&b'\n'))
        {
            return Err(unusable(&target, "its line endings are not handled"));
        }
        let mut raw: Vec<Vec<u8>> = bytes.split(|b| *b == b'\n').map(<[u8]>::to_vec).collect();
        for entry in lines {
            let line = &mut raw[entry.line - 1];
            let Some(at) = line
                .windows(entry.hex.len())
                .position(|w| w == entry.hex.as_bytes())
            else {
                return Err(unusable(&target, "a line could not be found again"));
            };
            line.splice(at..at + entry.hex.len(), digest.hex.bytes());
        }
        let content = raw.join(&b'\n');
        if publish(&dir, &target, &content, &before)? {
            forget(&target);
            return Ok(Some(target));
        }
    }
    Err(Problem::new(
        ProblemKind::Io,
        format!(
            "{} kept changing while it was written: try again",
            target.display()
        ),
    ))
}

/// Size and date of a list, `None` when there is none.
type State = Option<(u64, Option<SystemTime>)>;

fn state(path: &Path) -> io::Result<State> {
    match std::fs::metadata(platform::extended(path)) {
        Ok(metadata) => Ok(Some((metadata.len(), metadata.modified().ok()))),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Writes `content` to a temporary file of `dir`, flushed to the disk, then
/// renames it over `target`: only if `target` is still as it was read
/// (`before`); false when it changed. A new list never replaces one written
/// meanwhile by someone else.
fn publish(dir: &Path, target: &Path, content: &[u8], before: &State) -> Result<bool, Problem> {
    let io_problem = |err: io::Error| Problem::from_io(&err);
    let mut temp = tempfile::Builder::new()
        .prefix(".fileseal-")
        .suffix(".tmp")
        .tempfile_in(platform::extended(dir))
        .map_err(io_problem)?;
    temp.write_all(content).map_err(io_problem)?;
    temp.as_file().sync_all().map_err(io_problem)?;
    faults::check(FaultPoint::SidecarPublish, target).map_err(io_problem)?;
    if state(target).map_err(io_problem)? != *before {
        return Ok(false);
    }
    // On failure the temporary file is deleted when `err.file` is dropped.
    let published = match before {
        Some(_) => temp.persist(platform::extended(target)),
        None => temp.persist_noclobber(platform::extended(target)),
    };
    match published {
        Ok(_) => Ok(true),
        Err(err) if err.error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(err) => Err(Problem::from_io(&err.error)),
    }
}

fn unusable(path: &Path, reason: &str) -> Problem {
    Problem::new(
        ProblemKind::InvalidHashFile,
        format!("{}: {reason}; left as it is", path.display()),
    )
}

/// Lists recently read, by path, size and date.
struct Cached {
    path: PathBuf,
    state: State,
    parsed: Arc<Parsed>,
}

static CACHE: Mutex<Vec<Cached>> = Mutex::new(Vec::new());

/// How many lists are kept: one per folder being read at the same time is
/// enough (one reader per disk).
const CACHED: usize = 8;

fn read(path: &Path) -> Result<Option<Arc<Parsed>>, String> {
    let current = state(path).map_err(|err| format!("cannot be read: {err}"))?;
    if current.is_none() {
        return Ok(None);
    }
    {
        let cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = cache.iter().find(|c| c.path == path && c.state == current) {
            return Ok(Some(cached.parsed.clone()));
        }
    }
    let Some(bytes) = sidecar::read_limited(path)? else {
        return Ok(None);
    };
    let parsed = Arc::new(sidecar::parse(&bytes));
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    cache.retain(|c| c.path != path);
    cache.insert(
        0,
        Cached {
            path: path.to_owned(),
            state: current,
            parsed: parsed.clone(),
        },
    );
    cache.truncate(CACHED);
    Ok(Some(parsed))
}

/// A list just written is read again from the disk next time.
fn forget(path: &Path) {
    CACHE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|c| c.path != path);
}

#[cfg(test)]
mod tests;
