//! NIP-04: Encrypted Direct Message
//!
//! **WARNING**: This NIP is deprecated in favor of NIP-17. It has known security
//! issues and leaks metadata. Only implement for backwards compatibility.
//!
//! Implements encrypted direct messages (kind 4) using:
//! - ECDH for shared secret derivation (X coordinate only, not hashed)
//! - AES-256-CBC encryption with random IV
//! - Base64 encoding with format: `<encrypted>?iv=<iv_base64>`
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/04.md>

#[cfg(feature = "full")]
use aes::Aes256;
#[cfg(feature = "full")]
use bitcoin::secp256k1::{PublicKey, SecretKey, ecdh::SharedSecret};
#[cfg(feature = "full")]
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
#[cfg(feature = "full")]
use cbc::{Decryptor, Encryptor};
#[cfg(feature = "full")]
use rand::RngCore;
use thiserror::Error;

#[cfg(feature = "full")]
use base64::Engine;

/// Event kind for encrypted direct messages
pub const ENCRYPTED_DM_KIND: u16 = 4;

/// Errors that can occur during NIP-04 operations
#[derive(Debug, Error)]
pub enum Nip04Error {
    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("decryption failed: {0}")]
    Decryption(String),

    #[error("invalid format: {0}")]
    InvalidFormat(String),

    #[error("base64 decode error: {0}")]
    Base64Decode(String),

    #[error("invalid key: {0}")]
    InvalidKey(String),

    #[error("padding error: {0}")]
    Padding(String),
}

#[cfg(feature = "full")]
type Aes256CbcEnc = Encryptor<Aes256>;
#[cfg(feature = "full")]
type Aes256CbcDec = Decryptor<Aes256>;

/// Encrypt a message using NIP-04
///
/// # Arguments
/// * `sender_privkey` - Sender's private key (32 bytes)
/// * `recipient_pubkey` - Recipient's public key (33 bytes compressed or 65 bytes uncompressed)
/// * `plaintext` - The message to encrypt
///
/// # Returns
/// Encrypted content in format: `<base64_encrypted>?iv=<base64_iv>`
///
/// # Example
/// ```ignore
/// let encrypted = encrypt(&sender_sk, &recipient_pk, "Hello!")?;
/// // Returns something like: "Rb9E...?iv=MTIz..."
/// ```
#[cfg(feature = "full")]
pub fn encrypt(
    sender_privkey: &[u8; 32],
    recipient_pubkey: &[u8],
    plaintext: &str,
) -> Result<String, Nip04Error> {
    // Parse keys
    let secret_key =
        SecretKey::from_slice(sender_privkey).map_err(|e| Nip04Error::InvalidKey(e.to_string()))?;

    let public_key = PublicKey::from_slice(recipient_pubkey)
        .map_err(|e| Nip04Error::InvalidKey(e.to_string()))?;

    // Generate shared secret using ECDH
    // NIP-04 uses only the X coordinate (first 32 bytes after the prefix byte)
    let shared_secret = SharedSecret::new(&public_key, &secret_key);
    let shared_x = shared_secret.as_ref();

    // Generate random IV (16 bytes for AES)
    let mut iv = [0u8; 16];
    rand::rng().fill_bytes(&mut iv);

    // Pad plaintext to AES block size (16 bytes) using PKCS#7
    let plaintext_bytes = plaintext.as_bytes();
    let padding_len = 16 - (plaintext_bytes.len() % 16);
    let mut padded = plaintext_bytes.to_vec();
    padded.extend(vec![padding_len as u8; padding_len]);

    // Encrypt using AES-256-CBC
    let cipher = Aes256CbcEnc::new(shared_x.into(), &iv.into());
    let ciphertext =
        cipher.encrypt_padded_vec_mut::<cbc::cipher::block_padding::NoPadding>(&padded);

    // Encode to base64
    let encrypted_b64 = base64::engine::general_purpose::STANDARD.encode(&ciphertext);
    let iv_b64 = base64::engine::general_purpose::STANDARD.encode(iv);

    Ok(format!("{}?iv={}", encrypted_b64, iv_b64))
}

/// Decrypt a message using NIP-04
///
/// # Arguments
/// * `recipient_privkey` - Recipient's private key (32 bytes)
/// * `sender_pubkey` - Sender's public key (33 bytes compressed or 65 bytes uncompressed)
/// * `ciphertext` - Encrypted content in format: `<base64_encrypted>?iv=<base64_iv>`
///
/// # Returns
/// Decrypted plaintext message
///
/// # Example
/// ```ignore
/// let plaintext = decrypt(&recipient_sk, &sender_pk, "Rb9E...?iv=MTIz...")?;
/// // Returns: "Hello!"
/// ```
#[cfg(feature = "full")]
pub fn decrypt(
    recipient_privkey: &[u8; 32],
    sender_pubkey: &[u8],
    ciphertext: &str,
) -> Result<String, Nip04Error> {
    // Parse the ciphertext format: <encrypted>?iv=<iv>
    let parts: Vec<&str> = ciphertext.split("?iv=").collect();
    if parts.len() != 2 {
        return Err(Nip04Error::InvalidFormat(
            "expected format: <encrypted>?iv=<iv>".to_string(),
        ));
    }

    let encrypted_b64 = parts[0];
    let iv_b64 = parts[1];

    // Decode base64
    let encrypted_bytes = base64::engine::general_purpose::STANDARD
        .decode(encrypted_b64)
        .map_err(|e| Nip04Error::Base64Decode(e.to_string()))?;
    let iv = base64::engine::general_purpose::STANDARD
        .decode(iv_b64)
        .map_err(|e| Nip04Error::Base64Decode(e.to_string()))?;

    if iv.len() != 16 {
        return Err(Nip04Error::InvalidFormat(format!(
            "IV must be 16 bytes, got {}",
            iv.len()
        )));
    }

    // Parse keys
    let secret_key = SecretKey::from_slice(recipient_privkey)
        .map_err(|e| Nip04Error::InvalidKey(e.to_string()))?;

    let public_key =
        PublicKey::from_slice(sender_pubkey).map_err(|e| Nip04Error::InvalidKey(e.to_string()))?;

    // Generate shared secret using ECDH (same as encryption)
    let shared_secret = SharedSecret::new(&public_key, &secret_key);
    let shared_x = shared_secret.as_ref();

    // Decrypt using AES-256-CBC
    let mut iv_array = [0u8; 16];
    iv_array.copy_from_slice(&iv);

    let cipher = Aes256CbcDec::new(shared_x.into(), &iv_array.into());
    let mut buffer = encrypted_bytes;
    let decrypted = cipher
        .decrypt_padded_mut::<cbc::cipher::block_padding::NoPadding>(&mut buffer)
        .map_err(|e| Nip04Error::Decryption(e.to_string()))?;

    // Remove PKCS#7 padding
    if decrypted.is_empty() {
        return Err(Nip04Error::Padding("empty decrypted data".to_string()));
    }

    let padding_len = decrypted[decrypted.len() - 1] as usize;
    if padding_len == 0 || padding_len > 16 || padding_len > decrypted.len() {
        return Err(Nip04Error::Padding(format!(
            "invalid padding length: {}",
            padding_len
        )));
    }

    // Verify padding
    for i in 0..padding_len {
        if decrypted[decrypted.len() - 1 - i] != padding_len as u8 {
            return Err(Nip04Error::Padding("invalid padding bytes".to_string()));
        }
    }

    let plaintext_bytes = &decrypted[..decrypted.len() - padding_len];
    let plaintext = String::from_utf8(plaintext_bytes.to_vec())
        .map_err(|e| Nip04Error::Decryption(format!("invalid UTF-8: {}", e)))?;

    Ok(plaintext)
}

#[cfg(all(test, feature = "full"))]
mod tests {
    use super::*;
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secp = Secp256k1::new();

        // Generate sender keys
        let sender_sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let sender_pk = PublicKey::from_secret_key(&secp, &sender_sk);

        // Generate recipient keys
        let recipient_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let recipient_pk = PublicKey::from_secret_key(&secp, &recipient_sk);

        let message = "Hello, Nostr!";

        // Encrypt
        let encrypted = encrypt(
            &sender_sk.secret_bytes(),
            &recipient_pk.serialize(),
            message,
        )
        .expect("encryption should succeed");

        // Verify format
        assert!(encrypted.contains("?iv="));

        // Decrypt
        let decrypted = decrypt(
            &recipient_sk.secret_bytes(),
            &sender_pk.serialize(),
            &encrypted,
        )
        .expect("decryption should succeed");

        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_encrypt_decrypt_empty_message() {
        let secp = Secp256k1::new();

        let sender_sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let sender_pk = PublicKey::from_secret_key(&secp, &sender_sk);

        let recipient_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let recipient_pk = PublicKey::from_secret_key(&secp, &recipient_sk);

        let message = "";

        let encrypted = encrypt(
            &sender_sk.secret_bytes(),
            &recipient_pk.serialize(),
            message,
        )
        .expect("encryption should succeed");

        let decrypted = decrypt(
            &recipient_sk.secret_bytes(),
            &sender_pk.serialize(),
            &encrypted,
        )
        .expect("decryption should succeed");

        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_encrypt_decrypt_long_message() {
        let secp = Secp256k1::new();

        let sender_sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let sender_pk = PublicKey::from_secret_key(&secp, &sender_sk);

        let recipient_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let recipient_pk = PublicKey::from_secret_key(&secp, &recipient_sk);

        let message = "This is a much longer message that spans multiple AES blocks to test the encryption and decryption with padding across block boundaries.";

        let encrypted = encrypt(
            &sender_sk.secret_bytes(),
            &recipient_pk.serialize(),
            message,
        )
        .expect("encryption should succeed");

        let decrypted = decrypt(
            &recipient_sk.secret_bytes(),
            &sender_pk.serialize(),
            &encrypted,
        )
        .expect("decryption should succeed");

        assert_eq!(decrypted, message);
    }

    #[test]
    fn test_decrypt_invalid_format() {
        let recipient_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let sender_pk_bytes = [3u8; 33];

        let result = decrypt(
            &recipient_sk.secret_bytes(),
            &sender_pk_bytes,
            "invalid_format",
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_iv_length() {
        let recipient_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let sender_pk_bytes = [3u8; 33];

        // Valid base64 but IV is wrong length
        let result = decrypt(
            &recipient_sk.secret_bytes(),
            &sender_pk_bytes,
            "dGVzdA==?iv=dGVzdA==", // "test" base64 encoded (4 bytes, not 16)
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_different_keys_same_message() {
        let secp = Secp256k1::new();

        // First pair
        let sender1_sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let sender1_pk = PublicKey::from_secret_key(&secp, &sender1_sk);
        let recipient1_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let recipient1_pk = PublicKey::from_secret_key(&secp, &recipient1_sk);

        // Second pair
        let sender2_sk = SecretKey::from_slice(&[3u8; 32]).unwrap();
        let sender2_pk = PublicKey::from_secret_key(&secp, &sender2_sk);
        let recipient2_sk = SecretKey::from_slice(&[4u8; 32]).unwrap();
        let recipient2_pk = PublicKey::from_secret_key(&secp, &recipient2_sk);

        let message = "Same message";

        let encrypted1 = encrypt(
            &sender1_sk.secret_bytes(),
            &recipient1_pk.serialize(),
            message,
        )
        .unwrap();

        let encrypted2 = encrypt(
            &sender2_sk.secret_bytes(),
            &recipient2_pk.serialize(),
            message,
        )
        .unwrap();

        // Different key pairs should produce different ciphertexts
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same message
        let decrypted1 = decrypt(
            &recipient1_sk.secret_bytes(),
            &sender1_pk.serialize(),
            &encrypted1,
        )
        .unwrap();

        let decrypted2 = decrypt(
            &recipient2_sk.secret_bytes(),
            &sender2_pk.serialize(),
            &encrypted2,
        )
        .unwrap();

        assert_eq!(decrypted1, message);
        assert_eq!(decrypted2, message);
    }
}
