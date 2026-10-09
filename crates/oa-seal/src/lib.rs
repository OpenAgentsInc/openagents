//! Encryption at rest for small server-held secrets (#11041).
//!
//! A [`Keyring`] holds one or more named 256-bit keys and says which one is
//! current. It is read from a private file or a secret the platform injects,
//! never from the directory the sealed records live in, so a copy of that
//! directory (a disk snapshot, a backup, a stolen volume) holds only
//! ciphertext.
//!
//! [`Keyring::seal`] encrypts with AES-256-GCM under the current key and a
//! fresh random 96-bit nonce, binding the caller's associated data (the
//! record's own metadata) so that changing any of it makes
//! [`Keyring::open`] fail. Each [`Sealed`] names the key it was made with,
//! so keys rotate: add a new key, make it current, and records sealed under
//! an older key still open and are resealed under the new one when the
//! caller next touches them ([`Keyring::is_current`]). A record whose key
//! is unknown, wrong, or whose bytes changed fails closed.
//!
//! Keys are zeroed when the keyring drops and never print.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, Zeroizing};

/// The keyring document's schema.
pub const KEYRING_SCHEMA: &str = "openagents.seal.keyring.v1";
const DOMAIN: &[u8] = b"openagents.seal.v1\0";
const FILE_MAX: u64 = 16 * 1024;
const ID_MAX: usize = 64;

/// Why sealing or opening failed. Deliberately coarse: a caller learns
/// only that the record cannot be trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SealError {
    /// The record names a key this keyring does not hold.
    UnknownKey,
    /// The record is malformed, was sealed under another key with the same
    /// name, or its bytes or associated data changed.
    Rejected,
    /// The system random source failed.
    Random,
}

impl std::fmt::Display for SealError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::UnknownKey => "The record was sealed under a key that is not loaded.",
            Self::Rejected => "The record could not be opened.",
            Self::Random => "The system random source failed.",
        })
    }
}

impl std::error::Error for SealError {}

/// One sealed value: the key it was made with, its nonce, and the
/// ciphertext with its 128-bit tag (both base64).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sealed {
    pub key_id: String,
    pub nonce: String,
    pub ciphertext: String,
}

/// Named 256-bit keys and the one new records use.
pub struct Keyring {
    current: String,
    keys: BTreeMap<String, Zeroizing<[u8; 32]>>,
    source: Option<PathBuf>,
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Keyring")
            .field("current", &self.current)
            .field("keys", &self.keys.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    schema: String,
    current: String,
    keys: BTreeMap<String, String>,
}

impl Drop for Document {
    fn drop(&mut self) {
        for value in self.keys.values_mut() {
            value.zeroize();
        }
    }
}

impl Keyring {
    /// Parse a keyring document:
    /// `{"schema":"openagents.seal.keyring.v1","current":"ID","keys":{"ID":"<32 bytes, base64>"}}`.
    pub fn parse(text: &[u8]) -> Result<Self, String> {
        let document: Document =
            serde_json::from_slice(text).map_err(|_| "The keyring is not a keyring document.")?;
        if document.schema != KEYRING_SCHEMA {
            return Err(format!("The keyring schema must be {KEYRING_SCHEMA}."));
        }
        let mut keys = BTreeMap::new();
        for (id, value) in &document.keys {
            if !valid_id(id) {
                return Err(
                    "Key ids are 1 to 64 letters, digits, dots, dashes, or underscores.".into(),
                );
            }
            let mut bytes = Zeroizing::new(
                STANDARD
                    .decode(value.trim())
                    .map_err(|_| "Each key must be 32 bytes of base64.")?,
            );
            let mut key = Zeroizing::new([0u8; 32]);
            if bytes.len() != 32 {
                return Err("Each key must be 32 bytes of base64.".into());
            }
            key.copy_from_slice(&bytes);
            bytes.zeroize();
            if key.iter().all(|byte| *byte == 0) {
                return Err("A key of all zeros is refused.".into());
            }
            keys.insert(id.clone(), key);
        }
        if !keys.contains_key(&document.current) {
            return Err("The current key id names no key in the keyring.".into());
        }
        Ok(Self {
            current: document.current.clone(),
            keys,
            source: None,
        })
    }

    /// Read a keyring file. It must be a small regular file (not a symbolic
    /// link) owned by this user and readable by its owner only (mode 0600).
    pub fn load(path: &Path) -> Result<Self, String> {
        let unavailable = || "The keyring file is unavailable.".to_owned();
        let meta = std::fs::symlink_metadata(path).map_err(|_| unavailable())?;
        if !meta.is_file() || meta.len() > FILE_MAX {
            return Err("The keyring file must be a small regular file.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // SAFETY: geteuid has no preconditions and cannot fail.
            if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
                return Err(
                    "The keyring file must be owned by this user and readable by its owner only (chmod 600)."
                        .into(),
                );
            }
        }
        let mut text = Zeroizing::new(Vec::new());
        std::fs::File::open(path)
            .and_then(|file| file.take(FILE_MAX + 1).read_to_end(&mut text))
            .map_err(|_| unavailable())?;
        let mut keyring = Self::parse(&text)?;
        keyring.source = Some(path.canonicalize().map_err(|_| unavailable())?);
        Ok(keyring)
    }

    /// A fresh one-key keyring and its document, for local fixtures and
    /// tests. Never use a scratch keyring for real records: it is not kept.
    pub fn scratch(id: &str) -> Result<(Self, Zeroizing<String>), String> {
        if !valid_id(id) {
            return Err(
                "Key ids are 1 to 64 letters, digits, dots, dashes, or underscores.".into(),
            );
        }
        let mut key = Zeroizing::new([0u8; 32]);
        getrandom::fill(key.as_mut()).map_err(|_| "The system random source failed.")?;
        let document = Zeroizing::new(
            serde_json::json!({
                "schema": KEYRING_SCHEMA,
                "current": id,
                "keys": { id: STANDARD.encode(key.as_ref()) },
            })
            .to_string(),
        );
        Ok((Self::parse(document.as_bytes())?, document))
    }

    /// The id new records are sealed under.
    #[must_use]
    pub fn current(&self) -> &str {
        &self.current
    }

    /// The canonical path the keyring was loaded from, if it came from a file.
    #[must_use]
    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    /// Whether `sealed` already uses the current key.
    #[must_use]
    pub fn is_current(&self, sealed: &Sealed) -> bool {
        sealed.key_id == self.current
    }

    /// Encrypt `plaintext` under the current key, bound to `associated`.
    pub fn seal(&self, associated: &[u8], plaintext: &[u8]) -> Result<Sealed, SealError> {
        let key = self.keys.get(&self.current).ok_or(SealError::UnknownKey)?;
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| SealError::Random)?;
        let aad = bound(&self.current, associated);
        let ciphertext = cipher(key)
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| SealError::Rejected)?;
        Ok(Sealed {
            key_id: self.current.clone(),
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
        })
    }

    /// Decrypt `sealed`, which must have been made with the same associated
    /// data. The plaintext is zeroed when the returned buffer drops.
    pub fn open(
        &self,
        associated: &[u8],
        sealed: &Sealed,
    ) -> Result<Zeroizing<Vec<u8>>, SealError> {
        let key = self.keys.get(&sealed.key_id).ok_or(SealError::UnknownKey)?;
        let nonce = STANDARD
            .decode(&sealed.nonce)
            .map_err(|_| SealError::Rejected)?;
        if nonce.len() != 12 {
            return Err(SealError::Rejected);
        }
        let ciphertext = STANDARD
            .decode(&sealed.ciphertext)
            .map_err(|_| SealError::Rejected)?;
        let aad = bound(&sealed.key_id, associated);
        cipher(key)
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| SealError::Rejected)
    }
}

fn cipher(key: &[u8; 32]) -> Aes256Gcm {
    Aes256Gcm::new(key.into())
}

/// The domain, the key id, and the caller's data, so a record cannot be
/// relabelled to another key or reused by another kind of sealed value.
fn bound(key_id: &str, associated: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(DOMAIN.len() + key_id.len() + 1 + associated.len());
    aad.extend_from_slice(DOMAIN);
    aad.extend_from_slice(key_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(associated);
    aad
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ID_MAX
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"sk-ant-api03-synthetic-seal-test";

    fn ring(current: &str, keys: &[(&str, u8)]) -> Keyring {
        let keys: BTreeMap<_, _> = keys
            .iter()
            .map(|(id, fill)| (id.to_string(), STANDARD.encode([*fill; 32])))
            .collect();
        Keyring::parse(
            serde_json::json!({"schema":KEYRING_SCHEMA,"current":current,"keys":keys})
                .to_string()
                .as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn round_trips_without_plaintext_in_the_record() {
        let ring = ring("k1", &[("k1", 7)]);
        let sealed = ring.seal(b"meta", SECRET).unwrap();
        assert_eq!(sealed.key_id, "k1");
        let text = serde_json::to_string(&sealed).unwrap();
        assert!(!text.contains("sk-ant"));
        assert_eq!(ring.open(b"meta", &sealed).unwrap().as_slice(), SECRET);
        // Fresh nonces: sealing twice never repeats.
        assert_ne!(ring.seal(b"meta", SECRET).unwrap(), sealed);
    }

    #[test]
    fn a_wrong_or_missing_key_fails_closed() {
        let sealed = ring("k1", &[("k1", 7)]).seal(b"meta", SECRET).unwrap();
        // Same name, different bytes.
        assert_eq!(
            ring("k1", &[("k1", 8)]).open(b"meta", &sealed).unwrap_err(),
            SealError::Rejected
        );
        // The key is not loaded at all.
        assert_eq!(
            ring("k2", &[("k2", 7)]).open(b"meta", &sealed).unwrap_err(),
            SealError::UnknownKey
        );
    }

    #[test]
    fn tampering_with_bytes_metadata_or_key_label_is_detected() {
        let ring = ring("k1", &[("k1", 7), ("k2", 7)]);
        let sealed = ring.seal(b"meta", SECRET).unwrap();
        assert_eq!(
            ring.open(b"other", &sealed).unwrap_err(),
            SealError::Rejected
        );
        let mut bytes = STANDARD.decode(&sealed.ciphertext).unwrap();
        bytes[0] ^= 1;
        let flipped = Sealed {
            ciphertext: STANDARD.encode(&bytes),
            ..sealed.clone()
        };
        assert_eq!(
            ring.open(b"meta", &flipped).unwrap_err(),
            SealError::Rejected
        );
        // Relabelled to another key with identical bytes: still refused,
        // because the key id is bound.
        let relabelled = Sealed {
            key_id: "k2".into(),
            ..sealed.clone()
        };
        assert_eq!(
            ring.open(b"meta", &relabelled).unwrap_err(),
            SealError::Rejected
        );
        let short = Sealed {
            nonce: STANDARD.encode([0u8; 8]),
            ..sealed
        };
        assert_eq!(ring.open(b"meta", &short).unwrap_err(), SealError::Rejected);
    }

    #[test]
    fn rotation_opens_old_records_and_seals_new_ones_under_the_current_key() {
        let old = ring("k1", &[("k1", 7)]);
        let sealed = old.seal(b"meta", SECRET).unwrap();
        let rotated = ring("k2", &[("k1", 7), ("k2", 9)]);
        assert!(!rotated.is_current(&sealed));
        assert_eq!(rotated.open(b"meta", &sealed).unwrap().as_slice(), SECRET);
        let resealed = rotated.seal(b"meta", SECRET).unwrap();
        assert!(rotated.is_current(&resealed));
        assert_eq!(
            old.open(b"meta", &resealed).unwrap_err(),
            SealError::UnknownKey
        );
    }

    #[test]
    fn refuses_malformed_keyrings_and_shared_files() {
        for bad in [
            r#"{"schema":"x","current":"k","keys":{"k":"AAAA"}}"#.to_owned(),
            format!(r#"{{"schema":"{KEYRING_SCHEMA}","current":"k","keys":{{"k":"AAAA"}}}}"#),
            format!(
                r#"{{"schema":"{KEYRING_SCHEMA}","current":"missing","keys":{{"k":"{}"}}}}"#,
                STANDARD.encode([1u8; 32])
            ),
            format!(
                r#"{{"schema":"{KEYRING_SCHEMA}","current":"k","keys":{{"k":"{}"}}}}"#,
                STANDARD.encode([0u8; 32])
            ),
            format!(
                r#"{{"schema":"{KEYRING_SCHEMA}","current":"a/b","keys":{{"a/b":"{}"}}}}"#,
                STANDARD.encode([1u8; 32])
            ),
        ] {
            assert!(Keyring::parse(bad.as_bytes()).is_err(), "{bad}");
        }
        let (scratch, document) = Keyring::scratch("scratch").unwrap();
        assert_eq!(scratch.current(), "scratch");
        let parsed: serde_json::Value = serde_json::from_str(&document).unwrap();
        let encoded = parsed["keys"]["scratch"].as_str().unwrap();
        assert!(!format!("{scratch:?}").contains(encoded));
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("keys.json");
        std::fs::write(&path, document.as_bytes()).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Keyring::load(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let loaded = Keyring::load(&path).unwrap();
        let sealed = scratch.seal(b"m", SECRET).unwrap();
        assert_eq!(loaded.open(b"m", &sealed).unwrap().as_slice(), SECRET);
        assert!(loaded.source().is_some());
        let link = temp.path().join("link.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(Keyring::load(&link).is_err());
    }
}
