//! Source-backed task knowledge, kept separate from the structural graph.

pub mod episodes;
pub mod private;

use sha2::{Digest, Sha256};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
