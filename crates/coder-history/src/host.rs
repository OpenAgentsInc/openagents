use crate::*;
use std::path::PathBuf;

/// Only these harness subdirectories are read. Credentials and unrelated
/// configuration files in the selected roots are never scanned.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub codex: Option<PathBuf>,
    pub claude: Option<PathBuf>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod catalog;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod confined;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod transcript;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub struct History {
    roots: Vec<confined::Root>,
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub struct History;

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl History {
    pub fn open(config: Config) -> Result<Self, Error> {
        let mut roots = Vec::new();
        for (harness, path) in [
            (Harness::Codex, config.codex),
            (Harness::Claude, config.claude),
        ] {
            if let Some(path) = path {
                roots.push(confined::Root::open(harness, path)?);
            }
        }
        if roots.is_empty() {
            return Err(Error::InvalidRoot);
        }
        Ok(Self { roots })
    }

    pub fn catalog(&self, request: CatalogRequest) -> Result<CatalogPage, Error> {
        catalog::page(self, request)
    }

    pub fn transcript(&self, request: TranscriptRequest) -> Result<TranscriptPage, Error> {
        transcript::page(self, request)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
impl History {
    pub fn open(_: Config) -> Result<Self, Error> {
        Err(Error::UnsupportedPlatform)
    }
    pub fn catalog(&self, _: CatalogRequest) -> Result<CatalogPage, Error> {
        Err(Error::UnsupportedPlatform)
    }
    pub fn transcript(&self, _: TranscriptRequest) -> Result<TranscriptPage, Error> {
        Err(Error::UnsupportedPlatform)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn bounded(value: &str, max: usize) -> (String, bool) {
    let mut end = value.len().min(max);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), end < value.len())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn encoded_len<T: Serialize>(value: &T) -> Result<usize, Error> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|_| Error::Encoding)
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests;
