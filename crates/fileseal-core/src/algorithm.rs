//! Supported hash algorithms. Adding one means adding a variant here and the
//! crate that implements it: hash files, databases and the UI all read the
//! metadata below instead of naming algorithms themselves.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Algorithm {
    Sha256,
    Md5,
}

impl Algorithm {
    /// Every supported algorithm, strongest first.
    pub const ALL: [Algorithm; 2] = [Algorithm::Sha256, Algorithm::Md5];

    /// Stable code stored in databases and used as the hash-file extension.
    pub fn code(self) -> &'static str {
        match self {
            Algorithm::Sha256 => "sha256",
            Algorithm::Md5 => "md5",
        }
    }

    /// Case-insensitive, so `.SHA256` files are recognised too.
    pub fn from_code(code: &str) -> Option<Algorithm> {
        Algorithm::ALL
            .into_iter()
            .find(|a| a.code().eq_ignore_ascii_case(code))
    }

    /// Length of the digest written in hexadecimal.
    pub fn hex_len(self) -> usize {
        match self {
            Algorithm::Sha256 => 64,
            Algorithm::Md5 => 32,
        }
    }

    /// False for MD5: it detects accidental changes, not deliberate forgery.
    pub fn is_collision_resistant(self) -> bool {
        match self {
            Algorithm::Sha256 => true,
            Algorithm::Md5 => false,
        }
    }

    fn hasher(self) -> Box<dyn Hasher> {
        match self {
            Algorithm::Sha256 => Box::new(<sha2::Sha256 as sha2::Digest>::new()),
            Algorithm::Md5 => Box::new(<md5::Md5 as md5::Digest>::new()),
        }
    }
}

/// Our own small trait rather than `digest`'s, so that an algorithm whose
/// crate does not implement `digest` (or another version of it) can still
/// be added with a few lines.
trait Hasher: Send {
    fn update(&mut self, data: &[u8]);
    fn finish(self: Box<Self>) -> Vec<u8>;
}

impl<D: sha2::Digest + Send> Hasher for D {
    fn update(&mut self, data: &[u8]) {
        sha2::Digest::update(self, data);
    }

    fn finish(self: Box<Self>) -> Vec<u8> {
        sha2::Digest::finalize(*self).to_vec()
    }
}

/// A digest in lowercase hexadecimal, tagged with its algorithm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Digest {
    pub algorithm: Algorithm,
    pub hex: String,
}

impl Digest {
    /// Accepts upper or lower case; `None` if the text is not a digest of
    /// this algorithm (wrong length or non-hex characters).
    pub fn parse(algorithm: Algorithm, text: &str) -> Option<Digest> {
        (text.len() == algorithm.hex_len() && text.bytes().all(|b| b.is_ascii_hexdigit())).then(
            || Digest {
                algorithm,
                hex: text.to_ascii_lowercase(),
            },
        )
    }
}

/// Finds the digest of `algorithm` in a list.
pub fn find(digests: &[Digest], algorithm: Algorithm) -> Option<&Digest> {
    digests.iter().find(|d| d.algorithm == algorithm)
}

/// Several algorithms fed by the same reads: a file is read once whatever
/// the number of algorithms.
pub struct MultiHasher {
    hashers: Vec<(Algorithm, Box<dyn Hasher>)>,
}

impl MultiHasher {
    pub fn new(algorithms: &[Algorithm]) -> MultiHasher {
        let mut unique: Vec<Algorithm> = algorithms.to_vec();
        unique.sort_unstable();
        unique.dedup();
        MultiHasher {
            hashers: unique.into_iter().map(|a| (a, a.hasher())).collect(),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        for (_, hasher) in &mut self.hashers {
            hasher.update(data);
        }
    }

    pub fn finish(self) -> Vec<Digest> {
        self.hashers
            .into_iter()
            .map(|(algorithm, hasher)| Digest {
                algorithm,
                hex: to_hex(&hasher.finish()),
            })
            .collect()
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[usize::from(byte >> 4)] as char);
        text.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_of(algorithm: Algorithm, data: &[u8]) -> String {
        let mut hasher = MultiHasher::new(&[algorithm]);
        hasher.update(data);
        hasher.finish().remove(0).hex
    }

    // Reference values: FIPS 180-2 examples and RFC 1321 test suite.
    #[test]
    fn known_vectors() {
        assert_eq!(
            digest_of(Algorithm::Sha256, b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest_of(Algorithm::Sha256, b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            digest_of(
                Algorithm::Sha256,
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            ),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            digest_of(Algorithm::Md5, b""),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
        assert_eq!(
            digest_of(Algorithm::Md5, b"abc"),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(
            digest_of(Algorithm::Md5, b"message digest"),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
    }

    #[test]
    fn several_algorithms_in_one_pass_match_separate_passes() {
        let data = b"The quick brown fox jumps over the lazy dog";
        let mut both = MultiHasher::new(&[Algorithm::Md5, Algorithm::Sha256, Algorithm::Md5]);
        for chunk in data.chunks(7) {
            both.update(chunk);
        }
        let digests = both.finish();
        assert_eq!(digests.len(), 2, "duplicates are merged");
        assert_eq!(
            find(&digests, Algorithm::Md5).unwrap().hex,
            "9e107d9d372bb6826bd81d3542a419d6"
        );
        assert_eq!(
            find(&digests, Algorithm::Sha256).unwrap().hex,
            digest_of(Algorithm::Sha256, data)
        );
    }

    #[test]
    fn parses_digests_strictly() {
        let upper = "D41D8CD98F00B204E9800998ECF8427E";
        assert_eq!(
            Digest::parse(Algorithm::Md5, upper).unwrap().hex,
            upper.to_lowercase()
        );
        assert!(
            Digest::parse(Algorithm::Sha256, upper).is_none(),
            "wrong length"
        );
        assert!(Digest::parse(Algorithm::Md5, "g41d8cd98f00b204e9800998ecf8427e").is_none());
        assert_eq!(Algorithm::from_code("SHA256"), Some(Algorithm::Sha256));
        assert_eq!(Algorithm::from_code("sha1"), None);
    }
}
