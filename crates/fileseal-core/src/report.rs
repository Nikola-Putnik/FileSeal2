//! Reports of operations, exported as CSV: one line per file, with stable
//! column names and status codes (docs/rapport-csv.md). UTF-8 with a BOM
//! so Excel reads accents right; the separator is chosen by the caller
//! (`,` for English Excel, `;` for French Excel).

use std::io::Write;
use std::path::Path;

use serde::Serialize;

use crate::algorithm::{self, Algorithm, Digest};
use crate::ops::content::ContentFinding;
use crate::ops::search::SearchReport;
use crate::ops::{CreateReport, SourceKind, VerifyReport};
use crate::platform;
use crate::problem::Problem;

/// One file of an operation, as reported to the user.
#[derive(Debug, Clone)]
pub enum ReportEntry {
    Create(CreateReport),
    Verify(VerifyReport),
    Search(SearchReport),
    /// A file of a batch that was not read (not processed, duplicate).
    Skipped {
        action: &'static str,
        path: String,
        status: String,
        detail: String,
    },
}

pub const COLUMNS: [&str; 9] = [
    "action",
    "path",
    "status",
    "size",
    "sha256",
    "md5",
    "expected_sha256",
    "expected_md5",
    "detail",
];

/// The CSV text, header included, without the BOM.
pub fn to_csv(entries: &[ReportEntry], separator: char) -> String {
    let mut csv = String::new();
    push_line(&mut csv, COLUMNS.iter().map(|c| c.to_string()), separator);
    for entry in entries {
        push_line(&mut csv, row(entry).into_iter(), separator);
    }
    csv
}

/// Writes the report atomically (never half a file), with a UTF-8 BOM.
pub fn write_csv(path: &Path, entries: &[ReportEntry], separator: char) -> Result<(), Problem> {
    let io_problem = |err: std::io::Error| Problem::from_io(&err);
    let path = platform::absolute(path).map_err(io_problem)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut temp = tempfile::Builder::new()
        .prefix(".fileseal-")
        .suffix(".tmp")
        .tempfile_in(platform::extended(dir))
        .map_err(io_problem)?;
    temp.write_all("\u{feff}".as_bytes()).map_err(io_problem)?;
    temp.write_all(to_csv(entries, separator).as_bytes())
        .map_err(io_problem)?;
    temp.as_file().sync_all().map_err(io_problem)?;
    temp.persist(platform::extended(&path))
        .map_err(|err| Problem::from_io(&err.error))?;
    Ok(())
}

fn row(entry: &ReportEntry) -> Vec<String> {
    let digest = |digests: &[Digest], algorithm| {
        algorithm::find(digests, algorithm)
            .map(|d| d.hex.clone())
            .unwrap_or_default()
    };
    let size = |size: Option<u64>| size.map(|s| s.to_string()).unwrap_or_default();
    let (action, path, status, size, digests, expected, detail) = match entry {
        ReportEntry::Skipped {
            action,
            path,
            status,
            detail,
        } => (
            *action,
            path,
            status.clone(),
            String::new(),
            Vec::new(),
            Vec::new(),
            vec![detail.clone()],
        ),
        ReportEntry::Create(report) => {
            let expected = report
                .database
                .as_ref()
                .map(|d| d.existing.clone())
                .unwrap_or_default();
            let mut details = Vec::new();
            for sidecar in &report.sidecars {
                details.push(format!(
                    "{} hash file: {}",
                    sidecar.algorithm.code(),
                    code(&sidecar.outcome)
                ));
            }
            if let Some(database) = &report.database {
                details.push(format!("database: {}", code(&database.outcome)));
            }
            details.extend(report.problem.iter().map(ToString::to_string));
            details.extend(followed(report.relocated_from.as_ref()));
            details.extend(finding_detail(report.finding.as_ref()));
            (
                "create",
                &report.path,
                code(&report.status),
                size(report.size),
                report.digests.clone(),
                expected,
                details,
            )
        }
        ReportEntry::Verify(report) => {
            // The database's expectation first, then the hash files'.
            let mut expected: Vec<Digest> = Vec::new();
            let mut sources: Vec<_> = report.sources.iter().collect();
            sources.sort_by_key(|s| s.source != SourceKind::Database);
            for source in sources {
                for digest in &source.expected {
                    if algorithm::find(&expected, digest.algorithm).is_none() {
                        expected.push(digest.clone());
                    }
                }
            }
            let mut details: Vec<String> = report
                .sources
                .iter()
                .filter_map(|s| s.detail.clone())
                .collect();
            details.extend(report.problem.iter().map(ToString::to_string));
            details.extend(
                report
                    .location_change
                    .iter()
                    .map(|from| format!("recorded at {from}")),
            );
            details.extend(followed(report.relocated_from.as_ref()));
            details.extend(finding_detail(report.finding.as_ref()));
            (
                "verify",
                &report.path,
                code(&report.status),
                size(report.size),
                report.digests.clone(),
                expected,
                details,
            )
        }
        ReportEntry::Search(report) => {
            let mut details: Vec<String> = report.problem.iter().map(ToString::to_string).collect();
            details.extend(
                report
                    .location_change
                    .iter()
                    .map(|from| format!("recorded at {from}")),
            );
            details.extend(finding_detail(report.finding.as_ref()));
            (
                "search",
                &report.path,
                code(&report.status),
                size(report.size),
                report.digests.clone(),
                Vec::new(),
                details,
            )
        }
    };
    vec![
        action.to_owned(),
        path.clone(),
        status,
        size,
        digest(&digests, Algorithm::Sha256),
        digest(&digests, Algorithm::Md5),
        digest(&expected, Algorithm::Sha256),
        digest(&expected, Algorithm::Md5),
        detail.join(" | "),
    ]
}

/// A location followed automatically, from where it was recorded.
fn followed(from: Option<&String>) -> Option<String> {
    from.map(|from| format!("location followed from {from}"))
}

fn finding_detail(finding: Option<&ContentFinding>) -> Vec<String> {
    let Some(finding) = finding else {
        return Vec::new();
    };
    let mut details = vec![format!("content: {}", code(&finding.outcome))];
    details.extend(
        finding
            .same_content
            .iter()
            .map(|c| format!("same content: {} ({})", c.path, code(&c.presence))),
    );
    details.extend(
        finding
            .hints
            .iter()
            .map(|c| format!("same name: {}", c.path)),
    );
    details
}

/// The serialised name of a status (`match`, `notFound`…): the same codes
/// as the interface and the database history.
fn code<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// RFC 4180: quoted when needed, quotes doubled.
fn push_line(csv: &mut String, fields: impl Iterator<Item = String>, separator: char) {
    for (i, field) in fields.enumerate() {
        if i > 0 {
            csv.push(separator);
        }
        if field.contains([separator, '"', '\n', '\r']) {
            csv.push('"');
            csv.push_str(&field.replace('"', "\"\""));
            csv.push('"');
        } else {
            csv.push_str(&field);
        }
    }
    csv.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{SourceOutcome, SourceResult, VerifyStatus};

    fn verify(path: &str, status: VerifyStatus) -> VerifyReport {
        VerifyReport {
            path: path.to_owned(),
            status,
            size: Some(3),
            digests: vec![Digest {
                algorithm: Algorithm::Sha256,
                hex: "a".repeat(64),
            }],
            observed_mtime: None,
            sources: vec![SourceResult {
                source: SourceKind::Database,
                path: path.to_owned(),
                expected: vec![Digest {
                    algorithm: Algorithm::Sha256,
                    hex: "b".repeat(64),
                }],
                expected_size: Some(3),
                reference_created_at: None,
                outcome: SourceOutcome::Mismatch,
                detail: None,
                notes: Vec::new(),
            }],
            problem: None,
            record_problem: None,
            item_id: Some(1),
            verification_id: Some(1),
            location_change: None,
            finding: None,
            relocated_from: None,
        }
    }

    #[test]
    fn one_line_per_file_with_documented_columns() {
        let csv = to_csv(
            &[ReportEntry::Verify(verify(
                r"C:\Films\a.mkv",
                VerifyStatus::Mismatch,
            ))],
            ';',
        );
        let lines: Vec<&str> = csv.split("\r\n").collect();
        assert_eq!(
            lines[0],
            "action;path;status;size;sha256;md5;expected_sha256;expected_md5;detail"
        );
        assert_eq!(
            lines[1],
            format!(
                r"verify;C:\Films\a.mkv;mismatch;3;{};;{};;",
                "a".repeat(64),
                "b".repeat(64)
            )
        );
    }

    #[test]
    fn fields_are_quoted_when_needed() {
        let csv = to_csv(
            &[ReportEntry::Verify(verify(
                "C:\\Films\\Le \"film\"; vol, 2.mkv",
                VerifyStatus::Match,
            ))],
            ';',
        );
        assert!(csv.contains("verify;\"C:\\Films\\Le \"\"film\"\"; vol, 2.mkv\";match;"));
        let comma = to_csv(
            &[ReportEntry::Verify(verify(
                r"C:\a, b.mkv",
                VerifyStatus::Match,
            ))],
            ',',
        );
        assert!(comma.contains(r#"verify,"C:\a, b.mkv",match,"#));
    }

    #[test]
    fn written_with_a_bom_and_never_half() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.csv");
        let entries = [ReportEntry::Verify(verify(
            r"C:\é.mkv",
            VerifyStatus::Match,
        ))];
        write_csv(&path, &entries, ',').unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"\xEF\xBB\xBF"));
        assert!(String::from_utf8(bytes).unwrap().contains("é.mkv"));
        // Written again over the old one.
        write_csv(&path, &[], ',').unwrap();
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no temporary left"
        );
    }
}
