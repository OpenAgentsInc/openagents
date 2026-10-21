//! Deploy keys: a 32-byte secp256k1 secret stored as lowercase hex.

use std::path::Path;

use anyhow::{Context, Result};
use nostr::domain::RelaySigner;

/// Creates a new key file readable only by its owner and returns the
/// public key.
pub fn generate(path: &Path) -> Result<String> {
    let secret = loop {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!("random: {error}"))?;
        let secret = hex::encode(bytes);
        if RelaySigner::from_secret_hex(&secret).is_ok() {
            break secret;
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_private(path, &secret)?;
    Ok(RelaySigner::from_secret_hex(&secret)
        .map_err(|error| anyhow::anyhow!("{error}"))?
        .pubkey()
        .to_owned())
}

/// Loads a signer from a key file.
pub fn load(path: &Path) -> Result<RelaySigner> {
    let secret = std::fs::read_to_string(path)
        .with_context(|| format!("reading deploy key {}", path.display()))?;
    RelaySigner::from_secret_hex(secret.trim()).map_err(|error| anyhow::anyhow!("{error}"))
}

#[cfg(unix)]
fn write_private(path: &Path, contents: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("creating {}", path.display()))?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents)?;
    Ok(())
}
