//! Throughput with the disk in the way: test files are written without
//! going through the Windows file cache (unbuffered, write-through), so the
//! first read of each really comes from the disk. Measures the raw disk
//! (unbuffered reads), then FileSeal's own reader on files not in the cache.
//! Results go in docs/performances.md.
//!
//! cargo run -p fileseal-core --release --example disk_read -- [folder] [size in MiB, default 4096]
//!
//! The folder decides the disk measured (default: the system's temporary
//! folder). Files are deleted at the end.

#[cfg(windows)]
fn main() {
    use std::io::{Read, Write};
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use fileseal_core::Algorithm;
    use fileseal_core::reader::{Control, hash_file};

    const NO_BUFFERING: u32 = 0x2000_0000;
    const WRITE_THROUGH: u32 = 0x8000_0000;
    const BLOCK: usize = 4 << 20;

    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let size_mib: usize = args
        .next()
        .map(|a| a.parse().expect("size in MiB"))
        .unwrap_or(4096);
    let blocks = size_mib / 4;

    // A buffer aligned on 4 KiB, as unbuffered I/O requires, without unsafe
    // code: a larger vector, used from its first aligned byte.
    let mut raw = vec![0u8; BLOCK + 4096];
    let offset = raw.as_ptr().align_offset(4096);
    let aligned = &mut raw[offset..offset + BLOCK];

    let write = |path: &Path, aligned: &mut [u8]| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(NO_BUFFERING | WRITE_THROUGH)
            .open(path)
            .expect("test file created");
        // Pseudo-random content, so nothing is compressible.
        let mut state = 0x9E37_79B9_7F4A_7C15u64 ^ path.as_os_str().len() as u64;
        for _ in 0..blocks {
            for chunk in aligned.as_chunks_mut::<8>().0 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *chunk = state.to_le_bytes();
            }
            file.write_all(aligned).unwrap();
        }
    };
    let rate = |seconds: f64| (blocks * 4) as f64 / seconds;
    let files: Vec<PathBuf> = (0..4)
        .map(|i| dir.join(format!("fileseal-disk-read-{i}.bin")))
        .collect();
    for path in &files {
        let _ = std::fs::remove_file(path);
    }

    println!("{} MiB per file, in {}", blocks * 4, dir.display());
    let start = Instant::now();
    write(&files[0], aligned);
    println!(
        "{:<36} {:>7.0} MiB/s",
        "write, unbuffered",
        rate(start.elapsed().as_secs_f64())
    );

    // The disk alone: unbuffered reads, nothing kept in the cache.
    let start = Instant::now();
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(NO_BUFFERING)
        .open(&files[0])
        .unwrap();
    while file.read(aligned).unwrap() > 0 {}
    println!(
        "{:<36} {:>7.0} MiB/s",
        "read, unbuffered (the disk alone)",
        rate(start.elapsed().as_secs_f64())
    );

    // FileSeal's reader on files that are not in the cache.
    for (path, label, algorithms) in [
        (
            &files[1],
            "FileSeal SHA-256, not in cache",
            &[Algorithm::Sha256][..],
        ),
        (
            &files[2],
            "FileSeal MD5, not in cache",
            &[Algorithm::Md5][..],
        ),
        (
            &files[3],
            "FileSeal SHA-256 + MD5, not in cache",
            &[Algorithm::Sha256, Algorithm::Md5][..],
        ),
    ] {
        write(path, aligned);
        let start = Instant::now();
        hash_file(path, algorithms, &Control::new(), &mut |_, _| {}).unwrap();
        println!(
            "{label:<36} {:>7.0} MiB/s",
            rate(start.elapsed().as_secs_f64())
        );
    }
    let start = Instant::now();
    hash_file(
        &files[1],
        &[Algorithm::Sha256],
        &Control::new(),
        &mut |_, _| {},
    )
    .unwrap();
    println!(
        "{:<36} {:>7.0} MiB/s",
        "FileSeal SHA-256, in cache",
        rate(start.elapsed().as_secs_f64())
    );
    for path in &files {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("unbuffered I/O is only measured on Windows for now");
}
