use std::path::PathBuf;

use anyhow::{Context, Result};
use bech32::{Bech32, Hrp};
use bip39::{Language, Mnemonic};
use bitcoin::Network;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::{PublicKey, SecretKey};

use crate::app_state::NostrIdentityView;

const NOSTR_COIN_TYPE: u32 = 1237;
const NSEC_HRP: &str = "nsec";
const NPUB_HRP: &str = "npub";
const ENV_IDENTITY_MNEMONIC_PATH: &str = "OPENAGENTS_IDENTITY_MNEMONIC_PATH";

#[derive(Clone)]
struct Keypair {
    private_key: [u8; 32],
    public_key: [u8; 32],
}

pub fn load_or_create_identity() -> Result<NostrIdentityView> {
    let path = identity_mnemonic_path()?;
    if path.exists() {
        return load_identity_from_path(path);
    }
    regenerate_identity()
}

pub fn regenerate_identity() -> Result<NostrIdentityView> {
    let path = identity_mnemonic_path()?;
    let mnemonic = generate_mnemonic()?;
    write_mnemonic(&path, &mnemonic)?;
    build_identity_view(mnemonic, path)
}

fn load_identity_from_path(path: PathBuf) -> Result<NostrIdentityView> {
    let mnemonic = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read mnemonic file {}", path.display()))?
        .trim()
        .to_string();
    if mnemonic.is_empty() {
        return Err(anyhow::anyhow!(
            "identity mnemonic file is empty: {}",
            path.display()
        ));
    }
    build_identity_view(mnemonic, path)
}

fn build_identity_view(mnemonic: String, path: PathBuf) -> Result<NostrIdentityView> {
    let keypair = derive_keypair(&mnemonic)?;
    let npub = encode_bech32(NPUB_HRP, &keypair.public_key)?;
    let nsec = encode_bech32(NSEC_HRP, &keypair.private_key)?;

    Ok(NostrIdentityView {
        identity_path: path.display().to_string(),
        mnemonic,
        npub,
        nsec,
        public_key_hex: hex::encode(keypair.public_key),
        private_key_hex: hex::encode(keypair.private_key),
    })
}

fn generate_mnemonic() -> Result<String> {
    let entropy: [u8; 16] = rand::random();
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .context("failed to generate mnemonic")?;
    Ok(mnemonic.to_string())
}

fn identity_mnemonic_path() -> Result<PathBuf> {
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

fn write_mnemonic(path: &PathBuf, mnemonic: &str) -> Result<()> {
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

fn derive_keypair(mnemonic: &str) -> Result<Keypair> {
    let mnemonic = Mnemonic::parse(mnemonic.trim()).context("invalid mnemonic")?;
    let seed = mnemonic.to_seed("");
    derive_keypair_from_seed(&seed, 0)
}

fn derive_keypair_from_seed(seed: &[u8; 64], account: u32) -> Result<Keypair> {
    let secp = Secp256k1::new();

    let master =
        Xpriv::new_master(Network::Bitcoin, seed).context("failed to create master key")?;
    let path = DerivationPath::from(vec![
        ChildNumber::from_hardened_idx(44).context("failed to derive purpose")?,
        ChildNumber::from_hardened_idx(NOSTR_COIN_TYPE).context("failed to derive coin type")?,
        ChildNumber::from_hardened_idx(account).context("failed to derive account")?,
        ChildNumber::from_normal_idx(0).context("failed to derive change")?,
        ChildNumber::from_normal_idx(0).context("failed to derive index")?,
    ]);

    let derived = master
        .derive_priv(&secp, &path)
        .context("failed to derive nostr private key")?;
    let private_key = derived.private_key.secret_bytes();

    let secret_key =
        SecretKey::from_slice(&private_key).context("failed to create secp256k1 secret key")?;
    let public_key_full = PublicKey::from_secret_key(&secp, &secret_key).serialize();
    let mut public_key = [0_u8; 32];
    public_key.copy_from_slice(&public_key_full[1..33]);

    Ok(Keypair {
        private_key,
        public_key,
    })
}

fn encode_bech32(hrp: &str, data: &[u8; 32]) -> Result<String> {
    let parsed_hrp = Hrp::parse(hrp).context("invalid bech32 hrp")?;
    bech32::encode::<Bech32>(parsed_hrp, data).context("failed to encode bech32")
}
