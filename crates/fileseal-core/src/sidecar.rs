//! Hash files next to source files (`movie.mkv.sha256`), compatible with
//! GNU `sha256sum -c` / `md5sum -c`. Full specification with examples:
//! `docs/formats-hash.md`.
//!
//! Writing is strict: UTF-8 without BOM, `<hex>  <name>\n`, file name only
//! (so the pair can be moved together), published atomically and never over
//! an existing file. Reading is tolerant: it also accepts what other tools
//! and older FileSeal versions wrote (CRLF, BOM, `*name`, BSD tags,
//! `MD5: hash  name`, cp1252).

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::algorithm::{Algorithm, Digest};
use crate::faults::{self, FaultPoint};
use crate::platform;
use crate::problem::{Problem, ProblemKind};

/// A hash file is a few hundred bytes; anything much bigger is not one.
const MAX_SIZE: u64 = 16 << 20;

/// Where the hash file of `file` for `algorithm` is, or would be.
pub fn path_for(file: &Path, algorithm: Algorithm) -> Option<PathBuf> {
    let name = file.file_name()?;
    let mut sidecar = name.to_os_string();
    sidecar.push(".");
    sidecar.push(algorithm.code());
    Some(file.with_file_name(sidecar))
}

/// One line in GNU format. A name containing `\`, LF or CR (impossible on
/// Windows, possible on Linux) is escaped the way coreutils does it: the
/// line starts with `\` and those characters become `\\`, `\n`, `\r`.
pub fn format_line(hex: &str, name: &str) -> String {
    if name.contains(['\\', '\n', '\r']) {
        let escaped = name
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        format!("\\{hex}  {escaped}\n")
    } else {
        format!("{hex}  {name}\n")
    }
}

/// Writes the hash file of `file`, only if none exists. The content goes to
/// a temporary file in the same folder, is flushed to disk, then renamed
/// without replacement: after a crash or a full disk there is either a
/// complete hash file or none, never a truncated one.
pub fn write_new(file: &Path, digest: &Digest) -> Result<PathBuf, Problem> {
    publish(file, digest, false)
}

/// Replaces the hash file of `file` (or writes it): only for a new version
/// the user accepted. Same atomic publication: the old content stays until
/// the new one is complete.
pub fn replace(file: &Path, digest: &Digest) -> Result<PathBuf, Problem> {
    publish(file, digest, true)
}

fn publish(file: &Path, digest: &Digest, replace: bool) -> Result<PathBuf, Problem> {
    let name = file_name_text(file)?;
    let io_problem = |err: io::Error| Problem::from_io(&err);
    let file = platform::absolute(file).map_err(io_problem)?;
    let target = path_for(&file, digest.algorithm).ok_or_else(|| no_name(&file))?;
    let dir = file.parent().ok_or_else(|| no_name(&file))?;
    // Short temporary name, whatever the length of the source name: a long
    // name plus a suffix could exceed the 255-character limit of a name.
    // Extended paths: `tempfile` calls Windows directly to rename, without
    // the long-path handling the standard library does on its own.
    let mut temp = tempfile::Builder::new()
        .prefix(".fileseal-")
        .suffix(".tmp")
        .tempfile_in(platform::extended(dir))
        .map_err(io_problem)?;
    temp.write_all(format_line(&digest.hex, name).as_bytes())
        .map_err(io_problem)?;
    temp.as_file().sync_all().map_err(io_problem)?;
    faults::check(FaultPoint::SidecarPublish, &target).map_err(io_problem)?;
    // On failure the temporary file is deleted when `err.file` is dropped.
    let published = if replace {
        temp.persist(platform::extended(&target))
    } else {
        temp.persist_noclobber(platform::extended(&target))
    };
    published.map_err(|err| Problem::from_io(&err.error))?;
    Ok(target)
}

/// Remarks about a hash file that was read successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum SidecarNote {
    /// Not UTF-8: read as Windows-1252 (older tools).
    LegacyEncoding,
    /// `MD5: hash  name`, BSD `MD5 (name) = hash`, or a hash without a name.
    NonStandardFormat,
    /// The name inside is not the name of the file next to it (both were
    /// renamed, or the hash file was copied). The hash is still used.
    NameDiffers { written: String },
    /// Same name except for case, on a file system that ignores case.
    NameCaseDiffers { written: String },
}

/// What was found next to a file for one algorithm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidecarLookup {
    Absent,
    Found {
        path: PathBuf,
        digest: Digest,
        notes: Vec<SidecarNote>,
    },
    /// Present but unusable: unreadable, empty, malformed, wrong algorithm.
    Invalid {
        path: PathBuf,
        reason: String,
    },
}

/// Reads the hash file of `file` for `algorithm`, if there is one.
pub fn lookup(file: &Path, algorithm: Algorithm) -> SidecarLookup {
    let Some(path) = path_for(file, algorithm) else {
        return SidecarLookup::Absent;
    };
    let bytes = match read_limited(&path) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return SidecarLookup::Absent,
        Err(reason) => return SidecarLookup::Invalid { path, reason },
    };
    let parsed = parse(&bytes);
    let file_name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match select(&parsed, &file_name, algorithm) {
        Ok((digest, mut notes)) => {
            if parsed.legacy_encoding {
                notes.insert(0, SidecarNote::LegacyEncoding);
            }
            SidecarLookup::Found {
                path,
                digest,
                notes,
            }
        }
        Err(reason) => SidecarLookup::Invalid { path, reason },
    }
}

pub(crate) fn read_limited(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let file = match std::fs::File::open(platform::extended(path)) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("cannot be read: {err}")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("cannot be read: {err}"))?;
    if bytes.len() as u64 > MAX_SIZE {
        return Err("too large to be a hash file".to_owned());
    }
    Ok(Some(bytes))
}

/// One usable line of a hash file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Algorithm named in the line itself (BSD tag, `MD5:` prefix).
    pub tagged: Option<Algorithm>,
    pub hex: String,
    /// Empty for a line holding only a hash.
    pub name: String,
    pub non_standard: bool,
    /// 1-based line number in the hash file.
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    pub entries: Vec<Entry>,
    /// 1-based numbers of lines that are neither entries, comments nor blank.
    pub invalid_lines: Vec<usize>,
    pub legacy_encoding: bool,
}

/// Parses the content of a hash file (any number of lines).
pub fn parse(bytes: &[u8]) -> Parsed {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let (text, legacy_encoding) = match std::str::from_utf8(bytes) {
        Ok(text) => (std::borrow::Cow::Borrowed(text), false),
        Err(_) => (
            encoding_rs::WINDOWS_1252
                .decode_without_bom_handling(bytes)
                .0,
            true,
        ),
    };
    let mut parsed = Parsed {
        legacy_encoding,
        ..Parsed::default()
    };
    // LF, CRLF and lone CR all end a line; names never contain them (GNU
    // escapes them). Normalised first so line numbers stay right.
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    for (index, line) in text.split('\n').enumerate() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_line(line) {
            Some(entry) => parsed.entries.push(Entry {
                line: index + 1,
                ..entry
            }),
            None => parsed.invalid_lines.push(index + 1),
        }
    }
    parsed
}

fn parse_line(line: &str) -> Option<Entry> {
    let (escaped, line) = match line.strip_prefix('\\') {
        Some(rest) => (true, rest),
        None => (false, line),
    };
    let unescape = |name: &str| {
        if escaped {
            unescape(name)
        } else {
            Some(name.to_owned())
        }
    };

    // BSD / `--tag` style: "SHA256 (name) = hash".
    if let Some((tag, rest)) = line.split_once(" (")
        && let Some(algorithm) = tag_algorithm(tag)
        && let Some((name, hex)) = rest.rsplit_once(") = ")
    {
        return Some(Entry {
            tagged: Some(algorithm),
            hex: hex.trim_end().to_owned(),
            name: unescape(name)?,
            non_standard: true,
            line: 0,
        });
    }

    // Older FileSeal: "MD5: hash  name".
    let (tagged, line) = match line.split_once(':') {
        Some((tag, rest)) if tag_algorithm(tag).is_some() => {
            (tag_algorithm(tag), rest.trim_start())
        }
        _ => (None, line),
    };

    let hex_end = line
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(line.len());
    // Lengths of real digests (MD5, SHA-1, SHA-224, SHA-256, SHA-384,
    // SHA-512): "bad line" or "cafe menu" must not pass for a hash.
    if !matches!(hex_end, 32 | 40 | 56 | 64 | 96 | 128) {
        return None;
    }
    let (hex, rest) = line.split_at(hex_end);
    let name = if rest.is_empty() {
        String::new()
    } else {
        // GNU: one space, then ' ' (text) or '*' (binary), then the name.
        // A single space is not GNU, but some tools write it.
        let name = rest
            .strip_prefix("  ")
            .or_else(|| rest.strip_prefix(" *"))
            .or_else(|| rest.strip_prefix(' '))?;
        unescape(name)?
    };
    if name.is_empty() && !rest.is_empty() {
        // "hash  " with nothing after the separator.
        return None;
    }
    Some(Entry {
        tagged,
        hex: hex.to_owned(),
        non_standard: tagged.is_some() || name.is_empty(),
        name,
        line: 0,
    })
}

/// `MD5`, `SHA256`, `SHA-256` (any case) → algorithm.
fn tag_algorithm(tag: &str) -> Option<Algorithm> {
    if tag.is_empty()
        || tag.len() > 10
        || !tag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    Algorithm::from_code(&tag.replace('-', ""))
}

fn unescape(name: &str) -> Option<String> {
    let mut out = String::with_capacity(name.len());
    let mut chars = name.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            _ => return None,
        }
    }
    Some(out)
}

/// Picks the line describing `file_name` in a file made for one algorithm.
fn select(
    parsed: &Parsed,
    file_name: &str,
    algorithm: Algorithm,
) -> Result<(Digest, Vec<SidecarNote>), String> {
    if parsed.entries.is_empty() {
        return Err(if parsed.invalid_lines.is_empty() {
            "empty".to_owned()
        } else {
            format!(
                "no valid line (line {} is malformed)",
                parsed.invalid_lines[0]
            )
        });
    }
    let bare = |name: &str| -> String {
        let name = name
            .strip_prefix("./")
            .or_else(|| name.strip_prefix(".\\"))
            .unwrap_or(name);
        name.to_owned()
    };
    let mut notes = Vec::new();
    let entry =
        if let Some(entry) = parsed.entries.iter().find(|e| bare(&e.name) == file_name) {
            entry
        } else if let Some(entry) = parsed.entries.iter().find(|e| {
            case_insensitive_fs() && bare(&e.name).to_lowercase() == file_name.to_lowercase()
        }) {
            notes.push(SidecarNote::NameCaseDiffers {
                written: entry.name.clone(),
            });
            entry
        } else if parsed.entries.len() == 1 {
            // A file dedicated to one file: its only line is about that file,
            // even if the name inside is outdated.
            let entry = &parsed.entries[0];
            if !entry.name.is_empty() {
                notes.push(SidecarNote::NameDiffers {
                    written: entry.name.clone(),
                });
            }
            entry
        } else {
            return Err(format!(
                "none of its {} lines is about {file_name}",
                parsed.entries.len()
            ));
        };
    if entry.tagged.is_some_and(|tagged| tagged != algorithm) {
        return Err(format!(
            "the line is tagged {:?} but the file is a .{} file",
            entry.tagged.unwrap(),
            algorithm.code()
        ));
    }
    let digest = Digest::parse(algorithm, &entry.hex).ok_or_else(|| {
        format!(
            "\"{}\" is not a {} hash ({} hexadecimal digits expected)",
            entry.hex,
            algorithm.code(),
            algorithm.hex_len()
        )
    })?;
    if entry.non_standard {
        notes.insert(0, SidecarNote::NonStandardFormat);
    }
    Ok((digest, notes))
}

fn case_insensitive_fs() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

fn file_name_text(file: &Path) -> Result<&str, Problem> {
    let name = file.file_name().ok_or_else(|| no_name(file))?;
    name.to_str().ok_or_else(|| {
        Problem::new(
            ProblemKind::NameNotRepresentable,
            format!("{} is not valid Unicode", file.display()),
        )
    })
}

fn no_name(file: &Path) -> Problem {
    Problem::new(
        ProblemKind::NotAFile,
        format!("{} has no file name", file.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const MD5: &str = "d41d8cd98f00b204e9800998ecf8427e";

    fn digest(algorithm: Algorithm, hex: &str) -> Digest {
        Digest::parse(algorithm, hex).unwrap()
    }

    fn found(lookup: SidecarLookup) -> (Digest, Vec<SidecarNote>) {
        match lookup {
            SidecarLookup::Found { digest, notes, .. } => (digest, notes),
            other => panic!("expected a hash, got {other:?}"),
        }
    }

    #[test]
    fn writes_gnu_lines() {
        assert_eq!(
            format_line(SHA, "Blade Runner.mkv"),
            format!("{SHA}  Blade Runner.mkv\n")
        );
        assert_eq!(
            format_line(MD5, "a\\b\nc"),
            format!("\\{MD5}  a\\\\b\\nc\n"),
            "escaped like coreutils"
        );
        assert_eq!(
            path_for(Path::new("dir/Blade Runner.mkv"), Algorithm::Sha256).unwrap(),
            Path::new("dir/Blade Runner.mkv.sha256")
        );
    }

    #[test]
    fn writes_once_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Été à Noël — 1.mkv");
        std::fs::write(&file, b"").unwrap();
        let written = write_new(&file, &digest(Algorithm::Sha256, SHA)).unwrap();
        assert_eq!(
            std::fs::read(&written).unwrap(),
            format!("{SHA}  Été à Noël — 1.mkv\n").as_bytes(),
            "UTF-8 without BOM, LF"
        );
        let again = write_new(&file, &digest(Algorithm::Sha256, MD5.repeat(2).as_str()));
        assert_eq!(again.unwrap_err().kind, ProblemKind::AlreadyExists);
        assert_eq!(
            std::fs::read(&written).unwrap(),
            format!("{SHA}  Été à Noël — 1.mkv\n").as_bytes(),
            "existing hash file untouched"
        );
        assert_eq!(
            names_in(dir.path()),
            ["Été à Noël — 1.mkv", "Été à Noël — 1.mkv.sha256"]
        );
    }

    #[test]
    fn a_failed_publication_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.mkv");
        std::fs::write(&file, b"").unwrap();
        faults::arm(FaultPoint::SidecarPublish);
        let result = write_new(&file, &digest(Algorithm::Sha256, SHA));
        faults::clear();
        assert!(result.is_err());
        assert_eq!(
            names_in(dir.path()),
            ["a.mkv"],
            "no hash file, no temporary file"
        );
    }

    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn reads_what_other_tools_write() {
        let name = "Blade Runner.mkv";
        let upper = SHA.to_uppercase();
        for content in [
            format!("{SHA}  {name}\n"),
            format!("{SHA} *{name}\r\n"),
            format!("\u{feff}{upper}  {name}"),
            format!("# made by hand\n\n{SHA}  {name}\n"),
            format!("SHA256 ({name}) = {SHA}\n"),
            format!("SHA256: {SHA}  {name}\r\n"),
            format!("{SHA} {name}"),
            format!("{SHA}  ./{name}\n"),
        ] {
            let parsed = parse(content.as_bytes());
            let (digest, _) = select(&parsed, name, Algorithm::Sha256)
                .unwrap_or_else(|e| panic!("{content:?}: {e}"));
            assert_eq!(digest.hex, SHA, "{content:?}");
        }
    }

    #[test]
    fn reads_older_fileseal_files_in_cp1252() {
        // "MD5: <hash>  Amélie.mkv" written in Windows-1252 with CRLF.
        let mut bytes = format!("MD5: {MD5}  Am").into_bytes();
        bytes.push(0xE9);
        bytes.extend_from_slice(b"lie.mkv\r\n");
        let parsed = parse(&bytes);
        assert!(parsed.legacy_encoding);
        let (digest, notes) = select(&parsed, "Amélie.mkv", Algorithm::Md5).unwrap();
        assert_eq!(digest.hex, MD5);
        assert_eq!(notes, [SidecarNote::NonStandardFormat]);
    }

    #[test]
    fn line_numbers_are_right_whatever_the_line_endings() {
        for newline in ["\n", "\r\n", "\r"] {
            let content = [
                "# comment",
                &format!("{SHA}  a"),
                "bad line",
                "",
                &format!("{SHA}  b"),
            ]
            .join(newline);
            let parsed = parse(content.as_bytes());
            let lines: Vec<usize> = parsed.entries.iter().map(|e| e.line).collect();
            assert_eq!(lines, [2, 5], "{newline:?}");
            assert_eq!(parsed.invalid_lines, [3], "{newline:?}");
        }
    }

    #[test]
    fn unescapes_gnu_names() {
        let parsed = parse(format!("\\{MD5}  a\\\\b\\nc\n").as_bytes());
        assert_eq!(parsed.entries[0].name, "a\\b\nc");
        assert_eq!(
            parse(format!("\\{MD5}  bad\\x\n").as_bytes()).invalid_lines,
            [1]
        );
    }

    #[test]
    fn rejects_what_is_not_a_hash_of_this_algorithm() {
        let name = "a.mkv";
        for (content, why) in [
            (String::new(), "empty"),
            ("not a hash file\n".to_owned(), "malformed"),
            (format!("{MD5}  {name}\n"), "MD5 length in a .sha256"),
            (format!("MD5 ({name}) = {MD5}\n"), "tagged MD5"),
            (
                format!("{SHA}  b.mkv\n{SHA}  c.mkv\n"),
                "several lines, none about a.mkv",
            ),
        ] {
            let parsed = parse(content.as_bytes());
            assert!(select(&parsed, name, Algorithm::Sha256).is_err(), "{why}");
        }
    }

    #[test]
    fn notes_names_that_differ() {
        let parsed = parse(format!("{SHA}  old name.mkv\n").as_bytes());
        let (_, notes) = select(&parsed, "new name.mkv", Algorithm::Sha256).unwrap();
        assert_eq!(
            notes,
            [SidecarNote::NameDiffers {
                written: "old name.mkv".to_owned()
            }]
        );
        let parsed = parse(format!("{SHA}  hash only\n{SHA}\n").as_bytes());
        assert_eq!(parsed.entries.len(), 2);
        let (_, notes) = select(&parse(SHA.as_bytes()), "x.mkv", Algorithm::Sha256).unwrap();
        assert_eq!(
            notes,
            [SidecarNote::NonStandardFormat],
            "a hash without a name"
        );
    }

    #[test]
    fn lookup_next_to_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.mkv");
        std::fs::write(&file, b"").unwrap();
        assert_eq!(lookup(&file, Algorithm::Md5), SidecarLookup::Absent);
        std::fs::write(dir.path().join("a.mkv.md5"), format!("{MD5}  a.mkv\n")).unwrap();
        assert_eq!(found(lookup(&file, Algorithm::Md5)).0.hex, MD5);
        std::fs::write(dir.path().join("a.mkv.sha256"), "garbage").unwrap();
        assert!(matches!(
            lookup(&file, Algorithm::Sha256),
            SidecarLookup::Invalid { .. }
        ));
    }
}
