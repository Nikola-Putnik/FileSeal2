//! FileSeal core: hashing, hash files ("sidecars"), reference databases and
//! the rules that decide what a result means. No UI code lives here: the
//! Tauri shell and the CLI are thin layers over this crate, and every rule
//! can be tested with `cargo test`.
//!
//! Invariants enforced here (see `docs/cadrage.md`):
//! - a source file is only ever opened for reading;
//! - a verification never creates or replaces a reference;
//! - no hash is published unless the whole file was read, unchanged;
//! - a result with one failed destination is never reported as a success.

pub mod algorithm;
pub mod batch;
pub mod discovery;
pub mod matching;
pub mod ops;
pub mod problem;
pub mod progress;
pub mod reader;
pub mod report;
pub mod settings;
pub mod sidecar;
pub mod store;
pub mod sums;

mod faults;
pub mod platform;
mod timestamp;

pub use algorithm::{Algorithm, Digest};
pub use problem::{Problem, ProblemKind};
pub use reader::Control;
pub use store::{DatabaseInfo, Store};

/// Version written into databases and reports.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    /// The workspace version is the one of every Rust crate and of the
    /// installer; the interface package must say the same.
    #[test]
    fn the_interface_package_has_the_same_version() {
        let package = include_str!("../../../package.json");
        let expected = format!("\"version\": \"{}\"", super::APP_VERSION);
        assert!(package.contains(&expected), "package.json: {expected}");
    }
}
