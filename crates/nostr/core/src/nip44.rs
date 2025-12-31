//! NIP-44: Versioned Encryption
//!
//! This module implements NIP-44, which defines a versioned encryption format for keypair-based
//! encryption in Nostr. Version 2 is the current standard.
//!
//! ## Cryptographic Primitives
//!
//! - ECDH using secp256k1 for shared secret generation
//! - HKDF-SHA256 for key derivation
//! - ChaCha20 for encryption
//! - HMAC-SHA256 for authentication
//!
//! ## Security Considerations
//!
//! This encryption scheme has several limitations:
//! - No deniability (events are signed)
//! - No forward secrecy
//! - No post-compromise security
//! - No post-quantum security
//! - Potential IP address leakage
//! - Timestamp visibility
//!
//! ## Example
//!
//! ```no_run
//! use nostr_core::nip44::{encrypt, decrypt};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let sender_secret_key = [1u8; 32];
//! let recipient_public_key = [2u8; 33]; // Compressed public key
//! let plaintext = "Hello, Nostr!";
//!
//! // Encrypt
//! let encrypted = encrypt(&sender_secret_key, &recipient_public_key, plaintext)?;
//!
//! // Decrypt
//! let decrypted = decrypt(&sender_secret_key, &recipient_public_key, &encrypted)?;
//! assert_eq!(decrypted, plaintext);
//! # Ok(())
//! # }
//! ```

use base64::{Engine, engine::general_purpose::STANDARD as BASE64_STANDARD};
use bitcoin::secp256k1::{PublicKey, SecretKey, ecdh};
use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

/// NIP-44 version 2
pub const VERSION: u8 = 2;

/// Minimum plaintext length (1 byte)
pub const MIN_PLAINTEXT_LEN: usize = 1;

/// Maximum plaintext length (65535 bytes)
pub const MAX_PLAINTEXT_LEN: usize = 65535;

/// Minimum padded message size (32 bytes)
pub const MIN_PADDED_LEN: usize = 32;

/// Nonce size (32 bytes)
pub const NONCE_SIZE: usize = 32;

/// MAC size (32 bytes)
pub const MAC_SIZE: usize = 32;

/// ChaCha20 key size (32 bytes)
pub const CHACHA_KEY_SIZE: usize = 32;

/// ChaCha20 nonce size (12 bytes)
pub const CHACHA_NONCE_SIZE: usize = 12;

/// HMAC key size (32 bytes)
pub const HMAC_KEY_SIZE: usize = 32;

/// HKDF salt for conversation key
pub const HKDF_SALT: &[u8] = b"nip44-v2";

/// Errors that can occur during NIP-44 operations.
#[derive(Debug, Error)]
pub enum Nip44Error {
    #[error("plaintext too short (minimum 1 byte)")]
    PlaintextTooShort,

    #[error("plaintext too long (maximum 65535 bytes)")]
    PlaintextTooLong,

    #[error("invalid payload structure")]
    InvalidPayload,

    #[error("unsupported version: {0}")]
    UnsupportedVersion(u8),

    #[error("MAC verification failed")]
    MacVerificationFailed,

    #[error("invalid padding")]
    InvalidPadding,

    #[error("secp256k1 error: {0}")]
    Secp256k1(#[from] bitcoin::secp256k1::Error),

    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("invalid public key format")]
    InvalidPublicKey,

    #[error("invalid secret key format")]
    InvalidSecretKey,
}

type HmacSha256 = Hmac<Sha256>;

/// Calculate the conversation key from a sender's secret key and recipient's public key.
///
/// Uses ECDH to generate a shared secret, then applies HKDF-extract with the salt "nip44-v2".
///
/// # Arguments
///
/// * `secret_key` - The sender's 32-byte secret key
/// * `public_key` - The recipient's 33-byte compressed public key
///
/// # Returns
///
/// A 32-byte conversation key
fn get_conversation_key(secret_key: &[u8; 32], public_key: &[u8]) -> Result<[u8; 32], Nip44Error> {
    // Parse keys
    let sk = SecretKey::from_slice(secret_key)?;
    let pk = PublicKey::from_slice(public_key).map_err(|_| Nip44Error::InvalidPublicKey)?;

    // Perform ECDH - get shared secret
    let shared_secret = ecdh::shared_secret_point(&pk, &sk);

    // Extract x-coordinate (first 32 bytes of the 64-byte shared secret)
    let shared_x = &shared_secret[..32];

    // Apply HKDF-extract with salt
    let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), shared_x);

    // Extract 32 bytes for conversation key
    let mut conversation_key = [0u8; 32];
    hkdf.expand(&[], &mut conversation_key)
        .map_err(|_| Nip44Error::InvalidPayload)?;

    Ok(conversation_key)
}

/// Derive message keys from the conversation key and nonce.
///
/// Uses HKDF-expand to derive 76 bytes:
/// - bytes 0-31: ChaCha20 key
/// - bytes 32-43: ChaCha20 nonce
/// - bytes 44-75: HMAC key
fn derive_message_keys(
    conversation_key: &[u8; 32],
    nonce: &[u8; 32],
) -> Result<([u8; 32], [u8; 12], [u8; 32]), Nip44Error> {
    let hkdf = Hkdf::<Sha256>::new(Some(conversation_key), &[]);

    let mut output = [0u8; 76];
    hkdf.expand(nonce, &mut output)
        .map_err(|_| Nip44Error::InvalidPayload)?;

    let mut chacha_key = [0u8; 32];
    let mut chacha_nonce = [0u8; 12];
    let mut hmac_key = [0u8; 32];

    chacha_key.copy_from_slice(&output[0..32]);
    chacha_nonce.copy_from_slice(&output[32..44]);
    hmac_key.copy_from_slice(&output[44..76]);

    Ok((chacha_key, chacha_nonce, hmac_key))
}

/// Calculate the padded length for a given plaintext length.
///
/// Uses power-of-two rounding with a minimum of 32 bytes.
fn calc_padded_len(unpadded_len: usize) -> usize {
    if unpadded_len <= 32 {
        return 32;
    }

    if unpadded_len <= 256 {
        // Find next power of 2 >= unpadded_len
        unpadded_len.next_power_of_two()
    } else {
        // For lengths > 256, round up to next multiple of 32
        unpadded_len.div_ceil(32) * 32
    }
}

/// Pad plaintext according to NIP-44 padding scheme.
///
/// Format: [length: u16 big-endian][plaintext][zero bytes]
fn pad(plaintext: &str) -> Result<Vec<u8>, Nip44Error> {
    let plaintext_bytes = plaintext.as_bytes();
    let plaintext_len = plaintext_bytes.len();

    if plaintext_len < MIN_PLAINTEXT_LEN {
        return Err(Nip44Error::PlaintextTooShort);
    }

    if plaintext_len > MAX_PLAINTEXT_LEN {
        return Err(Nip44Error::PlaintextTooLong);
    }

    // Calculate padded length (includes 2-byte length prefix)
    let unpadded_len = 2 + plaintext_len;
    let padded_len = calc_padded_len(unpadded_len);

    let mut padded = vec![0u8; padded_len];

    // Write length as big-endian u16
    padded[0] = ((plaintext_len >> 8) & 0xFF) as u8;
    padded[1] = (plaintext_len & 0xFF) as u8;

    // Copy plaintext
    padded[2..2 + plaintext_len].copy_from_slice(plaintext_bytes);

    // Remaining bytes are already zeros

    Ok(padded)
}

/// Remove padding from decrypted data.
///
/// Validates the padding structure and extracts the plaintext.
fn unpad(padded: &[u8]) -> Result<String, Nip44Error> {
    if padded.len() < 2 {
        return Err(Nip44Error::InvalidPadding);
    }

    // Read length as big-endian u16
    let plaintext_len = ((padded[0] as usize) << 8) | (padded[1] as usize);

    if plaintext_len < MIN_PLAINTEXT_LEN || plaintext_len > MAX_PLAINTEXT_LEN {
        return Err(Nip44Error::InvalidPadding);
    }

    let unpadded_len = 2 + plaintext_len;

    if padded.len() < unpadded_len {
        return Err(Nip44Error::InvalidPadding);
    }

    // Verify padding is correct length
    let expected_padded_len = calc_padded_len(unpadded_len);
    if padded.len() != expected_padded_len {
        return Err(Nip44Error::InvalidPadding);
    }

    // Verify zero padding
    for &byte in &padded[unpadded_len..] {
        if byte != 0 {
            return Err(Nip44Error::InvalidPadding);
        }
    }

    // Extract plaintext
    let plaintext_bytes = &padded[2..2 + plaintext_len];
    String::from_utf8(plaintext_bytes.to_vec()).map_err(|_| Nip44Error::InvalidPadding)
}

/// Encrypt plaintext using NIP-44 version 2.
///
/// # Arguments
///
/// * `sender_secret_key` - The sender's 32-byte secret key
/// * `recipient_public_key` - The recipient's 33-byte compressed public key
/// * `plaintext` - The plaintext to encrypt
///
/// # Returns
///
/// Base64-encoded encrypted payload: version(1) + nonce(32) + ciphertext(var) + mac(32)
///
/// # Example
///
/// ```no_run
/// use nostr_core::nip44::encrypt;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let sender_key = [1u8; 32];
/// let recipient_key = [2u8; 33];
/// let encrypted = encrypt(&sender_key, &recipient_key, "Hello!")?;
/// # Ok(())
/// # }
/// ```
pub fn encrypt(
    sender_secret_key: &[u8; 32],
    recipient_public_key: &[u8],
    plaintext: &str,
) -> Result<String, Nip44Error> {
    // Get conversation key
    let conversation_key = get_conversation_key(sender_secret_key, recipient_public_key)?;

    // Generate random nonce
    let mut nonce = [0u8; NONCE_SIZE];
    use rand::RngCore;
    rand::rng().fill_bytes(&mut nonce);

    // Derive message keys
    let (chacha_key, chacha_nonce, hmac_key) = derive_message_keys(&conversation_key, &nonce)?;

    // Pad plaintext
    let padded = pad(plaintext)?;

    // Encrypt with ChaCha20
    let mut ciphertext = padded;
    let mut cipher = ChaCha20::new(&chacha_key.into(), &chacha_nonce.into());
    cipher.apply_keystream(&mut ciphertext);

    // Calculate MAC over nonce || ciphertext
    let mut mac = HmacSha256::new_from_slice(&hmac_key).map_err(|_| Nip44Error::InvalidPayload)?;
    mac.update(&nonce);
    mac.update(&ciphertext);
    let mac_bytes = mac.finalize().into_bytes();

    // Build payload: version || nonce || ciphertext || mac
    let mut payload = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len() + MAC_SIZE);
    payload.push(VERSION);
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);
    payload.extend_from_slice(&mac_bytes);

    // Base64 encode
    Ok(BASE64_STANDARD.encode(&payload))
}

/// Decrypt a NIP-44 encrypted payload.
///
/// # Arguments
///
/// * `recipient_secret_key` - The recipient's 32-byte secret key
/// * `sender_public_key` - The sender's 33-byte compressed public key
/// * `payload` - The base64-encoded encrypted payload
///
/// # Returns
///
/// The decrypted plaintext
///
/// # Example
///
/// ```no_run
/// use nostr_core::nip44::{encrypt, decrypt};
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let key = [1u8; 32];
/// let pubkey = [2u8; 33];
/// let encrypted = encrypt(&key, &pubkey, "Hello!")?;
/// let decrypted = decrypt(&key, &pubkey, &encrypted)?;
/// assert_eq!(decrypted, "Hello!");
/// # Ok(())
/// # }
/// ```
pub fn decrypt(
    recipient_secret_key: &[u8; 32],
    sender_public_key: &[u8],
    payload: &str,
) -> Result<String, Nip44Error> {
    // Base64 decode
    let decoded = BASE64_STANDARD.decode(payload)?;

    // Minimum payload size: version(1) + nonce(32) + ciphertext(32 min) + mac(32)
    if decoded.len() < 1 + NONCE_SIZE + MIN_PADDED_LEN + MAC_SIZE {
        return Err(Nip44Error::InvalidPayload);
    }

    // Extract components
    let version = decoded[0];
    if version != VERSION {
        return Err(Nip44Error::UnsupportedVersion(version));
    }

    let nonce = &decoded[1..1 + NONCE_SIZE];
    let ciphertext = &decoded[1 + NONCE_SIZE..decoded.len() - MAC_SIZE];
    let mac_received = &decoded[decoded.len() - MAC_SIZE..];

    // Get conversation key
    let conversation_key = get_conversation_key(recipient_secret_key, sender_public_key)?;

    // Derive message keys
    let nonce_array: [u8; 32] = nonce.try_into().unwrap();
    let (chacha_key, chacha_nonce, hmac_key) =
        derive_message_keys(&conversation_key, &nonce_array)?;

    // Verify MAC
    let mut mac = HmacSha256::new_from_slice(&hmac_key).map_err(|_| Nip44Error::InvalidPayload)?;
    mac.update(nonce);
    mac.update(ciphertext);

    // Constant-time comparison
    mac.verify_slice(mac_received)
        .map_err(|_| Nip44Error::MacVerificationFailed)?;

    // Decrypt with ChaCha20
    let mut plaintext = ciphertext.to_vec();
    let mut cipher = ChaCha20::new(&chacha_key.into(), &chacha_nonce.into());
    cipher.apply_keystream(&mut plaintext);

    // Remove padding
    unpad(&plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::Secp256k1;

    #[test]
    fn test_calc_padded_len() {
        assert_eq!(calc_padded_len(1), 32);
        assert_eq!(calc_padded_len(32), 32);
        assert_eq!(calc_padded_len(33), 64);
        assert_eq!(calc_padded_len(64), 64);
        assert_eq!(calc_padded_len(65), 128);
        assert_eq!(calc_padded_len(256), 256);
        assert_eq!(calc_padded_len(257), 288); // Next multiple of 32
        assert_eq!(calc_padded_len(1000), 1024);
    }

    #[test]
    fn test_pad_unpad() {
        let plaintext = "Hello, Nostr!";
        let padded = pad(plaintext).unwrap();

        // Verify length prefix
        let len = ((padded[0] as usize) << 8) | (padded[1] as usize);
        assert_eq!(len, plaintext.len());

        // Verify plaintext
        assert_eq!(&padded[2..2 + plaintext.len()], plaintext.as_bytes());

        // Verify padding is zeros
        for &byte in &padded[2 + plaintext.len()..] {
            assert_eq!(byte, 0);
        }

        // Unpad
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn test_pad_minimum() {
        let padded = pad("x").unwrap();
        assert_eq!(padded.len(), 32); // Minimum padding
    }

    #[test]
    fn test_pad_too_short() {
        let result = pad("");
        assert!(matches!(result, Err(Nip44Error::PlaintextTooShort)));
    }

    #[test]
    fn test_pad_too_long() {
        let long_text = "x".repeat(65536);
        let result = pad(&long_text);
        assert!(matches!(result, Err(Nip44Error::PlaintextTooLong)));
    }

    #[test]
    fn test_unpad_invalid_length() {
        let invalid = vec![0xFF, 0xFF]; // Length > MAX
        let result = unpad(&invalid);
        assert!(matches!(result, Err(Nip44Error::InvalidPadding)));
    }

    #[test]
    fn test_unpad_incorrect_padding() {
        let mut padded = pad("test").unwrap();
        let len = padded.len();
        padded[len - 1] = 1; // Non-zero padding byte
        let result = unpad(&padded);
        assert!(matches!(result, Err(Nip44Error::InvalidPadding)));
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secp = Secp256k1::new();
        let sender_sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let recipient_sk = SecretKey::from_slice(&[2u8; 32]).unwrap();
        let sender_pk = PublicKey::from_secret_key(&secp, &sender_sk);
        let recipient_pk = PublicKey::from_secret_key(&secp, &recipient_sk);

        let sender_secret = sender_sk.secret_bytes();
        let recipient_secret = recipient_sk.secret_bytes();
        let recipient_pub = recipient_pk.serialize();
        let sender_pub = sender_pk.serialize();

        let plaintext = "Hello NIP-44";
        let encrypted = encrypt(&sender_secret, &recipient_pub, plaintext).unwrap();
        assert_ne!(encrypted, plaintext);

        let decrypted = decrypt(&recipient_secret, &sender_pub, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
