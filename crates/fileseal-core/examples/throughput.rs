//! Hashing throughput on a file held in the OS cache, so the processor is
//! the limit, not the disk. Results go in docs/performances.md.
//!
//! cargo run -p fileseal-core --release --example throughput -- [size in MiB, default 2048]

use std::io::{Read, Write};
use std::time::Instant;

use fileseal_core::Algorithm;
use fileseal_core::reader::{Control, hash_file};

fn main() {
    let size_mib: usize = std::env::args()
        .nth(1)
        .map(|a| a.parse().expect("size in MiB"))
        .unwrap_or(2048);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("throughput.bin");
    {
        // Pseudo-random content (xorshift), so nothing is trivially compressible.
        let mut file = std::fs::File::create(&path).unwrap();
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut block = vec![0u8; 1 << 20];
        for _ in 0..size_mib {
            for chunk in block.as_chunks_mut::<8>().0 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *chunk = state.to_le_bytes();
            }
            file.write_all(&block).unwrap();
        }
    }
    let mib = size_mib as f64;
    let measure = |label: &str, run: &dyn Fn()| {
        run(); // warm-up: puts the file in the cache
        let best = (0..3)
            .map(|_| {
                let start = Instant::now();
                run();
                start.elapsed().as_secs_f64()
            })
            .fold(f64::INFINITY, f64::min);
        println!("{label:<22} {:>7.0} MiB/s", mib / best);
    };

    println!("{size_mib} MiB file in the OS cache, best of 3 runs");
    measure("read only (1 MiB)", &|| {
        let mut file = std::fs::File::open(&path).unwrap();
        let mut buffer = vec![0u8; 1 << 20];
        while file.read(&mut buffer).unwrap() > 0 {}
    });
    for (label, algorithms) in [
        ("SHA-256", &[Algorithm::Sha256][..]),
        ("MD5", &[Algorithm::Md5][..]),
        ("SHA-256 + MD5", &[Algorithm::Sha256, Algorithm::Md5][..]),
    ] {
        measure(label, &|| {
            hash_file(&path, algorithms, &Control::new(), &mut |_, _| {}).unwrap();
        });
    }
}
