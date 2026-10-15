//! Source-backed task knowledge, kept separate from the structural graph.

pub mod builder;
pub mod demand;
pub mod episodes;
pub mod evaluation;
pub mod evaluation_pilot;
pub mod learning;
pub mod lookup;
pub mod pilot;
pub mod private;
pub mod recipes;
pub mod runtime;
pub mod store;

pub fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Choose an input smaller than 64 MiB.".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "The input does not match the expected format.".into())
}

use sha2::{Digest, Sha256};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
