//! What one file costs besides its reading, measured on many small files:
//! system calls made per file and database lookups. Used to choose the
//! optimisations of step 10 (docs/performances.md).
//!
//! cargo run -p fileseal-core --release --example costs -- [folder]

use std::path::PathBuf;
use std::time::Instant;

use fileseal_core::platform;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("fileseal-costs");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let files: Vec<PathBuf> = (0..2000)
        .map(|i| {
            let path = dir.join(format!("f{i}.bin"));
            std::fs::write(&path, format!("x{i}")).unwrap();
            path
        })
        .collect();
    let per_file = |label: &str, run: &dyn Fn(&PathBuf)| {
        let start = Instant::now();
        for file in &files {
            run(file);
        }
        let micros = start.elapsed().as_secs_f64() * 1e6 / files.len() as f64;
        println!("{label:<40} {micros:>8.1} µs per file");
    };
    println!(
        "{} files in {} ({} folders deep)",
        files.len(),
        dir.display(),
        dir.components().count()
    );
    per_file("metadata", &|f| {
        std::fs::metadata(f).unwrap();
    });
    per_file("open (read)", &|f| {
        std::fs::File::open(f).unwrap();
    });
    per_file("exists (presence of a recorded file)", &|f| {
        assert!(f.exists());
    });
    per_file("volume root (GetVolumePathNameW)", &|f| {
        platform::volume_root(f).unwrap();
    });
    per_file("volume root of a file (remembered)", &|f| {
        platform::volume_root_of_file(f).unwrap();
    });
    per_file("volume available (root exists)", &|f| {
        platform::volume_available(f);
    });
    per_file("absolute path", &|f| {
        platform::absolute(f).unwrap();
    });
    let _ = std::fs::remove_dir_all(&dir);
}
