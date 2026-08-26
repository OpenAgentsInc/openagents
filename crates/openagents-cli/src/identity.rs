//! Cryptographic identity: one BIP-39 seed, one Nostr identity, one wallet branch.
//!
//! This is the Rust port of `packages/openagents-cli/src/seed-identity.ts`, and the
//! TypeScript module is the contract. The derivation profile
//! (`openagents.legacy_unified_nostr_spark.v1`), the NIP-06 path, the wallet path,
//! the English word list, and the empty BIP-39 passphrase all come from there. An
//! identity that differs between the two CLIs is a different account, so
//! `tests/identity_test.rs` pins the derivation against shared vectors: change the
//! derivation and the build fails rather than silently reissuing every `npub`.
//!
//! SECRETS. The mnemonic is returned from exactly one function,
//! [`SeedStore::read_phrase`], and derived from in memory. [`SeedIdentity`] carries
//! public identifiers only and is safe to print. No `nsec` and no private key is
//! ever written to disk or returned by `show`; the seed file is `0600` inside a
//! `0700` directory.

use bech32::{Bech32, Hrp};
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

/// The frozen shared-root profile both the CLI and Pylon derive under.
pub const DERIVATION_PROFILE_ID: &str = "openagents.legacy_unified_nostr_spark.v1";

/// Nostr identity path: NIP-06 account zero.
pub const NOSTR_DERIVATION_PATH: &str = "m/44'/1237'/0'/0/0";

/// Wallet path: BIP-44 Bitcoin account zero, first external key.
pub const WALLET_DERIVATION_PATH: &str = "m/44'/0'/0'/0/0";

/// The frozen BIP-39 passphrase. It is empty, and a non-empty one produces a
/// different identity, so it is a constant here rather than an option.
const BIP39_PASSPHRASE: &str = "";

/// Mainnet pay-to-public-key-hash version byte, the standard BIP-44 pairing.
const P2PKH_VERSION: u8 = 0x00;

/// Every way identity work fails. No variant carries secret material.
#[derive(Debug)]
pub enum IdentityError {
    /// The phrase is not a valid English BIP-39 mnemonic. Never quotes the phrase.
    InvalidPhrase,
    /// No seed is stored, and the command needs one.
    NoSeed,
    /// A seed is already stored and the command would have overwritten it.
    SeedExists(PathBuf),
    /// Key derivation failed underneath us.
    Derivation(String),
    Io(std::io::Error),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPhrase => {
                write!(f, "The seed phrase is not a valid English BIP-39 mnemonic.")
            }
            Self::NoSeed => write!(
                f,
                "No seed is stored. Run `oa identity create` to make one, or \
                 `oa identity import` to restore an existing seed phrase."
            ),
            Self::SeedExists(path) => write!(
                f,
                "A seed is already stored at {}. Run `oa identity forget --force` first \
                 if you mean to replace it.",
                path.display()
            ),
            Self::Derivation(why) => write!(f, "Key derivation failed: {}", why),
            Self::Io(err) => write!(f, "{}", err),
        }
    }
}

impl std::error::Error for IdentityError {}

impl From<std::io::Error> for IdentityError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// The public half of one seed. Nothing here can spend, sign, or reconstruct the
/// seed, so every field is safe to print, store, and export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedIdentity {
    /// The frozen derivation profile these identifiers were produced under.
    pub profile: String,
    /// The NIP-19 `npub`, and the one cross-surface name for this identity.
    pub npub: String,
    /// The x-only 32-byte Nostr public key as hex.
    pub nostr_public_key_hex: String,
    pub nostr_derivation_path: String,
    /// The compressed 33-byte wallet public key as hex.
    pub wallet_public_key_hex: String,
    /// The BIP-32 key fingerprint, `HASH160(pubkey)[0..4]`, as hex.
    pub wallet_fingerprint_hex: String,
    /// The mainnet P2PKH receive address for the wallet path.
    pub wallet_address: String,
    pub wallet_derivation_path: String,
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Trim and collapse whitespace without changing the words themselves.
pub fn normalize_phrase(phrase: &str) -> String {
    phrase.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True when the phrase is a valid English BIP-39 mnemonic with a good checksum.
pub fn is_valid_seed_phrase(phrase: &str) -> bool {
    Mnemonic::parse_in_normalized(Language::English, &normalize_phrase(phrase)).is_ok()
}

/// Generate a fresh mnemonic from OS entropy. 12 words is 128 bits, 24 words is 256.
///
/// The entropy comes from the operating system through `bip39`'s `rand` feature, so
/// two runs — on one machine or two — produce different phrases. There is no seed
/// constant in this file, and a test asserts successive calls differ.
pub fn generate_seed_phrase(words: usize) -> Result<String, IdentityError> {
    let count = if words == 24 { 24 } else { 12 };
    Mnemonic::generate_in(Language::English, count)
        .map(|m| m.to_string())
        .map_err(|e| IdentityError::Derivation(e.to_string()))
}

/// Derive the public identity and wallet from one mnemonic.
///
/// Deterministic and side-effect free: the same phrase always yields the same `npub`
/// and the same wallet address, on every machine and every version. Refuses a phrase
/// that is not valid BIP-39 English, because deriving from a mistyped phrase would
/// hand back a plausible identity nobody can recover.
pub fn derive_seed_identity(phrase: &str) -> Result<SeedIdentity, IdentityError> {
    let normalized = normalize_phrase(phrase);
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, &normalized)
        .map_err(|_| IdentityError::InvalidPhrase)?;
    let seed = mnemonic.to_seed_normalized(BIP39_PASSPHRASE);

    let nostr_public_key = derive_public_key(&seed, NOSTR_DERIVATION_PATH)?;
    // NIP-06 keys are x-only: drop the compressed-form parity byte.
    let nostr_x_only = &nostr_public_key[1..];

    let wallet_public_key = derive_public_key(&seed, WALLET_DERIVATION_PATH)?;
    let wallet_hash160 = hash160(&wallet_public_key);

    let mut address_payload = [0u8; 21];
    address_payload[0] = P2PKH_VERSION;
    address_payload[1..].copy_from_slice(&wallet_hash160);

    let hrp = Hrp::parse("npub").map_err(|e| IdentityError::Derivation(e.to_string()))?;
    let npub = bech32::encode::<Bech32>(hrp, nostr_x_only)
        .map_err(|e| IdentityError::Derivation(e.to_string()))?;

    Ok(SeedIdentity {
        profile: DERIVATION_PROFILE_ID.to_string(),
        npub,
        nostr_public_key_hex: to_hex(nostr_x_only),
        nostr_derivation_path: NOSTR_DERIVATION_PATH.to_string(),
        wallet_public_key_hex: to_hex(&wallet_public_key),
        wallet_fingerprint_hex: to_hex(&wallet_hash160[..4]),
        wallet_address: bs58::encode(address_payload).with_check().into_string(),
        wallet_derivation_path: WALLET_DERIVATION_PATH.to_string(),
    })
}

/// Derive one compressed 33-byte secp256k1 public key at `path` from a BIP-32 seed.
fn derive_public_key(seed: &[u8; 64], path: &str) -> Result<[u8; 33], IdentityError> {
    let parsed =
        DerivationPath::from_str(path).map_err(|e| IdentityError::Derivation(e.to_string()))?;
    let xprv = XPrv::derive_from_path(seed, &parsed)
        .map_err(|e| IdentityError::Derivation(e.to_string()))?;
    Ok(xprv.public_key().to_bytes())
}

/// `RIPEMD160(SHA256(bytes))`, the standard Bitcoin HASH160.
fn hash160(bytes: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(bytes);
    let mut out = [0u8; 20];
    out.copy_from_slice(&Ripemd160::digest(sha));
    out
}

/// Where the seed lives on disk, and the only thing that touches it.
pub struct SeedStore {
    directory: PathBuf,
}

impl SeedStore {
    /// `OPENAGENTS_IDENTITY_DIR` moves the store, which is how tests get an isolated
    /// identity without touching the developer's own. Matches the TypeScript CLI so
    /// both read the same seed.
    pub fn default_directory() -> PathBuf {
        match std::env::var("OPENAGENTS_IDENTITY_DIR") {
            Ok(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
            _ => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".openagents").join("identity")
            }
        }
    }

    pub fn new(directory: Option<PathBuf>) -> Self {
        Self {
            directory: directory.unwrap_or_else(Self::default_directory),
        }
    }

    /// The seed file itself: one line, the mnemonic, mode `0600`.
    pub fn path(&self) -> PathBuf {
        self.directory.join("seed")
    }

    /// True when a seed is already stored. Presence only; the bytes stay on disk.
    pub fn present(&self) -> bool {
        self.path().is_file()
    }

    /// Read the stored mnemonic. This is the only function that returns secret
    /// material, and every caller either derives from it or hands it to the reader
    /// who asked for a backup.
    pub fn read_phrase(&self) -> Result<Option<String>, IdentityError> {
        let path = self.path();
        if !path.is_file() {
            return Ok(None);
        }
        let phrase = normalize_phrase(&fs::read_to_string(&path)?);
        Ok(if phrase.is_empty() { None } else { Some(phrase) })
    }

    /// Write the mnemonic, `0600` inside a `0700` directory, after validating it.
    /// The validation is not politeness: a phrase stored here that does not validate
    /// would be an identity nobody can recover from its own backup.
    pub fn write_phrase(&self, phrase: &str) -> Result<PathBuf, IdentityError> {
        let normalized = normalize_phrase(phrase);
        if !is_valid_seed_phrase(&normalized) {
            return Err(IdentityError::InvalidPhrase);
        }
        fs::create_dir_all(&self.directory)?;
        Self::set_mode(&self.directory, 0o700)?;
        let path = self.path();
        fs::write(&path, format!("{}\n", normalized))?;
        Self::set_mode(&path, 0o600)?;
        Ok(path)
    }

    /// Remove the stored seed. Idempotent, and it deletes nothing else.
    pub fn forget(&self) -> Result<bool, IdentityError> {
        let path = self.path();
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)?;
        Ok(true)
    }

    /// Derive the public identity from the stored seed, or say there is none.
    pub fn identity(&self) -> Result<SeedIdentity, IdentityError> {
        match self.read_phrase()? {
            Some(phrase) => derive_seed_identity(&phrase),
            None => Err(IdentityError::NoSeed),
        }
    }

    #[cfg(unix)]
    fn set_mode(path: &std::path::Path, mode: u32) -> Result<(), IdentityError> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn set_mode(_path: &std::path::Path, _mode: u32) -> Result<(), IdentityError> {
        Ok(())
    }
}
