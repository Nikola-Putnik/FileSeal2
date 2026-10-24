//! Portable FileSeal: a `portable.flag` file beside the program makes it
//! keep everything it writes (settings, logs, WebView2 data) in a `data`
//! folder there, and nothing in the user's profile. Meant for a USB key.

use std::path::{Component, Path, PathBuf, Prefix};

/// The file that turns portable mode on, beside the program.
const FLAG: &str = "portable.flag";

pub enum Mode {
    Installed,
    Portable(Portable),
    /// The flag is there but its folder cannot be written (read-only
    /// medium): FileSeal then works as if installed, rather than not start.
    ReadOnly(PathBuf),
}

pub struct Portable {
    /// Where the program is: proposed for new databases.
    pub program_dir: PathBuf,
    /// Settings, logs and WebView2 data.
    pub data_dir: PathBuf,
    /// The drive the program runs from (`E:`), if it is a drive letter.
    pub drive: Option<String>,
}

impl Mode {
    pub fn detect() -> Mode {
        let Some(program_dir) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        else {
            return Mode::Installed;
        };
        if !program_dir.join(FLAG).is_file() {
            return Mode::Installed;
        }
        let data_dir = program_dir.join("data");
        if !writable(&data_dir) {
            return Mode::ReadOnly(data_dir);
        }
        Mode::Portable(Portable {
            drive: drive_of(&program_dir),
            program_dir,
            data_dir,
        })
    }

    pub fn portable(&self) -> Option<&Portable> {
        match self {
            Mode::Portable(portable) => Some(portable),
            _ => None,
        }
    }
}

fn writable(dir: &Path) -> bool {
    let probe = dir.join(".fileseal-write-test");
    let ok = std::fs::create_dir_all(dir).is_ok() && std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

fn drive_of(path: &Path) -> Option<String> {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                Some(format!("{}:", char::from(letter).to_ascii_uppercase()))
            }
            _ => None,
        },
        _ => None,
    }
}
