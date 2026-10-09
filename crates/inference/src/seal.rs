//! Sealing for what the gateway keeps or hands back encrypted: stored
//! responses (`store: true`) and compaction items.
//!
//! AES-256-GCM with a random 96-bit nonce per seal. Every seal binds
//! additional data naming its owner and purpose, so a blob sealed for one
//! tenant (or one response id) does not open for another, even with the
//! same key. A sealed blob reads `oa1.<base64url(nonce || ciphertext)>`.
//!
//! The key is 32 bytes, from the environment (base64, or a mounted file)
//! or a key file the gateway creates in its state directory with mode
//! 0600 on first use. It never appears in an error or a log line.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};

/// The prefix of a sealed blob; the version of this format.
pub const PREFIX: &str = "oa1.";

/// A sealing key.
pub struct Sealer {
    key: LessSafeKey,
    random: SystemRandom,
}

impl std::fmt::Debug for Sealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sealer(..)")
    }
}

impl Sealer {
    /// A sealer on these 32 key bytes.
    ///
    /// # Errors
    ///
    /// A sentence when the key is not 32 bytes.
    pub fn new(key: &[u8]) -> Result<Self, String> {
        let unbound = UnboundKey::new(&AES_256_GCM, key)
            .map_err(|_| "a sealing key must be 32 bytes".to_owned())?;
        Ok(Self {
            key: LessSafeKey::new(unbound),
            random: SystemRandom::new(),
        })
    }

    /// A sealer on a fresh random key, and the key's bytes.
    ///
    /// # Errors
    ///
    /// A sentence when the system has no randomness to give.
    pub fn generate() -> Result<(Self, [u8; 32]), String> {
        let mut key = [0u8; 32];
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| "no randomness for a sealing key".to_owned())?;
        Ok((Self::new(&key)?, key))
    }

    /// A sealer on a base64 key (standard or URL-safe alphabet).
    ///
    /// # Errors
    ///
    /// A sentence when the text is not base64 of 32 bytes.
    pub fn from_base64(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let bytes = STANDARD
            .decode(text)
            .or_else(|_| URL_SAFE_NO_PAD.decode(text.trim_end_matches('=')))
            .map_err(|_| "the sealing key is not base64".to_owned())?;
        Self::new(&bytes)
    }

    /// The key from `var` (base64) or the file `var_FILE` names; else the
    /// key file at `path`, created with a fresh key (mode 0600) when it
    /// does not exist.
    ///
    /// # Errors
    ///
    /// A sentence when a configured key is malformed or the key file
    /// cannot be read or written.
    pub fn from_env_or_file(var: &str, path: &Path) -> Result<Self, String> {
        if let Ok(value) = std::env::var(var)
            && !value.trim().is_empty()
        {
            return Self::from_base64(&value);
        }
        if let Ok(file) = std::env::var(format!("{var}_FILE"))
            && !file.trim().is_empty()
        {
            let text = std::fs::read_to_string(file.trim())
                .map_err(|_| format!("{var}_FILE can't be read"))?;
            return Self::from_base64(&text);
        }
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_base64(&text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let (sealer, key) = Self::generate()?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|_| "the sealing key's folder can't be made".to_owned())?;
                }
                write_private(path, STANDARD.encode(key).as_bytes())
                    .map_err(|_| "the sealing key file can't be written".to_owned())?;
                Ok(sealer)
            }
            Err(_) => Err("the sealing key file can't be read".to_owned()),
        }
    }

    /// `plaintext` sealed, bound to `aad`.
    ///
    /// # Errors
    ///
    /// A sentence when there is no randomness for a nonce or the cipher
    /// refuses (neither happens in practice).
    pub fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<String, String> {
        let mut nonce = [0u8; NONCE_LEN];
        self.random
            .fill(&mut nonce)
            .map_err(|_| "no randomness for a nonce".to_owned())?;
        let mut sealed = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(aad),
                &mut sealed,
            )
            .map_err(|_| "sealing failed".to_owned())?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&sealed);
        Ok(format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(blob)))
    }

    /// The plaintext of a blob sealed with this key and `aad`, or `None`
    /// when it was not (another key, another owner, or tampered with).
    #[must_use]
    pub fn open(&self, aad: &[u8], blob: &str) -> Option<Vec<u8>> {
        let bytes = URL_SAFE_NO_PAD.decode(blob.strip_prefix(PREFIX)?).ok()?;
        if bytes.len() < NONCE_LEN {
            return None;
        }
        let (nonce, sealed) = bytes.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).ok()?;
        let mut sealed = sealed.to_vec();
        let plain = self
            .key
            .open_in_place(nonce, Aad::from(aad), &mut sealed)
            .ok()?;
        Some(plain.to_vec())
    }
}

/// Writes `bytes` to a new file only its owner can read.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// A random id with `prefix`: `resp_` and 32 hex characters.
#[must_use]
pub fn random_id(prefix: &str) -> String {
    let mut bytes = [0u8; 16];
    if SystemRandom::new().fill(&mut bytes).is_err() {
        // No randomness: fall back to the clock, still unique enough for
        // an id that is never a secret.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_nanos())
            .unwrap_or_default();
        bytes = now.to_le_bytes();
    }
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{prefix}{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seal_opens_only_with_its_key_and_its_owner() {
        let (sealer, _) = Sealer::generate().unwrap();
        let blob = sealer.seal(b"tenant-a", b"hello").unwrap();
        assert!(blob.starts_with(PREFIX));
        assert_eq!(sealer.open(b"tenant-a", &blob).unwrap(), b"hello");
        assert!(sealer.open(b"tenant-b", &blob).is_none());
        let (other, _) = Sealer::generate().unwrap();
        assert!(other.open(b"tenant-a", &blob).is_none());
        let mut tampered = blob.clone();
        tampered.push('A');
        assert!(sealer.open(b"tenant-a", &tampered).is_none());
        assert!(sealer.open(b"tenant-a", "plain").is_none());
    }

    #[test]
    fn the_key_file_is_made_once_and_reused() {
        let dir = std::env::temp_dir().join(random_id("seal-test-"));
        let path = dir.join("key");
        let first = Sealer::from_env_or_file("SEAL_TEST_UNSET_VAR", &path).unwrap();
        let blob = first.seal(b"x", b"kept").unwrap();
        let second = Sealer::from_env_or_file("SEAL_TEST_UNSET_VAR", &path).unwrap();
        assert_eq!(second.open(b"x", &blob).unwrap(), b"kept");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ids_are_prefixed_and_distinct() {
        let a = random_id("resp_");
        let b = random_id("resp_");
        assert!(a.starts_with("resp_"));
        assert_eq!(a.len(), 5 + 32);
        assert_ne!(a, b);
    }
}
