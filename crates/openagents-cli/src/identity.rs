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
//! ever written to disk or returned by `show`.
//!
//! AT REST. The seed file is `0600` inside a `0700` directory, and on a machine
//! with an OS keychain it holds ciphertext rather than the phrase: a 32-byte
//! ChaCha20-Poly1305 wrapping key lives in the keychain under service
//! `openagents-cli-identity`, and the file holds only the sealed envelope. That is
//! what stops the threats permissions never did — a backup tool, a sync client, an
//! agent with read access to `$HOME`, or a stolen unlocked disk image.
//!
//! Where there is no keychain — CI, a container, an unattended agent host — the
//! phrase is written as plaintext at `0600`, exactly as before, and [`SeedStore`]
//! reports [`SeedProtection::PlaintextFile`] so every surface that shows an identity
//! can say so. A silent fall back to plaintext would be worse than no encryption at
//! all, because it would read as protection that is not there. The key never goes in
//! the file, so the phrase exists in exactly one place either way.

use bech32::{Bech32, Hrp};
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};
use ripemd::Ripemd160;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::Mutex;
use zeroize::Zeroize;

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
    /// This machine has no OS keychain to hold a wrapping key. Not a failure on
    /// its own: it selects the plaintext store, and the caller must say so.
    NoKeychain,
    /// The keychain is here but would not answer, or answered with a record that
    /// is not a wrapping key. Never a reason to mint a second key: that would
    /// orphan the sealed seed the first one opens.
    Keychain(String),
    /// The seed on disk is sealed and the keychain holds no key for it.
    SealedWithoutKey(PathBuf),
    /// The seed on disk is sealed and the key present does not open it.
    Undecryptable(PathBuf),
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
            Self::NoKeychain => write!(
                f,
                "This machine has no OS keychain, so there is nowhere to hold a key."
            ),
            Self::Keychain(why) => write!(f, "The OS keychain could not be used: {}", why),
            Self::SealedWithoutKey(path) => write!(
                f,
                "The seed at {} is encrypted, and the OS keychain holds no key that opens it. \
                 The key does not travel with the file and is not in any backup of it. Restore \
                 the seed phrase with `oa identity import`.",
                path.display()
            ),
            Self::Undecryptable(path) => write!(
                f,
                "The seed at {} is encrypted and the key in the OS keychain does not open it. \
                 Restore the seed phrase with `oa identity import`.",
                path.display()
            ),
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

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.is_empty() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
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

// ---------------------------------------------------------------------------
// protection at rest
// ---------------------------------------------------------------------------

/// What is actually protecting the stored seed. Every surface that shows an
/// identity reports this, because the difference between the two is the whole
/// security posture of the machine and a person cannot infer it from the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedProtection {
    /// The file holds a sealed envelope. The key that opens it is in the OS
    /// keychain and never touches the identity directory.
    OsKeychain,
    /// The file holds the phrase itself at `0600`. Filesystem permissions are
    /// the entire protection.
    PlaintextFile,
}

impl SeedProtection {
    /// The stable machine name. `oa identity show --json` carries this.
    pub fn id(self) -> &'static str {
        match self {
            Self::OsKeychain => "os_keychain",
            Self::PlaintextFile => "plaintext_file",
        }
    }

    pub fn encrypted_at_rest(self) -> bool {
        matches!(self, Self::OsKeychain)
    }

    /// The sentence a person reads. It says what is protecting the seed and, for
    /// the plaintext store, what that protection does not cover — a fallback
    /// nobody is told about is the same defect as a redaction that reports
    /// success and leaves the secret in place.
    pub fn describe(self, path: &Path) -> String {
        match self {
            Self::OsKeychain => format!(
                "Protection: OS keychain. The seed at {} is encrypted \
                 ({}); the key that opens it is held by the OS keychain under service {}, \
                 never in the file and never in a backup of it.",
                path.display(),
                SEED_ENVELOPE_ALG,
                IDENTITY_KEYCHAIN_SERVICE
            ),
            Self::PlaintextFile => format!(
                "Protection: NONE. The seed phrase is stored as readable text at {} (mode 0600). \
                 No OS keychain is available here, so file permissions are the whole protection: \
                 they stop another local user, and they stop nothing that already runs as you — \
                 a backup tool, a sync client, or an agent that can read your home directory. \
                 Treat this file the way you would treat the phrase written on paper.",
                path.display()
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// the sealed envelope
// ---------------------------------------------------------------------------

/// The on-disk format both CLIs read and write. Changing any of these three
/// constants makes one CLI unable to open the other's seed.
const SEED_ENVELOPE_SCHEMA: &str = "openagents.cli_identity_seed.v1";
const SEED_ENVELOPE_ALG: &str = "chacha20-poly1305";
/// Bound into the AEAD as additional data, so an envelope cannot be replayed
/// under a different schema.
const SEED_ENVELOPE_AAD: &[u8] = SEED_ENVELOPE_SCHEMA.as_bytes();

#[derive(Serialize, Deserialize)]
struct SeedEnvelope {
    schema: String,
    alg: String,
    /// The 12-byte AEAD nonce, hex. Fresh on every write.
    nonce: String,
    /// Ciphertext with the 16-byte Poly1305 tag appended, hex.
    ciphertext: String,
}

/// True when the file at hand is a sealed envelope rather than a bare mnemonic.
/// A BIP-39 phrase can never start with `{`, so the two formats cannot be
/// confused and an old plaintext seed is still recognised for migration.
fn looks_sealed(text: &str) -> bool {
    text.trim_start().starts_with('{')
}

fn seal_phrase(phrase: &str, key: &[u8; 32]) -> Result<String, IdentityError> {
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, key)
        .map_err(|_| IdentityError::Keychain("the wrapping key is not usable".to_string()))?;
    let sealing = LessSafeKey::new(unbound);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce_bytes)
        .map_err(|_| IdentityError::Derivation("the system random source failed".to_string()))?;

    let mut in_out = phrase.as_bytes().to_vec();
    sealing
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(SEED_ENVELOPE_AAD),
            &mut in_out,
        )
        .map_err(|_| IdentityError::Derivation("the seed could not be encrypted".to_string()))?;

    let envelope = SeedEnvelope {
        schema: SEED_ENVELOPE_SCHEMA.to_string(),
        alg: SEED_ENVELOPE_ALG.to_string(),
        nonce: to_hex(&nonce_bytes),
        ciphertext: to_hex(&in_out),
    };
    in_out.zeroize();
    serde_json::to_string(&envelope)
        .map_err(|e| IdentityError::Derivation(format!("the envelope could not be encoded: {e}")))
}

fn open_envelope(text: &str, key: &[u8; 32], path: &Path) -> Result<String, IdentityError> {
    let envelope: SeedEnvelope = serde_json::from_str(text.trim())
        .map_err(|_| IdentityError::Undecryptable(path.to_path_buf()))?;
    if envelope.schema != SEED_ENVELOPE_SCHEMA || envelope.alg != SEED_ENVELOPE_ALG {
        return Err(IdentityError::Undecryptable(path.to_path_buf()));
    }
    let nonce_bytes: [u8; NONCE_LEN] = from_hex(&envelope.nonce)
        .and_then(|bytes| <[u8; NONCE_LEN]>::try_from(bytes.as_slice()).ok())
        .ok_or_else(|| IdentityError::Undecryptable(path.to_path_buf()))?;
    let mut in_out = from_hex(&envelope.ciphertext)
        .ok_or_else(|| IdentityError::Undecryptable(path.to_path_buf()))?;

    let unbound = UnboundKey::new(&CHACHA20_POLY1305, key)
        .map_err(|_| IdentityError::Keychain("the wrapping key is not usable".to_string()))?;
    let opening = LessSafeKey::new(unbound);
    let opened = opening
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(SEED_ENVELOPE_AAD),
            &mut in_out,
        )
        .map_err(|_| IdentityError::Undecryptable(path.to_path_buf()))?;
    let phrase = String::from_utf8(opened.to_vec())
        .map_err(|_| IdentityError::Undecryptable(path.to_path_buf()))?;
    in_out.zeroize();
    Ok(normalize_phrase(&phrase))
}

// ---------------------------------------------------------------------------
// where the wrapping key lives
// ---------------------------------------------------------------------------

/// The service name the OS keychain files the identity wrapping key under. It is
/// deliberately not `openagents-cli` (account tokens) or `openagents-cli-computer`
/// (machine tokens), so no two of the three can overwrite each other. The
/// TypeScript CLI uses the same one.
pub const IDENTITY_KEYCHAIN_SERVICE: &str = "openagents-cli-identity";

/// Set this to opt out of the keychain and store the phrase as plaintext at
/// `0600`. It exists because a keychain that prompts is worse than no keychain
/// on an unattended host, and because the choice should be stateable rather than
/// discovered. It is never selected implicitly.
pub const PLAINTEXT_ENV: &str = "OPENAGENTS_IDENTITY_PLAINTEXT";

/// Where the 32-byte wrapping key lives. One implementation talks to the OS
/// keychain; the others exist so a test exercises the real seal, open, and
/// migration paths without touching the developer's own keychain.
pub trait SeedKeyStore: Send + Sync {
    /// `Ok(None)` means the store answered and holds no key for this identity
    /// directory. `Err(NoKeychain)` means there is no store on this machine,
    /// which selects the plaintext file. Any other error must not be read as
    /// "no key": minting a second key would orphan the sealed seed.
    fn get(&self) -> Result<Option<[u8; 32]>, IdentityError>;
    /// Store the key and prove it by reading it back. A store that reports
    /// success without keeping the value would seal a seed nobody can open.
    fn put(&self, key: &[u8; 32]) -> Result<(), IdentityError>;
    /// Best-effort removal. Used by `forget`, so a deleted identity does not
    /// leave its key behind.
    fn delete(&self);
}

/// The OS keychain: `security` on macOS, `secret-tool` on Linux.
///
/// The record is keyed by the identity directory, exactly as the credential
/// store keys tokens by origin, so a second identity directory gets a second key
/// and a test with a temporary directory can never reach the developer's own.
pub struct OsKeychainKeyStore {
    account: String,
}

impl OsKeychainKeyStore {
    pub fn for_directory(directory: &Path) -> Self {
        Self {
            account: directory.display().to_string(),
        }
    }

    fn get_command(&self) -> Option<Command> {
        if cfg!(target_os = "macos") {
            let mut command = Command::new("security");
            command.args([
                "find-generic-password",
                "-a",
                &self.account,
                "-s",
                IDENTITY_KEYCHAIN_SERVICE,
                "-w",
            ]);
            command.stderr(Stdio::null());
            Some(command)
        } else if cfg!(target_os = "linux") {
            let mut command = Command::new("secret-tool");
            command.args([
                "lookup",
                "service",
                IDENTITY_KEYCHAIN_SERVICE,
                "account",
                &self.account,
            ]);
            command.stderr(Stdio::null());
            Some(command)
        } else {
            None
        }
    }
}

impl SeedKeyStore for OsKeychainKeyStore {
    fn get(&self) -> Result<Option<[u8; 32]>, IdentityError> {
        let Some(mut command) = self.get_command() else {
            return Err(IdentityError::NoKeychain);
        };
        // A `security` or `secret-tool` that will not start is not an empty
        // store: this platform has no keychain, and that is a different answer.
        let output = command.output().map_err(|_| IdentityError::NoKeychain)?;
        if !output.status.success() {
            return Ok(None);
        }
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if value.is_empty() {
            return Ok(None);
        }
        match from_hex(&value).and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok()) {
            Some(key) => Ok(Some(key)),
            // Never regenerate here. A record that is not a wrapping key means
            // something else wrote it, and overwriting it would make the sealed
            // seed permanently unopenable.
            None => Err(IdentityError::Keychain(format!(
                "the record under service {} is not an identity wrapping key",
                IDENTITY_KEYCHAIN_SERVICE
            ))),
        }
    }

    fn put(&self, key: &[u8; 32]) -> Result<(), IdentityError> {
        let encoded = to_hex(key);
        let stored = if cfg!(target_os = "macos") {
            // `security` reads the value from argv, so the wrapping key is
            // briefly visible to `ps`. The seed phrase never is: it goes to the
            // file sealed, and the key alone opens nothing without that file.
            Command::new("security")
                .args([
                    "add-generic-password",
                    "-U",
                    "-a",
                    &self.account,
                    "-s",
                    IDENTITY_KEYCHAIN_SERVICE,
                    "-w",
                    &encoded,
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .map_err(|_| IdentityError::NoKeychain)?
        } else if cfg!(target_os = "linux") {
            let child = Command::new("secret-tool")
                .args([
                    "store",
                    "--label=OpenAgents identity",
                    "service",
                    IDENTITY_KEYCHAIN_SERVICE,
                    "account",
                    &self.account,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            match child {
                Ok(mut child) => {
                    if let Some(mut pipe) = child.stdin.take() {
                        let _ = pipe.write_all(encoded.as_bytes());
                    }
                    matches!(child.wait(), Ok(status) if status.success())
                }
                Err(_) => return Err(IdentityError::NoKeychain),
            }
        } else {
            return Err(IdentityError::NoKeychain);
        };
        if !stored {
            return Err(IdentityError::Keychain(
                "the OS keychain refused to store the identity wrapping key".to_string(),
            ));
        }
        match self.get()? {
            Some(read_back) if read_back == *key => Ok(()),
            _ => Err(IdentityError::Keychain(
                "the OS keychain did not return the key that was just written".to_string(),
            )),
        }
    }

    fn delete(&self) {
        if cfg!(target_os = "macos") {
            let _ = Command::new("security")
                .args([
                    "delete-generic-password",
                    "-a",
                    &self.account,
                    "-s",
                    IDENTITY_KEYCHAIN_SERVICE,
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        } else if cfg!(target_os = "linux") {
            let _ = Command::new("secret-tool")
                .args([
                    "clear",
                    "service",
                    IDENTITY_KEYCHAIN_SERVICE,
                    "account",
                    &self.account,
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

/// A machine with no keychain: CI, a container, an unattended agent host. Every
/// call says so, which is what selects the plaintext store and the warning that
/// goes with it.
pub struct NoKeyStore;

impl SeedKeyStore for NoKeyStore {
    fn get(&self) -> Result<Option<[u8; 32]>, IdentityError> {
        Err(IdentityError::NoKeychain)
    }
    fn put(&self, _key: &[u8; 32]) -> Result<(), IdentityError> {
        Err(IdentityError::NoKeychain)
    }
    fn delete(&self) {}
}

/// A keychain that lives for the length of one test, so the seal, open, and
/// migration paths are exercised for real without writing to the developer's own
/// keychain or depending on one existing.
#[derive(Default)]
pub struct InMemoryKeyStore {
    key: Mutex<Option<[u8; 32]>>,
}

impl InMemoryKeyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SeedKeyStore for InMemoryKeyStore {
    fn get(&self) -> Result<Option<[u8; 32]>, IdentityError> {
        Ok(*self.key.lock().unwrap())
    }
    fn put(&self, key: &[u8; 32]) -> Result<(), IdentityError> {
        *self.key.lock().unwrap() = Some(*key);
        Ok(())
    }
    fn delete(&self) {
        *self.key.lock().unwrap() = None;
    }
}

/// A seed read back off disk, and what was protecting it there.
pub struct StoredSeed {
    /// The mnemonic. Secret; there is deliberately no `Debug`.
    pub phrase: String,
    pub protection: SeedProtection,
}

/// Where the seed lives on disk, and the only thing that touches it.
pub struct SeedStore {
    directory: PathBuf,
    keys: Box<dyn SeedKeyStore>,
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

    /// The production store: the OS keychain holds the wrapping key, unless
    /// [`PLAINTEXT_ENV`] says otherwise.
    pub fn new(directory: Option<PathBuf>) -> Self {
        let directory = directory.unwrap_or_else(Self::default_directory);
        let keys: Box<dyn SeedKeyStore> = if plaintext_requested() {
            Box::new(NoKeyStore)
        } else {
            Box::new(OsKeychainKeyStore::for_directory(&directory))
        };
        Self { directory, keys }
    }

    /// A store with the wrapping key held somewhere a test controls, so the seal,
    /// open, and migration paths run for real without touching the developer's
    /// own keychain. Mirrors `CredentialStore::isolated`.
    pub fn with_key_store(directory: PathBuf, keys: Box<dyn SeedKeyStore>) -> Self {
        Self { directory, keys }
    }

    /// The seed file: a sealed envelope under the OS keychain, or the mnemonic
    /// itself where there is no keychain. Mode `0600` either way.
    pub fn path(&self) -> PathBuf {
        self.directory.join(Self::SEED_FILE_NAME)
    }

    const SEED_FILE_NAME: &'static str = "seed";

    /// A staging path for one rewrite, unique to this call.
    ///
    /// It used to be the fixed `seed.tmp`. Two processes writing a seed under
    /// one `$HOME` then shared it: each truncated the other's staged bytes and
    /// removed the file out from under it, so one rewrite could rename a
    /// half-written envelope over the seed, or fail after the other had already
    /// replaced it. A seed is the one file where either outcome loses an
    /// identity outright. See [`crate::auth::unique_temp_path`].
    fn temp_path(&self) -> PathBuf {
        crate::auth::unique_temp_path(&self.path())
    }

    /// Remove staging files this directory still holds from crashed rewrites.
    ///
    /// With one fixed staging name `forget` could just delete it. Unique names
    /// mean a sweep instead: anything beside the seed that this store's writes
    /// would have named.
    fn sweep_temp_files(&self) {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return;
        };
        let prefix = format!(".{}.", Self::SEED_FILE_NAME);
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && name.ends_with(crate::auth::TEMP_SUFFIX) {
                let _ = fs::remove_file(entry.path());
            }
            // The name a crashed pre-sweep `oa` left behind.
            if name == "seed.tmp" {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    /// True when a seed is already stored. Presence only; the bytes stay on disk.
    pub fn present(&self) -> bool {
        self.path().is_file()
    }

    /// What a write would use on this machine right now. `Err` only when the
    /// keychain is present but unusable, which must not be silently downgraded
    /// to plaintext.
    pub fn available_protection(&self) -> Result<SeedProtection, IdentityError> {
        match self.keys.get() {
            Ok(_) => Ok(SeedProtection::OsKeychain),
            Err(IdentityError::NoKeychain) => Ok(SeedProtection::PlaintextFile),
            Err(other) => Err(other),
        }
    }

    /// What is protecting the seed that is on disk now, without opening it.
    /// `Ok(None)` when nothing is stored.
    pub fn protection_on_disk(&self) -> Result<Option<SeedProtection>, IdentityError> {
        let path = self.path();
        if !path.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(if looks_sealed(&text) {
            SeedProtection::OsKeychain
        } else {
            SeedProtection::PlaintextFile
        }))
    }

    /// Read the stored seed and report what was protecting it. This and
    /// [`SeedStore::read_phrase`] are the only functions that return secret
    /// material.
    pub fn load(&self) -> Result<Option<StoredSeed>, IdentityError> {
        let path = self.path();
        if !path.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path)?;
        if text.trim().is_empty() {
            return Ok(None);
        }
        if !looks_sealed(&text) {
            let phrase = normalize_phrase(&text);
            return Ok(Some(StoredSeed {
                phrase,
                protection: SeedProtection::PlaintextFile,
            }));
        }
        // Sealed. A keychain that cannot be read is never reported as "no seed":
        // that reads as an identity that vanished, and the next command would
        // offer to make a new one.
        let key = self
            .keys
            .get()?
            .ok_or_else(|| IdentityError::SealedWithoutKey(path.clone()))?;
        let phrase = open_envelope(&text, &key, &path)?;
        Ok(Some(StoredSeed {
            phrase,
            protection: SeedProtection::OsKeychain,
        }))
    }

    /// Read the stored mnemonic. Every caller either derives from it or hands it
    /// to the reader who asked for a backup.
    pub fn read_phrase(&self) -> Result<Option<String>, IdentityError> {
        Ok(self.load()?.map(|stored| stored.phrase))
    }

    /// Write the mnemonic under the best protection this machine has, `0600`
    /// inside a `0700` directory, after validating it. The validation is not
    /// politeness: a phrase stored here that does not validate would be an
    /// identity nobody can recover from its own backup.
    ///
    /// The write is atomic — staged in a sibling file and renamed over the
    /// target — so the phrase is never in two files at once and a crash mid-write
    /// leaves the previous seed intact rather than half of the new one.
    pub fn write_phrase(&self, phrase: &str) -> Result<PathBuf, IdentityError> {
        Ok(self.store_phrase(phrase)?.0)
    }

    /// The same write, and the protection it landed under.
    pub fn store_phrase(&self, phrase: &str) -> Result<(PathBuf, SeedProtection), IdentityError> {
        let normalized = normalize_phrase(phrase);
        if !is_valid_seed_phrase(&normalized) {
            return Err(IdentityError::InvalidPhrase);
        }
        let protection = self.available_protection()?;
        let body = match protection {
            SeedProtection::OsKeychain => {
                let key = match self.keys.get()? {
                    Some(key) => key,
                    None => {
                        let mut fresh = [0u8; 32];
                        SystemRandom::new().fill(&mut fresh).map_err(|_| {
                            IdentityError::Derivation("the system random source failed".to_string())
                        })?;
                        // Prove the keychain kept it before anything is sealed
                        // under it. Sealing first would produce a file no key
                        // opens.
                        self.keys.put(&fresh)?;
                        fresh
                    }
                };
                let sealed = seal_phrase(&normalized, &key)?;
                format!("{}\n", sealed)
            }
            SeedProtection::PlaintextFile => format!("{}\n", normalized),
        };
        let path = self.write_atomic(body.as_bytes())?;
        Ok((path, protection))
    }

    fn write_atomic(&self, bytes: &[u8]) -> Result<PathBuf, IdentityError> {
        fs::create_dir_all(&self.directory)?;
        Self::set_mode(&self.directory, 0o700)?;
        let path = self.path();
        let temp = self.temp_path();
        // No `remove_file` first: the name belongs to this call alone, so
        // anything already at it would be a surprise rather than our own
        // leftovers, and `create_new` inside `write_sealed` says so.
        if let Err(error) = Self::write_sealed(&temp, bytes) {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        if let Err(error) = fs::rename(&temp, &path) {
            let _ = fs::remove_file(&temp);
            return Err(IdentityError::Io(error));
        }
        Self::set_mode(&path, 0o600)?;
        Ok(path)
    }

    /// Put `bytes` in a new file that is `0600` from the moment it exists.
    ///
    /// Created `0600` rather than created and then restricted: the seed must
    /// never be readable to the rest of the machine, not even for the instant
    /// between the two calls.
    fn write_sealed(temp: &std::path::Path, bytes: &[u8]) -> Result<(), IdentityError> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(temp)?;
        file.write_all(bytes)?;
        Self::set_mode(temp, 0o600)?;
        Ok(())
    }

    /// Move a plaintext seed under the OS keychain, and report what is protecting
    /// it afterwards. `Ok(None)` when nothing is stored.
    ///
    /// The rewrite lands on the same path by rename, so there is never a moment
    /// with the phrase in two files, and the plaintext is gone the instant the
    /// sealed envelope arrives. On a machine with no keychain this changes
    /// nothing and reports [`SeedProtection::PlaintextFile`], which is what the
    /// caller then has to say out loud.
    pub fn protect(&self) -> Result<Option<SeedProtection>, IdentityError> {
        let Some(on_disk) = self.protection_on_disk()? else {
            return Ok(None);
        };
        if on_disk == SeedProtection::OsKeychain {
            return Ok(Some(SeedProtection::OsKeychain));
        }
        if self.available_protection()? != SeedProtection::OsKeychain {
            return Ok(Some(SeedProtection::PlaintextFile));
        }
        let Some(stored) = self.load()? else {
            return Ok(None);
        };
        let (_, protection) = self.store_phrase(&stored.phrase)?;
        Ok(Some(protection))
    }

    /// Remove the stored seed, and the wrapping key with it. Idempotent, and it
    /// deletes nothing else. Leaving the key behind would leave a keychain record
    /// for an identity that no longer exists.
    pub fn forget(&self) -> Result<bool, IdentityError> {
        let path = self.path();
        self.sweep_temp_files();
        if !path.exists() {
            self.keys.delete();
            return Ok(false);
        }
        fs::remove_file(&path)?;
        self.keys.delete();
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

/// True when the environment asks for the plaintext store. Anything but an
/// explicit off value counts, so `=1`, `=true`, and `=yes` all work and a typo
/// does not silently leave the keychain on when the operator meant it off.
fn plaintext_requested() -> bool {
    match std::env::var(PLAINTEXT_ENV) {
        Ok(value) => {
            let value = value.trim().to_ascii_lowercase();
            !(value.is_empty() || value == "0" || value == "false" || value == "no")
        }
        Err(_) => false,
    }
}
