//! Hash files written by FileSeal must be accepted by an independent tool:
//! GNU `sha256sum -c` / `md5sum -c` (coreutils shipped with Git for Windows,
//! or the system's on Linux/macOS). Skipped with a message if not installed.

use std::path::{Path, PathBuf};
use std::process::Command;

use fileseal_core::Algorithm;
use fileseal_core::ops::{CreateOptions, CreateStatus, create_reference};
use fileseal_core::reader::Control;

fn gnu_tool(name: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(format!(r"C:\Program Files\Git\usr\bin\{name}.exe")),
        PathBuf::from(format!("/usr/bin/{name}")),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Names that need care: spaces, accents, a leading dash, no extension.
const NAMES: &[&str] = &[
    "Blade Runner (1982) [Final Cut].mkv",
    "Amélie — Le fabuleux destin.mkv",
    "-starts with a dash.txt",
    "no extension",
    "日本語のファイル.bin",
];

fn check_with(tool: &Path, algorithm: Algorithm) {
    let dir = tempfile::tempdir().unwrap();
    for (i, name) in NAMES.iter().enumerate() {
        let file = dir.path().join(name);
        std::fs::write(&file, format!("content {i}")).unwrap();
        let report = create_reference(
            &file,
            &CreateOptions {
                algorithms: vec![algorithm],
                write_sidecar: true,
                new_item: false,
                ..Default::default()
            },
            None,
            &Control::new(),
            &mut |_, _| {},
        );
        assert_eq!(report.status, CreateStatus::Created, "{report:#?}");
        let sidecar = format!("{name}.{}", algorithm.code());
        // `--` so that a hash file whose name starts with '-' is not an option.
        let output = Command::new(tool)
            .current_dir(dir.path())
            .args(["--strict", "-c", "--", &sidecar])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{} rejected {sidecar}: {stdout} {}",
            tool.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(stdout.trim_end().ends_with(": OK"), "{stdout}");
    }
}

#[test]
fn sha256sum_accepts_our_hash_files() {
    match gnu_tool("sha256sum") {
        Some(tool) => check_with(&tool, Algorithm::Sha256),
        None => eprintln!("sha256sum not found: compatibility test skipped"),
    }
}

#[test]
fn md5sum_accepts_our_hash_files() {
    match gnu_tool("md5sum") {
        Some(tool) => check_with(&tool, Algorithm::Md5),
        None => eprintln!("md5sum not found: compatibility test skipped"),
    }
}

#[test]
fn we_read_what_sha256sum_writes() {
    let Some(tool) = gnu_tool("sha256sum") else {
        eprintln!("sha256sum not found: test skipped");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let name = "Amélie — Le fabuleux destin.mkv";
    std::fs::write(dir.path().join(name), b"abc").unwrap();
    let output = Command::new(tool)
        .current_dir(dir.path())
        .args(["--", name])
        .output()
        .unwrap();
    assert!(output.status.success());
    // Git's coreutils writes "hash *name" (binary mode); GNU on Linux "hash  name".
    std::fs::write(dir.path().join(format!("{name}.sha256")), &output.stdout).unwrap();
    let report = fileseal_core::ops::verify_file(
        &dir.path().join(name),
        &fileseal_core::ops::VerifyOptions { use_sidecar: true },
        None,
        &Control::new(),
        &mut |_, _| {},
    );
    assert_eq!(
        report.status,
        fileseal_core::ops::VerifyStatus::Match,
        "{report:#?}\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        report.sources[0].notes.is_empty(),
        "same name, standard format"
    );
}

#[test]
fn we_verify_a_list_written_by_sha256sum() {
    let Some(tool) = gnu_tool("sha256sum") else {
        eprintln!("sha256sum not found: test skipped");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Saison 1")).unwrap();
    let names = ["a.bin", "Saison 1/Épisode 1.mkv", "-dash.txt"];
    for (i, name) in names.iter().enumerate() {
        std::fs::write(dir.path().join(name), format!("content {i}")).unwrap();
    }
    let output = Command::new(tool)
        .current_dir(dir.path())
        .arg("--")
        .args(names)
        .output()
        .unwrap();
    assert!(output.status.success());
    let manifest = dir.path().join("SHA256SUMS");
    std::fs::write(&manifest, &output.stdout).unwrap();

    use fileseal_core::ops::manifest;
    use fileseal_core::progress::ProgressReporter;
    let plan = manifest::plan(&manifest).unwrap();
    let mut reporter = ProgressReporter::new(plan.files(), plan.total_bytes(), |_| {});
    let report = manifest::verify(&plan, None, &Control::new(), &mut reporter);
    let statuses: Vec<_> = report.entries.iter().map(|e| e.status).collect();
    assert_eq!(
        statuses,
        [fileseal_core::ops::VerifyStatus::Match; 3],
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
