//! Verifying from a hash file the user opened, the way `sha256sum -c` does:
//! each line names a file (relative to the hash file's folder) and its
//! expected hash. No database is needed; one may be consulted as well.
//!
//! Two phases: `plan` reads the hash file and resolves its lines without
//! hashing anything (what will be verified, how much there is to read),
//! then `verify` checks each file in order.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use super::{
    DbTarget, SourceKind, SourceOutcome, SourceResult, VerifyOptions, VerifyReport, VerifyStatus,
    verify_against,
};
use crate::algorithm::{Algorithm, Digest};
use crate::batch::{BatchFile, ListedReference};
use crate::platform;
use crate::problem::{Problem, ProblemKind};
use crate::progress::ProgressReporter;
use crate::reader::Control;
use crate::sidecar::{self, Entry, SidecarNote};

/// A hash file read and its lines resolved, before anything is hashed.
#[derive(Debug, Clone)]
pub struct ManifestPlan {
    pub path: PathBuf,
    /// Algorithm of the file as a whole (lines may name their own).
    pub algorithm: Option<Algorithm>,
    pub legacy_encoding: bool,
    pub invalid_lines: Vec<usize>,
    pub entries: Vec<PlannedEntry>,
}

#[derive(Debug, Clone)]
pub struct PlannedEntry {
    pub line: usize,
    /// The file to verify; `None` when the line names no usable file.
    pub target: Option<PathBuf>,
    /// The expected digest, or why the line cannot be used.
    pub expected: Result<Digest, String>,
    pub notes: Vec<SidecarNote>,
    /// `None` when the file does not exist.
    pub size: Option<u64>,
}

impl ManifestPlan {
    pub fn files(&self) -> u32 {
        self.entries.len() as u32
    }

    /// Bytes to read: the files that exist and have a usable line.
    pub fn total_bytes(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.expected.is_ok())
            .filter_map(|e| e.size)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManifestReport {
    pub path: String,
    pub algorithm: Option<Algorithm>,
    pub legacy_encoding: bool,
    /// Lines that are neither entries, comments nor blank.
    pub invalid_lines: Vec<u32>,
    /// One result per usable or unusable entry, in the file's order.
    pub entries: Vec<VerifyReport>,
}

/// The files a hash file lists, ready to be verified in a batch, and the
/// lines that cannot be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListedFiles {
    pub path: String,
    pub algorithm: Option<Algorithm>,
    pub legacy_encoding: bool,
    pub files: Vec<BatchFile>,
    pub problems: Vec<ListProblem>,
}

/// A line of a hash file that names no usable file or hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListProblem {
    pub line: u32,
    /// The file the line names, if it names one.
    pub path: Option<String>,
    pub reason: String,
}

/// Reads a hash file into the list of files it names.
pub fn list(manifest: &Path) -> Result<ListedFiles, Problem> {
    let plan = plan(manifest)?;
    let list = plan.path.display().to_string();
    let mut files = Vec::new();
    let mut problems: Vec<ListProblem> = plan
        .invalid_lines
        .iter()
        .map(|&line| ListProblem {
            line: line as u32,
            path: None,
            reason: "neither a hash nor a comment".to_owned(),
        })
        .collect();
    for entry in plan.entries {
        let line = entry.line as u32;
        match (entry.target, entry.expected) {
            (Some(target), Ok(digest)) => files.push(BatchFile {
                path: target.display().to_string(),
                size: entry.size,
                listed: Some(ListedReference {
                    list: list.clone(),
                    line,
                    digest,
                    notes: entry.notes,
                }),
            }),
            (target, Err(reason)) => problems.push(ListProblem {
                line,
                path: target.map(|t| t.display().to_string()),
                reason,
            }),
            (None, Ok(_)) => problems.push(ListProblem {
                line,
                path: None,
                reason: "the line names no file".to_owned(),
            }),
        }
    }
    problems.sort_by_key(|p| p.line);
    Ok(ListedFiles {
        path: list,
        algorithm: plan.algorithm,
        legacy_encoding: plan.legacy_encoding,
        files,
        problems,
    })
}

/// Reads a hash file and resolves its lines.
pub fn plan(manifest: &Path) -> Result<ManifestPlan, Problem> {
    let manifest = platform::absolute(manifest).map_err(|err| Problem::from_io(&err))?;
    let bytes = match sidecar::read_limited(&manifest) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => {
            return Err(Problem::new(
                ProblemKind::NotFound,
                format!("{} does not exist", manifest.display()),
            ));
        }
        Err(reason) => return Err(Problem::new(ProblemKind::InvalidHashFile, reason)),
    };
    let parsed = sidecar::parse(&bytes);
    if parsed.entries.is_empty() {
        return Err(Problem::new(
            ProblemKind::InvalidHashFile,
            match parsed.invalid_lines.first() {
                Some(line) => format!("no valid line (line {line} is malformed)"),
                None => "no hash in this file".to_owned(),
            },
        ));
    }
    let named = algorithm_of_name(&manifest);
    let algorithm = named.or_else(|| algorithm_of_lines(&parsed.entries));
    let dir = manifest.parent().unwrap_or(Path::new("")).to_path_buf();
    // `movie.mkv.sha256` is made for `movie.mkv`.
    let dedicated = named.map(|_| manifest.with_extension(""));
    let single = parsed.entries.len() == 1;

    let entries = parsed
        .entries
        .iter()
        .map(|entry| {
            let mut notes = Vec::new();
            if parsed.legacy_encoding {
                notes.push(SidecarNote::LegacyEncoding);
            }
            if entry.non_standard {
                notes.push(SidecarNote::NonStandardFormat);
            }
            let target = resolve(entry, &dir, dedicated.as_deref(), single, &mut notes);
            let size = target
                .as_deref()
                .and_then(|t| std::fs::metadata(t).ok())
                .filter(|m| m.is_file())
                .map(|m| m.len());
            PlannedEntry {
                line: entry.line,
                expected: expected(entry, named, algorithm),
                target,
                notes,
                size,
            }
        })
        .collect();
    Ok(ManifestPlan {
        path: manifest,
        algorithm,
        legacy_encoding: parsed.legacy_encoding,
        invalid_lines: parsed.invalid_lines,
        entries,
    })
}

/// The file a line is about. The name is relative to the hash file's
/// folder (as `sha256sum -c` reads it); a dedicated hash file whose only
/// line names a file that no longer exists is taken to be about the file
/// it is named after (both were renamed together).
fn resolve(
    entry: &Entry,
    dir: &Path,
    dedicated: Option<&Path>,
    single: bool,
    notes: &mut Vec<SidecarNote>,
) -> Option<PathBuf> {
    if entry.name.is_empty() {
        // A lone hash only makes sense in a file made for one file.
        return dedicated.filter(|_| single).map(Path::to_path_buf);
    }
    let named = dir.join(&entry.name);
    if named.exists() {
        return Some(named);
    }
    if let Some(dedicated) = dedicated.filter(|d| single && d.exists()) {
        notes.push(SidecarNote::NameDiffers {
            written: entry.name.clone(),
        });
        return Some(dedicated.to_path_buf());
    }
    Some(named)
}

fn expected(
    entry: &Entry,
    named: Option<Algorithm>,
    algorithm: Option<Algorithm>,
) -> Result<Digest, String> {
    if let (Some(tagged), Some(named)) = (entry.tagged, named)
        && tagged != named
    {
        return Err(format!(
            "line tagged {} in a .{} file",
            tagged.code(),
            named.code()
        ));
    }
    let algorithm = entry
        .tagged
        .or(algorithm)
        .ok_or_else(|| "cannot tell which algorithm this hash is".to_owned())?;
    Digest::parse(algorithm, &entry.hex).ok_or_else(|| {
        format!(
            "\"{}\" is not a {} hash ({} hexadecimal digits expected)",
            entry.hex,
            algorithm.code(),
            algorithm.hex_len()
        )
    })
}

/// A hash file FileSeal can verify from, judging by its name: `.sha256`,
/// `.md5`, `SHA256SUMS`, `md5sums.txt`…
pub fn is_hash_file(path: &Path) -> bool {
    algorithm_of_name(path).is_some()
}

/// From the name: `.sha256` / `.md5` extension, else a name such as
/// `SHA256SUMS` or `md5sums.txt`.
fn algorithm_of_name(path: &Path) -> Option<Algorithm> {
    if let Some(algorithm) = path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(Algorithm::from_code)
    {
        return Some(algorithm);
    }
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    Algorithm::ALL
        .into_iter()
        .find(|a| name.contains(&format!("{}sum", a.code())))
}

/// From the lines: their tags, else the length of their hashes, if all
/// lines agree.
fn algorithm_of_lines(entries: &[Entry]) -> Option<Algorithm> {
    let candidates: Vec<Option<Algorithm>> = entries
        .iter()
        .map(|e| {
            e.tagged.or_else(|| {
                Algorithm::ALL
                    .into_iter()
                    .find(|a| a.hex_len() == e.hex.len())
            })
        })
        .collect();
    let first = (*candidates.first()?)?;
    candidates
        .iter()
        .all(|c| *c == Some(first))
        .then_some(first)
}

/// Verifies every file of the plan in order. After a cancellation, the
/// files not reached are "not processed".
pub fn verify(
    plan: &ManifestPlan,
    db: Option<DbTarget>,
    control: &Control,
    reporter: &mut ProgressReporter,
) -> ManifestReport {
    let manifest = plan.path.display().to_string();
    let mut entries = Vec::with_capacity(plan.entries.len());
    let mut stopped = false;
    for entry in &plan.entries {
        // A file not started when the user cancelled was not processed;
        // only the one being read is "cancelled".
        stopped |= control.is_cancelled();
        let source = SourceResult {
            source: SourceKind::Sidecar,
            path: manifest.clone(),
            expected: entry.expected.iter().cloned().collect(),
            expected_size: None,
            reference_created_at: None,
            outcome: match entry.expected {
                Ok(_) => SourceOutcome::NotCompared,
                Err(_) => SourceOutcome::Invalid,
            },
            detail: entry
                .expected
                .as_ref()
                .err()
                .map(|reason| format!("line {}: {reason}", entry.line)),
            notes: entry.notes.clone(),
        };
        let shown = match &entry.target {
            Some(target) => target.display().to_string(),
            None => format!("{manifest} (line {})", entry.line),
        };
        let report = match (&entry.target, &entry.expected) {
            (Some(target), Ok(_)) if !stopped => {
                let report = verify_against(
                    target,
                    &VerifyOptions {
                        use_sidecar: false,
                        follow_moves: false,
                    },
                    vec![source],
                    db,
                    control,
                    &mut |done, total| reporter.file_progress(&shown, done, total),
                );
                reporter.file_finished(entry.size.unwrap_or(0));
                stopped = report.status == VerifyStatus::Cancelled;
                report
            }
            (_, Ok(_)) if stopped => unchecked(shown, VerifyStatus::NotProcessed, source),
            _ => unchecked(shown, VerifyStatus::InvalidHashFile, source),
        };
        entries.push(report);
    }
    tracing::info!(path = %manifest, files = entries.len(), "verify from hash file");
    ManifestReport {
        path: manifest,
        algorithm: plan.algorithm,
        legacy_encoding: plan.legacy_encoding,
        invalid_lines: plan.invalid_lines.iter().map(|&l| l as u32).collect(),
        entries,
    }
}

fn unchecked(path: String, status: VerifyStatus, source: SourceResult) -> VerifyReport {
    VerifyReport {
        path,
        status,
        size: None,
        digests: Vec::new(),
        observed_mtime: None,
        sources: vec![source],
        problem: None,
        record_problem: None,
        item_id: None,
        verification_id: None,
        location_change: None,
        finding: None,
        relocated_from: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::ProgressReporter;

    const SHA_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const MD5_ABC: &str = "900150983cd24fb0d6963f7d28e17f72";

    fn run(manifest: &Path) -> ManifestReport {
        let plan = plan(manifest).unwrap();
        let mut reporter = ProgressReporter::new(plan.files(), plan.total_bytes(), |_| {});
        verify(&plan, None, &Control::new(), &mut reporter)
    }

    fn statuses(report: &ManifestReport) -> Vec<VerifyStatus> {
        report.entries.iter().map(|e| e.status).collect()
    }

    #[test]
    fn verifies_every_listed_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("good.bin"), b"abc").unwrap();
        std::fs::write(dir.path().join("changed.bin"), b"abd").unwrap();
        std::fs::create_dir(dir.path().join("Saison 1")).unwrap();
        std::fs::write(dir.path().join("Saison 1").join("é1.mkv"), b"abc").unwrap();
        let manifest = dir.path().join("SHA256SUMS");
        std::fs::write(
            &manifest,
            format!(
                "{SHA_ABC}  good.bin\n{SHA_ABC}  changed.bin\n{SHA_ABC}  gone.bin\n{SHA_ABC} *Saison 1/é1.mkv\n"
            ),
        )
        .unwrap();
        let report = run(&manifest);
        assert_eq!(
            report.algorithm,
            Some(Algorithm::Sha256),
            "from the name SHA256SUMS"
        );
        assert_eq!(
            statuses(&report),
            [
                VerifyStatus::Match,
                VerifyStatus::Mismatch,
                VerifyStatus::NotFound,
                VerifyStatus::Match
            ]
        );
        assert!(report.entries[2].path.ends_with("gone.bin"));
        assert_eq!(
            report.entries[2].sources[0].expected[0].hex, SHA_ABC,
            "expected shown"
        );
    }

    #[test]
    fn the_algorithm_comes_from_the_name_the_tags_or_the_hashes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"abc").unwrap();
        let by_length = dir.path().join("checksums.txt");
        std::fs::write(&by_length, format!("{MD5_ABC}  a.bin\n")).unwrap();
        let report = run(&by_length);
        assert_eq!(
            (report.algorithm, statuses(&report)[0]),
            (Some(Algorithm::Md5), VerifyStatus::Match)
        );

        let mixed = dir.path().join("hashes.txt");
        std::fs::write(
            &mixed,
            format!("SHA256 (a.bin) = {SHA_ABC}\nMD5 (a.bin) = {MD5_ABC}\n"),
        )
        .unwrap();
        let report = run(&mixed);
        assert_eq!(report.algorithm, None, "no single algorithm");
        assert_eq!(
            statuses(&report),
            [VerifyStatus::Match, VerifyStatus::Match]
        );

        let lying = dir.path().join("a.bin.sha256");
        std::fs::write(&lying, format!("MD5 (a.bin) = {MD5_ABC}\n")).unwrap();
        assert_eq!(statuses(&run(&lying)), [VerifyStatus::InvalidHashFile]);
    }

    #[test]
    fn a_renamed_pair_is_still_verified() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("new name.mkv"), b"abc").unwrap();
        let manifest = dir.path().join("new name.mkv.sha256");
        std::fs::write(&manifest, format!("{SHA_ABC}  old name.mkv\n")).unwrap();
        let report = run(&manifest);
        assert_eq!(statuses(&report), [VerifyStatus::Match]);
        assert!(report.entries[0].path.ends_with("new name.mkv"));
        assert_eq!(
            report.entries[0].sources[0].notes,
            [SidecarNote::NameDiffers {
                written: "old name.mkv".to_owned()
            }]
        );

        let lone = dir.path().join("new name.mkv.md5");
        std::fs::write(&lone, MD5_ABC).unwrap();
        assert_eq!(
            statuses(&run(&lone)),
            [VerifyStatus::Match],
            "a hash without a name"
        );
    }

    #[test]
    fn older_fileseal_files_are_verified() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Amélie.mkv"), b"abc").unwrap();
        let mut bytes = format!("MD5: {MD5_ABC}  Am").into_bytes();
        bytes.push(0xE9); // é in Windows-1252
        bytes.extend_from_slice(b"lie.mkv\r\n");
        let manifest = dir.path().join("Amélie.mkv.md5");
        std::fs::write(&manifest, bytes).unwrap();
        let report = run(&manifest);
        assert!(report.legacy_encoding);
        assert_eq!(statuses(&report), [VerifyStatus::Match]);
    }

    #[test]
    fn bad_lines_are_reported_and_the_others_verified() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"abc").unwrap();
        let manifest = dir.path().join("list.sha256");
        std::fs::write(
            &manifest,
            format!("garbage\n{MD5_ABC}  a.bin\n{SHA_ABC}  a.bin\n{SHA_ABC}\n"),
        )
        .unwrap();
        let report = run(&manifest);
        assert_eq!(report.invalid_lines, [1]);
        assert_eq!(
            statuses(&report),
            [
                VerifyStatus::InvalidHashFile, // MD5 length in a .sha256 file
                VerifyStatus::Match,
                VerifyStatus::InvalidHashFile // no name, several lines
            ]
        );
        assert!(report.entries[2].path.contains("(line 4)"));
        assert_eq!(
            plan(&dir.path().join("missing.sha256")).unwrap_err().kind,
            ProblemKind::NotFound
        );
        let empty = dir.path().join("empty.md5");
        std::fs::write(&empty, "# nothing\n").unwrap();
        assert_eq!(plan(&empty).unwrap_err().kind, ProblemKind::InvalidHashFile);
    }

    #[test]
    fn after_a_cancellation_the_rest_is_not_processed() {
        let dir = tempfile::tempdir().unwrap();
        let mut lines = String::new();
        for name in ["1.bin", "2.bin", "3.bin"] {
            std::fs::write(dir.path().join(name), vec![0u8; 3 << 20]).unwrap();
            lines.push_str(&format!("{SHA_ABC}  {name}\n"));
        }
        let manifest = dir.path().join("SHA256SUMS");
        std::fs::write(&manifest, lines).unwrap();
        let plan = plan(&manifest).unwrap();
        assert_eq!((plan.files(), plan.total_bytes()), (3, 9 << 20));
        // Cancelled as soon as the first file is done.
        let control = Control::new();
        let mut reporter = ProgressReporter::new(plan.files(), plan.total_bytes(), |s| {
            if s.files_done > 0 {
                control.cancel();
            }
        });
        let report = verify(&plan, None, &control, &mut reporter);
        assert_eq!(
            statuses(&report),
            [
                VerifyStatus::Mismatch, // processed (zeros are not "abc")
                VerifyStatus::NotProcessed,
                VerifyStatus::NotProcessed
            ]
        );
        assert!(report.entries[1].digests.is_empty(), "not read");
    }
}
