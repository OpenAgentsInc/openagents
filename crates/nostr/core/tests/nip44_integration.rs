//! Integration tests for NIP-44 encryption
//!
//! Tests the encrypt/decrypt roundtrip and compatibility

use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use nostr::{decrypt_v2, encrypt_v2};

/// Helper to convert x-only pubkey (32 bytes) to compressed pubkey (33 bytes)
#[allow(dead_code)]
fn xonly_to_compressed(xonly: &[u8; 32]) -> Vec<u8> {
    // Use even parity (0x02) prefix for x-only conversion
    let mut compressed = vec![0x02];
    compressed.extend_from_slice(xonly);
    compressed
}

/// Helper to get compressed public key from secret key
fn get_compressed_pubkey(secret_key: &[u8; 32]) -> Vec<u8> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(secret_key).expect("valid secret key");
    let pk = PublicKey::from_secret_key(&secp, &sk);
    pk.serialize().to_vec()
}

#[test]
fn test_encrypt_decrypt_roundtrip() {
    // Generate sender and recipient keys
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);

    // Get compressed public keys (33 bytes)
    let sender_pubkey = get_compressed_pubkey(&sender_key);
    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    let plaintext = "Hello, Nostr! This is a test message for NIP-44 encryption.";

    // Encrypt from sender to recipient
    let ciphertext = encrypt_v2(&sender_key, &recipient_pubkey, plaintext)
        .expect("encryption should succeed");

    // Decrypt at recipient
    let decrypted = decrypt_v2(&recipient_key, &sender_pubkey, &ciphertext)
        .expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_encrypt_decrypt_short_message() {
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);

    let sender_pubkey = get_compressed_pubkey(&sender_key);
    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    let plaintext = "x"; // Single character (minimum length)

    let ciphertext = encrypt_v2(&sender_key, &recipient_pubkey, plaintext)
        .expect("encryption should succeed");

    let decrypted = decrypt_v2(&recipient_key, &sender_pubkey, &ciphertext)
        .expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_encrypt_decrypt_long_message() {
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);

    let sender_pubkey = get_compressed_pubkey(&sender_key);
    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    // Long message (1000 characters)
    let plaintext = "a".repeat(1000);

    let ciphertext = encrypt_v2(&sender_key, &recipient_pubkey, &plaintext)
        .expect("encryption should succeed");

    let decrypted = decrypt_v2(&recipient_key, &sender_pubkey, &ciphertext)
        .expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_encrypt_decrypt_unicode() {
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);

    let sender_pubkey = get_compressed_pubkey(&sender_key);
    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    let plaintext = "Hello 世界! 🌍 Émojis and spëcial çharacters";

    let ciphertext = encrypt_v2(&sender_key, &recipient_pubkey, plaintext)
        .expect("encryption should succeed");

    let decrypted = decrypt_v2(&recipient_key, &sender_pubkey, &ciphertext)
        .expect("decryption should succeed");

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_decrypt_wrong_key_fails() {
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    let mut wrong_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);
    rand::rng().fill_bytes(&mut wrong_key);

    let sender_pubkey = get_compressed_pubkey(&sender_key);
    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    let plaintext = "Secret message";

    // Encrypt to recipient
    let ciphertext = encrypt_v2(&sender_key, &recipient_pubkey, plaintext)
        .expect("encryption should succeed");

    // Try to decrypt with wrong key
    let result = decrypt_v2(&wrong_key, &sender_pubkey, &ciphertext);

    // Should fail MAC verification
    assert!(result.is_err(), "decryption with wrong key should fail");
}

#[test]
fn test_decrypt_tampered_ciphertext_fails() {
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);

    let sender_pubkey = get_compressed_pubkey(&sender_key);
    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    let plaintext = "Original message";

    let ciphertext = encrypt_v2(&sender_key, &recipient_pubkey, plaintext)
        .expect("encryption should succeed");

    // Tamper with ciphertext by changing a character in the middle
    let mut tampered = ciphertext.chars().collect::<Vec<_>>();
    let len = tampered.len();
    if let Some(c) = tampered.get_mut(len / 2) {
        *c = if *c == 'A' { 'B' } else { 'A' };
    }
    let tampered_ciphertext: String = tampered.into_iter().collect();

    // Try to decrypt tampered ciphertext
    let result = decrypt_v2(&recipient_key, &sender_pubkey, &tampered_ciphertext);

    // Should fail (either base64 decode or MAC verification)
    assert!(result.is_err(), "decryption of tampered ciphertext should fail");
}

#[test]
fn test_empty_string_fails() {
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);

    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    // Empty string should fail (minimum 1 byte plaintext)
    let result = encrypt_v2(&sender_key, &recipient_pubkey, "");

    assert!(result.is_err(), "empty plaintext should fail");
}

#[test]
fn test_different_nonces_produce_different_ciphertexts() {
    use rand::RngCore;
    let mut sender_key = [0u8; 32];
    let mut recipient_key = [0u8; 32];
    rand::rng().fill_bytes(&mut sender_key);
    rand::rng().fill_bytes(&mut recipient_key);

    let recipient_pubkey = get_compressed_pubkey(&recipient_key);

    let plaintext = "Same message encrypted twice";

    // Encrypt the same message twice
    let ciphertext1 = encrypt_v2(&sender_key, &recipient_pubkey, plaintext)
        .expect("first encryption should succeed");

    let ciphertext2 = encrypt_v2(&sender_key, &recipient_pubkey, plaintext)
        .expect("second encryption should succeed");

    // Ciphertexts should be different (due to random nonce)
    assert_ne!(ciphertext1, ciphertext2, "same message should produce different ciphertexts");
}
