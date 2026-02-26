use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use bip39::{Language, Mnemonic};

use crate::nip06::derive_keypair;

pub const ENV_IDENTITY_MNEMONIC_PATH: &str = "OPENAGENTS_IDENTITY_MNEMONIC_PATH";

#[derive(Clone)]
pub struct NostrIdentity {
    pub identity_path: PathBuf,
    pub mnemonic: String,
    pub npub: String,
    pub nsec: String,
    pub public_key_hex: String,
    pub private_key_hex: String,
}

pub fn load_or_create_identity() -> Result<NostrIdentity> {
    let path = identity_mnemonic_path()?;
    if path.exists() {
        return load_identity_from_path(path);
    }
    regenerate_identity()
}

pub fn regenerate_identity() -> Result<NostrIdentity> {
    let path = identity_mnemonic_path()?;
    let mnemonic = generate_mnemonic()?;
    write_mnemonic(&path, &mnemonic)?;
    build_identity(mnemonic, path)
}

pub fn identity_mnemonic_path() -> Result<PathBuf> {
    if let Ok(override_path) = std::env::var(ENV_IDENTITY_MNEMONIC_PATH) {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join(".openagents")
        .join("pylon")
        .join("identity.mnemonic"))
}

pub fn load_identity_from_path(path: impl AsRef<Path>) -> Result<NostrIdentity> {
    let path = path.as_ref();
    let mnemonic = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read mnemonic file {}", path.display()))?
        .trim()
        .to_string();

    if mnemonic.is_empty() {
        return Err(anyhow::anyhow!(
            "identity mnemonic file is empty: {}",
            path.display()
        ));
    }

    build_identity(mnemonic, path.to_path_buf())
}

fn build_identity(mnemonic: String, path: PathBuf) -> Result<NostrIdentity> {
    let keypair = derive_keypair(&mnemonic)?;

    Ok(NostrIdentity {
        identity_path: path,
        mnemonic,
        npub: keypair.npub()?,
        nsec: keypair.nsec()?,
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: keypair.private_key_hex(),
    })
}

fn generate_mnemonic() -> Result<String> {
    let entropy: [u8; 16] = rand::random();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .context("failed to generate mnemonic")?;
    Ok(mnemonic.to_string())
}

fn write_mnemonic(path: &Path, mnemonic: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    std::fs::write(path, format!("{mnemonic}\n"))
        .with_context(|| format!("failed to write mnemonic file {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }

    Ok(())
}
