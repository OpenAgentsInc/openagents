//! NIP-49: Private Key Encryption
//!
//! Implements password-based encryption of private keys using:
//! - Unicode NFKC normalization for passwords
//! - scrypt for key derivation (configurable difficulty)
//! - XChaCha20-Poly1305 AEAD for encryption
//! - bech32 encoding with 'ncryptsec' prefix
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/49.md>

#[cfg(feature = "full")]
use bech32::{Bech32, Hrp};
#[cfg(feature = "full")]
use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead, KeyInit, Payload},
};
#[cfg(feature = "full")]
use rand::RngCore;
#[cfg(feature = "full")]
use scrypt::{Params, scrypt};
use thiserror::Error;
#[cfg(feature = "full")]
use unicode_normalization::UnicodeNormalization;

/// Version number for NIP-49 encryption format
pub const VERSION: u8 = 0x02;

/// Size of salt for scrypt (16 bytes)
pub const SALT_SIZE: usize = 16;

/// Size of nonce for XChaCha20-Poly1305 (24 bytes)
pub const NONCE_SIZE: usize = 24;

/// Size of private key (32 bytes)
pub const PRIVATE_KEY_SIZE: usize = 32;

/// Size of Poly1305 authentication tag (16 bytes)
pub const TAG_SIZE: usize = 16;

/// Total size of encrypted payload before bech32 encoding (91 bytes)
/// VERSION (1) + LOG_N (1) + SALT (16) + NONCE (24) + KEY_SECURITY (1) + CIPHERTEXT (32 + 16)
pub const ENCRYPTED_SIZE: usize = 91;

/// Key security indicators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySecurity {
    /// Key has been handled insecurely (stored unencrypted, etc)
    Insecure = 0x00,
    /// Key has NOT been handled insecurely
    Secure = 0x01,
    /// Client does not track this data
    Unknown = 0x02,
}

impl KeySecurity {
    /// Convert byte to KeySecurity
    pub fn from_byte(b: u8) -> Result<Self, Nip49Error> {
        match b {
            0x00 => Ok(KeySecurity::Insecure),
            0x01 => Ok(KeySecurity::Secure),
            0x02 => Ok(KeySecurity::Unknown),
            _ => Err(Nip49Error::InvalidKeySecurity(b)),
        }
    }

    /// Convert KeySecurity to byte
    pub fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Errors that can occur during NIP-49 operations
#[derive(Debug, Error)]
pub enum Nip49Error {
    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("decryption failed: {0}")]
    Decryption(String),

    #[error("invalid format: {0}")]
    InvalidFormat(String),

    #[error("invalid version: expected 0x02, got {0:#04x}")]
    InvalidVersion(u8),

    #[error("invalid key security byte: {0}")]
    InvalidKeySecurity(u8),

    #[error("invalid log_n: {0}")]
    InvalidLogN(String),

    #[error("bech32 encode error: {0}")]
    Bech32Encode(String),

    #[error("bech32 decode error: {0}")]
    Bech32Decode(String),

    #[error("scrypt error: {0}")]
    Scrypt(String),

    #[error("invalid private key length: expected 32, got {0}")]
    InvalidPrivateKeyLength(usize),
}

/// Normalize password using Unicode NFKC
///
/// This ensures passwords can be entered identically across different systems
#[cfg(feature = "full")]
pub fn normalize_password(password: &str) -> String {
    password.nfkc().collect()
}

/// Derive symmetric encryption key from password using scrypt
///
/// # Arguments
/// * `password` - Password (will be NFKC normalized)
/// * `salt` - 16-byte random salt
/// * `log_n` - Power of 2 for scrypt rounds (e.g., 16 = 2^16 = 65536 rounds)
///
/// # Returns
/// 32-byte symmetric key
#[cfg(feature = "full")]
pub fn derive_key(
    password: &str,
    salt: &[u8; SALT_SIZE],
    log_n: u8,
) -> Result<[u8; 32], Nip49Error> {
    // Normalize password to NFKC
    let normalized = normalize_password(password);

    // Validate log_n (reasonable range: 16-22)
    if !(10..=30).contains(&log_n) {
        return Err(Nip49Error::InvalidLogN(format!(
            "log_n should be between 10 and 30, got {}",
            log_n
        )));
    }

    // Create scrypt parameters
    let params = Params::new(log_n, 8, 1, 32)
        .map_err(|e| Nip49Error::Scrypt(format!("invalid params: {}", e)))?;

    // Derive key
    let mut output = [0u8; 32];
    scrypt(normalized.as_bytes(), salt, &params, &mut output)
        .map_err(|e| Nip49Error::Scrypt(e.to_string()))?;

    Ok(output)
}

/// Encrypt a private key with a password
///
/// # Arguments
/// * `private_key` - 32-byte private key
/// * `password` - Password (will be NFKC normalized)
/// * `log_n` - Power of 2 for scrypt rounds (16-22 recommended)
/// * `key_security` - Indicator of how securely the key has been handled
///
/// # Returns
/// bech32-encoded encrypted private key with 'ncryptsec' prefix
#[cfg(feature = "full")]
pub fn encrypt(
    private_key: &[u8; PRIVATE_KEY_SIZE],
    password: &str,
    log_n: u8,
    key_security: KeySecurity,
) -> Result<String, Nip49Error> {
    // Generate random salt
    let mut salt = [0u8; SALT_SIZE];
    rand::rng().fill_bytes(&mut salt);

    // Derive symmetric key
    let symmetric_key = derive_key(password, &salt, log_n)?;

    // Generate random nonce
    let mut nonce = [0u8; NONCE_SIZE];
    rand::rng().fill_bytes(&mut nonce);

    // Encrypt using XChaCha20-Poly1305
    let cipher = XChaCha20Poly1305::new(&symmetric_key.into());
    let associated_data = [key_security.to_byte()];

    let payload = Payload {
        msg: private_key,
        aad: &associated_data,
    };

    let ciphertext = cipher
        .encrypt(&nonce.into(), payload)
        .map_err(|e| Nip49Error::Encryption(e.to_string()))?;

    // Concatenate: VERSION || LOG_N || SALT || NONCE || KEY_SECURITY || CIPHERTEXT
    let mut output = Vec::with_capacity(ENCRYPTED_SIZE);
    output.push(VERSION);
    output.push(log_n);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.push(key_security.to_byte());
    output.extend_from_slice(&ciphertext);

    // bech32 encode with 'ncryptsec' HRP
    let hrp = Hrp::parse("ncryptsec").map_err(|e| Nip49Error::Bech32Encode(e.to_string()))?;
    let encoded = bech32::encode::<Bech32>(hrp, &output)
        .map_err(|e| Nip49Error::Bech32Encode(e.to_string()))?;

    Ok(encoded)
}

/// Decrypt an encrypted private key
///
/// # Arguments
/// * `encrypted` - bech32-encoded encrypted private key (ncryptsec...)
/// * `password` - Password used for encryption
///
/// # Returns
/// Tuple of (32-byte private key, log_n used, key security indicator)
#[cfg(feature = "full")]
pub fn decrypt(
    encrypted: &str,
    password: &str,
) -> Result<([u8; PRIVATE_KEY_SIZE], u8, KeySecurity), Nip49Error> {
    // Decode bech32
    let (hrp, data) =
        bech32::decode(encrypted).map_err(|e| Nip49Error::Bech32Decode(e.to_string()))?;

    // Verify HRP
    if hrp.to_string() != "ncryptsec" {
        return Err(Nip49Error::Bech32Decode(format!(
            "expected 'ncryptsec' HRP, got '{}'",
            hrp
        )));
    }

    // Verify length
    if data.len() != ENCRYPTED_SIZE {
        return Err(Nip49Error::InvalidFormat(format!(
            "expected {} bytes, got {}",
            ENCRYPTED_SIZE,
            data.len()
        )));
    }

    // Parse components
    let version = data[0];
    let log_n = data[1];
    let salt: [u8; SALT_SIZE] = data[2..2 + SALT_SIZE].try_into().unwrap();
    let nonce: [u8; NONCE_SIZE] = data[2 + SALT_SIZE..2 + SALT_SIZE + NONCE_SIZE]
        .try_into()
        .unwrap();
    let key_security_byte = data[2 + SALT_SIZE + NONCE_SIZE];
    let ciphertext = &data[2 + SALT_SIZE + NONCE_SIZE + 1..];

    // Verify version
    if version != VERSION {
        return Err(Nip49Error::InvalidVersion(version));
    }

    // Parse key security
    let key_security = KeySecurity::from_byte(key_security_byte)?;

    // Derive symmetric key
    let symmetric_key = derive_key(password, &salt, log_n)?;

    // Decrypt using XChaCha20-Poly1305
    let cipher = XChaCha20Poly1305::new(&symmetric_key.into());
    let associated_data = [key_security_byte];

    let payload = Payload {
        msg: ciphertext,
        aad: &associated_data,
    };

    let plaintext = cipher
        .decrypt(&nonce.into(), payload)
        .map_err(|e| Nip49Error::Decryption(e.to_string()))?;

    // Verify plaintext length
    if plaintext.len() != PRIVATE_KEY_SIZE {
        return Err(Nip49Error::InvalidPrivateKeyLength(plaintext.len()));
    }

    let mut private_key = [0u8; PRIVATE_KEY_SIZE];
    private_key.copy_from_slice(&plaintext);

    Ok((private_key, log_n, key_security))
}

#[cfg(all(test, feature = "full"))]
mod tests {
    use super::*;

    #[test]
    fn test_password_normalization() {
        // Test case from NIP-49 spec
        let input = "ÅΩẛ̣";
        let normalized = normalize_password(input);
        // The normalized form should be different from input
        // This test verifies the function runs without panic
        assert!(!normalized.is_empty());
    }

    #[test]
    fn test_key_security_conversion() {
        assert_eq!(KeySecurity::Insecure.to_byte(), 0x00);
        assert_eq!(KeySecurity::Secure.to_byte(), 0x01);
        assert_eq!(KeySecurity::Unknown.to_byte(), 0x02);

        assert_eq!(KeySecurity::from_byte(0x00).unwrap(), KeySecurity::Insecure);
        assert_eq!(KeySecurity::from_byte(0x01).unwrap(), KeySecurity::Secure);
        assert_eq!(KeySecurity::from_byte(0x02).unwrap(), KeySecurity::Unknown);

        assert!(KeySecurity::from_byte(0x03).is_err());
    }

    #[test]
    fn test_derive_key() {
        let password = "test_password";
        let salt = [0u8; SALT_SIZE];
        let log_n = 16;

        let key1 = derive_key(password, &salt, log_n).unwrap();
        let key2 = derive_key(password, &salt, log_n).unwrap();

        // Same inputs should produce same key
        assert_eq!(key1, key2);

        // Different salt should produce different key
        let salt2 = [1u8; SALT_SIZE];
        let key3 = derive_key(password, &salt2, log_n).unwrap();
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let private_key = [0x42u8; PRIVATE_KEY_SIZE];
        let password = "my_secure_password";
        let log_n = 16;

        let encrypted = encrypt(&private_key, password, log_n, KeySecurity::Secure).unwrap();

        // Verify it starts with ncryptsec
        assert!(encrypted.starts_with("ncryptsec1"));

        let (decrypted, recovered_log_n, recovered_security) =
            decrypt(&encrypted, password).unwrap();

        assert_eq!(decrypted, private_key);
        assert_eq!(recovered_log_n, log_n);
        assert_eq!(recovered_security, KeySecurity::Secure);
    }

    #[test]
    fn test_decrypt_nip49_test_vector() {
        // Test vector from NIP-49 spec
        let encrypted = "ncryptsec1qgg9947rlpvqu76pj5ecreduf9jxhselq2nae2kghhvd5g7dgjtcxfqtd67p9m0w57lspw8gsq6yphnm8623nsl8xn9j4jdzz84zm3frztj3z7s35vpzmqf6ksu8r89qk5z2zxfmu5gv8th8wclt0h4p";
        let password = "nostr";

        let (private_key, log_n, _) = decrypt(encrypted, password).unwrap();

        // Expected private key from spec
        let expected_hex = "3501454135014541350145413501453fefb02227e449e57cf4d3a3ce05378683";
        let expected_key = hex::decode(expected_hex).unwrap();

        assert_eq!(private_key.as_slice(), expected_key.as_slice());
        assert_eq!(log_n, 16);
    }

    #[test]
    fn test_decrypt_wrong_password() {
        let private_key = [0x42u8; PRIVATE_KEY_SIZE];
        let password = "correct_password";
        let log_n = 16;

        let encrypted = encrypt(&private_key, password, log_n, KeySecurity::Unknown).unwrap();

        let result = decrypt(&encrypted, "wrong_password");
        assert!(result.is_err());
    }

    #[test]
    fn test_different_key_security_levels() {
        let private_key = [0x42u8; PRIVATE_KEY_SIZE];
        let password = "password";
        let log_n = 16;

        for &security in &[
            KeySecurity::Insecure,
            KeySecurity::Secure,
            KeySecurity::Unknown,
        ] {
            let encrypted = encrypt(&private_key, password, log_n, security).unwrap();
            let (_, _, recovered_security) = decrypt(&encrypted, password).unwrap();
            assert_eq!(recovered_security, security);
        }
    }

    #[test]
    fn test_invalid_log_n() {
        let private_key = [0x42u8; PRIVATE_KEY_SIZE];
        let password = "password";

        // Too low
        let result = encrypt(&private_key, password, 5, KeySecurity::Unknown);
        assert!(result.is_err());

        // Too high
        let result = encrypt(&private_key, password, 35, KeySecurity::Unknown);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonce_randomness() {
        let private_key = [0x42u8; PRIVATE_KEY_SIZE];
        let password = "password";
        let log_n = 16;

        let encrypted1 = encrypt(&private_key, password, log_n, KeySecurity::Secure).unwrap();
        let encrypted2 = encrypt(&private_key, password, log_n, KeySecurity::Secure).unwrap();

        // Same inputs should produce different ciphertexts due to random nonce
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to same plaintext
        let (decrypted1, _, _) = decrypt(&encrypted1, password).unwrap();
        let (decrypted2, _, _) = decrypt(&encrypted2, password).unwrap();
        assert_eq!(decrypted1, decrypted2);
        assert_eq!(decrypted1, private_key);
    }
}
