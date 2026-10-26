//! Development and test command line: runs the same actions as the
//! application, without any interface. Not a public tool yet (plan,
//! evolution 7): output and options may change.
//!
//! Files and folders are turned into a list by the selection rules
//! (`discovery`), then processed as one batch (`batch`): one reader per
//! volume, several volumes at once.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Args, Parser, Subcommand, ValueEnum};
use fileseal_core::batch::{
    self, BatchAction, BatchEntry, BatchFile, BatchOptions, BatchSummary, EntryOutcome, Skip,
};
use fileseal_core::discovery::{self, DiscoveryOptions};
use fileseal_core::ops::content::ContentFinding;
use fileseal_core::ops::manifest;
use fileseal_core::ops::search::SearchReport;
use fileseal_core::ops::{
    CreateOptions, CreateReport, CreateStatus, ExistingReferences, VerifyOptions, VerifyReport,
};
use fileseal_core::{Algorithm, Control, Store};

#[derive(Parser)]
#[command(
    name = "fileseal-cli",
    version,
    about = "FileSeal: create and verify file references"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a reference database (`.fileseal` file).
    InitDb {
        path: PathBuf,
        #[arg(long)]
        name: String,
    },
    /// Create references for files and the files of folders.
    Create {
        #[command(flatten)]
        selection: Selection,
        /// Database to record references in.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Do not write hash files next to the files.
        #[arg(long)]
        no_sidecar: bool,
        /// Add the files to their folder's hash list (SHA256SUMS, MD5SUMS).
        #[arg(long)]
        sums: bool,
        #[arg(long = "algo", value_enum, default_values_t = [Algo::Sha256])]
        algorithms: Vec<Algo>,
        /// Create a new item even if the content looks like a known file
        /// that moved here.
        #[arg(long)]
        new_item: bool,
        /// Files that already have references: read them again, or skip
        /// them without reading.
        #[arg(long, value_enum, default_value_t = Existing::Read)]
        existing: Existing,
        /// Record the hash files of skipped files in the database, without
        /// reading the files.
        #[arg(long)]
        import_hash_files: bool,
        /// Attach a file found elsewhere than recorded to its item without
        /// asking: the database keeps its last known location.
        #[arg(long)]
        follow_moves: bool,
    },
    /// Verify files against their references.
    Verify {
        #[command(flatten)]
        selection: Selection,
        #[arg(long)]
        db: Option<PathBuf>,
        /// Ignore hash files next to the files.
        #[arg(long)]
        no_sidecar: bool,
        /// Attach a file found elsewhere than recorded to its item without
        /// asking: the database keeps its last known location.
        #[arg(long)]
        follow_moves: bool,
    },
    /// Look files up by content in a database (writes nothing).
    Search {
        #[command(flatten)]
        selection: Selection,
        #[arg(long)]
        db: PathBuf,
    },
    /// List the files that files and folders stand for, with what is left
    /// out and why (writes nothing).
    Scan {
        #[command(flatten)]
        selection: Selection,
        /// Print the totals only.
        #[arg(long)]
        quiet: bool,
    },
}

#[derive(Args)]
struct Selection {
    /// Files and folders.
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Do not look into subfolders.
    #[arg(long)]
    flat: bool,
    /// Keep hidden and system files found in folders.
    #[arg(long)]
    hidden: bool,
    /// Keep only this extension (repeatable; "" for files without one).
    #[arg(long = "ext")]
    extensions: Vec<String>,
    /// Volumes read at the same time (1 to 4).
    #[arg(long, default_value_t = 2)]
    parallel: u8,
}

impl Selection {
    fn options(&self) -> DiscoveryOptions {
        DiscoveryOptions {
            recursive: !self.flat,
            include_hidden: self.hidden,
            extensions: self.extensions.clone(),
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Algo {
    Sha256,
    Md5,
}

#[derive(Clone, Copy, ValueEnum)]
enum Existing {
    /// Check them and complete them.
    Read,
    /// Skip those with every algorithm asked.
    SkipComplete,
    /// Skip those with any reference.
    SkipAny,
}

impl From<Existing> for ExistingReferences {
    fn from(existing: Existing) -> ExistingReferences {
        match existing {
            Existing::Read => ExistingReferences::Read,
            Existing::SkipComplete => ExistingReferences::SkipComplete,
            Existing::SkipAny => ExistingReferences::SkipAny,
        }
    }
}

impl From<Algo> for Algorithm {
    fn from(algo: Algo) -> Algorithm {
        match algo {
            Algo::Sha256 => Algorithm::Sha256,
            Algo::Md5 => Algorithm::Md5,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

/// `Ok(true)` when every file ended well.
fn run(command: Command) -> Result<bool, String> {
    let (selection, action, db) = match command {
        Command::InitDb { path, name } => {
            let store = Store::create(&path, &name).map_err(|p| p.to_string())?;
            println!("created {} ({})", store.name(), store.path().display());
            return Ok(true);
        }
        Command::Scan { selection, quiet } => return Ok(scan(&selection, quiet)),
        Command::Create {
            selection,
            db,
            no_sidecar,
            sums,
            algorithms,
            new_item,
            existing,
            import_hash_files,
            follow_moves,
        } => {
            let options = CreateOptions {
                algorithms: algorithms.into_iter().map(Algorithm::from).collect(),
                write_sidecar: !no_sidecar,
                write_sums: sums,
                new_item,
                existing: existing.into(),
                import_hash_files,
                follow_moves,
            };
            (selection, BatchAction::Create(options), db)
        }
        Command::Verify {
            selection,
            db,
            no_sidecar,
            follow_moves,
        } => {
            let options = VerifyOptions {
                use_sidecar: !no_sidecar,
                follow_moves,
            };
            (selection, BatchAction::Verify(options), db)
        }
        Command::Search { selection, db } => (selection, BatchAction::Search, Some(db)),
    };
    let mut store = db
        .map(|path| Store::open(&path).map_err(|p| p.to_string()))
        .transpose()?;
    // As `sha256sum -c`: a hash file given to verify stands for the files
    // it lists.
    let files = select(&selection, matches!(action, BatchAction::Verify(_)));
    let options = BatchOptions {
        action,
        parallel_volumes: selection.parallel,
    };
    let control = Control::new();
    let mut all_ok = true;
    let summary = batch::run(&files, &options, store.as_mut(), &control, &mut |update| {
        for entry in &update.entries {
            // A line waiting for its folder's hash list: its final result
            // comes once the list is written.
            if matches!(&entry.outcome, EntryOutcome::Created(r) if r.status == CreateStatus::Pending) {
                continue;
            }
            all_ok &= print_entry(entry);
        }
    })
    .map_err(|p| p.to_string())?;
    print_summary(&summary);
    Ok(all_ok)
}

/// The files of the selection, with what was left out on standard error.
/// With `expand_lists`, a hash file given directly is replaced by the files
/// it lists, each to be verified against its line.
fn select(selection: &Selection, expand_lists: bool) -> Vec<BatchFile> {
    let (lists, paths): (Vec<PathBuf>, Vec<PathBuf>) = selection
        .paths
        .iter()
        .cloned()
        .partition(|path| expand_lists && path.is_file() && manifest::is_hash_file(path));
    let mut files = Vec::new();
    let summary = discovery::discover(
        &paths,
        &selection.options(),
        &Control::new(),
        &mut |batch| {
            files.extend(batch.files.into_iter().map(|f| BatchFile {
                path: f.path,
                size: Some(f.size),
                listed: None,
            }));
            for error in batch.errors {
                eprintln!("selection error\t{}\t{}", error.path, error.problem);
            }
        },
    );
    let excluded: u32 = summary.excluded.iter().map(|c| c.count).sum();
    eprintln!(
        "{} files selected, {excluded} excluded, {} filtered out",
        summary.files, summary.filtered_out
    );
    for list in lists {
        match manifest::list(&list) {
            Ok(listed) => {
                eprintln!("{} files listed in {}", listed.files.len(), listed.path);
                for problem in &listed.problems {
                    eprintln!("\tline {} not used: {}", problem.line, problem.reason);
                }
                files.extend(listed.files);
            }
            Err(problem) => eprintln!("selection error\t{}\t{problem}", list.display()),
        }
    }
    files
}

fn scan(selection: &Selection, quiet: bool) -> bool {
    let start = Instant::now();
    let mut first = None;
    let summary = discovery::discover(
        &selection.paths,
        &selection.options(),
        &Control::new(),
        &mut |batch| {
            if first.is_none() && !batch.files.is_empty() {
                first = Some(start.elapsed());
            }
            if quiet {
                return;
            }
            for file in &batch.files {
                println!("{}\t{}", file.size, file.path);
            }
            for exclusion in &batch.exclusions {
                let what = if exclusion.folder { "folder" } else { "file" };
                println!(
                    "excluded {what} ({:?})\t{}",
                    exclusion.reason, exclusion.path
                );
            }
            for error in &batch.errors {
                println!("error\t{}\t{}", error.path, error.problem);
            }
        },
    );
    println!(
        "{} files, {} bytes, {} folders, {} filtered out, {} errors, {} ms",
        summary.files,
        summary.bytes,
        summary.folders_scanned,
        summary.filtered_out,
        summary.errors,
        summary.elapsed_ms
    );
    if let Some(first) = first {
        println!("first files after {} ms", first.as_millis());
    }
    for count in &summary.excluded {
        println!("excluded: {:?} {}", count.reason, count.count);
    }
    for count in summary.extensions.iter().take(10) {
        let name = if count.extension.is_empty() {
            "(none)"
        } else {
            &count.extension
        };
        println!(
            "extension {name}: {} files, {} bytes",
            count.files, count.bytes
        );
    }
    summary.errors == 0
}

/// Prints one result; true if it ended well.
fn print_entry(entry: &BatchEntry) -> bool {
    println!(
        "#{}\t{}\t{}",
        entry.index + 1,
        entry.outcome.status_code(),
        entry.path
    );
    match &entry.outcome {
        EntryOutcome::Created(report) => print_created(report),
        EntryOutcome::Verified(report) => print_verified(report),
        EntryOutcome::Searched(report) => print_searched(report),
        EntryOutcome::Skipped(skip) => print_skipped(skip),
    }
}

fn print_created(report: &CreateReport) -> bool {
    for digest in &report.digests {
        println!("\t{} {}", digest.algorithm.code(), digest.hex);
    }
    for sidecar in &report.sidecars {
        println!("\thash file {}: {:?}", sidecar.path, sidecar.outcome);
        if let Some(problem) = &sidecar.problem {
            println!("\t\t{problem}");
        }
    }
    if let Some(database) = &report.database {
        println!("\tdatabase: {:?}", database.outcome);
        if let Some(problem) = &database.problem {
            println!("\t\t{problem}");
        }
    }
    if let Some(from) = &report.relocated_from {
        println!("\tlocation followed: was recorded at {from}");
    }
    print_finding(report.finding.as_ref());
    if let Some(problem) = &report.problem {
        println!("\t{problem}");
    }
    matches!(
        report.status,
        CreateStatus::Created | CreateStatus::Completed | CreateStatus::AlreadyReferenced
    )
}

fn print_verified(report: &VerifyReport) -> bool {
    use fileseal_core::ops::VerifyStatus;
    for source in &report.sources {
        println!(
            "\t{:?} {}: {:?}",
            source.source, source.path, source.outcome
        );
        if let Some(detail) = &source.detail {
            println!("\t\t{detail}");
        }
    }
    if let Some(from) = &report.location_change {
        println!("\trecorded at {from} (drive letter changed?)");
    }
    if let Some(from) = &report.relocated_from {
        println!("\tlocation followed: was recorded at {from}");
    }
    print_finding(report.finding.as_ref());
    if let Some(problem) = &report.problem {
        println!("\t{problem}");
    }
    if let Some(problem) = &report.record_problem {
        println!("\tnot recorded in the history: {problem}");
    }
    report.status == VerifyStatus::Match
}

fn print_searched(report: &SearchReport) -> bool {
    if let Some(from) = &report.location_change {
        println!("\trecorded at {from} (drive letter changed?)");
    }
    print_finding(report.finding.as_ref());
    if let Some(problem) = &report.problem {
        println!("\t{problem}");
        return false;
    }
    true
}

fn print_skipped(skip: &Skip) -> bool {
    println!("\t{:?}", skip.reason);
    if let Some(same) = &skip.same_as {
        println!("\tsame file as #{} {}", same.index + 1, same.path);
    }
    if let Some(problem) = &skip.problem {
        println!("\t{problem}");
    }
    skip.same_as.is_some()
}

fn print_finding(finding: Option<&ContentFinding>) {
    let Some(finding) = finding else { return };
    println!("\tcontent: {:?}", finding.outcome);
    for candidate in &finding.same_content {
        println!(
            "\t\tsame content: item {} {:?} {}",
            candidate.item_id, candidate.presence, candidate.path
        );
    }
    for hint in &finding.hints {
        println!(
            "\t\tsame name: item {} {:?} {}",
            hint.item_id, hint.presence, hint.path
        );
    }
}

fn print_summary(summary: &BatchSummary) {
    println!(
        "{} files: {} processed, {} failed, {} cancelled, {} not processed, {} duplicates, {} ms",
        summary.total,
        summary.processed,
        summary.failed,
        summary.cancelled,
        summary.not_processed,
        summary.duplicates,
        summary.elapsed_ms
    );
    for status in &summary.statuses {
        println!("\t{}: {}", status.status, status.count);
    }
    for volume in &summary.lost_volumes {
        println!("volume lost during the batch: {volume}");
    }
    if let Some(problem) = &summary.database_problem {
        println!("batch stopped, database unavailable: {problem}");
    }
}
