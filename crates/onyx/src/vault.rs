//! Vault management - flat folder of markdown notes

use std::path::PathBuf;

/// A vault is a flat directory containing markdown notes
pub struct Vault {
    /// Root path of the vault
    pub path: PathBuf,
}

impl Vault {
    /// Open a vault at the given path
    pub fn open(path: PathBuf) -> Self {
        Self { path }
    }
}
