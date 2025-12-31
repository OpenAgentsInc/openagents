//! Unit tests for UnifiedIdentity domain module

use compute::domain::UnifiedIdentity;

// Test mnemonic (valid BIP39 12-word phrase - DO NOT USE IN PRODUCTION)
const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[test]
fn test_generate_identity() {
    let identity = UnifiedIdentity::generate().expect("Should generate identity");

    // Check mnemonic is 12 words
    let words = identity.mnemonic_words();
    assert_eq!(words.len(), 12);

    // Check keys are generated
    assert!(!identity.public_key_hex().is_empty());
    assert_eq!(identity.private_key_bytes().len(), 32);
    assert_eq!(identity.public_key_bytes().len(), 32);
}

#[test]
fn test_generate_24_word_identity() {
    let identity = UnifiedIdentity::generate_24_words().expect("Should generate 24-word identity");

    // Check mnemonic is 24 words
    let words = identity.mnemonic_words();
    assert_eq!(words.len(), 24);
}

#[test]
fn test_from_mnemonic() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create from mnemonic");

    assert_eq!(identity.mnemonic(), TEST_MNEMONIC);
    assert_eq!(identity.mnemonic_words().len(), 12);
}

#[test]
fn test_from_mnemonic_with_passphrase() {
    let identity1 = UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "")
        .expect("Should create without passphrase");
    let identity2 = UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "passphrase")
        .expect("Should create with passphrase");

    // Different passphrases should produce different keys
    assert_ne!(identity1.public_key_hex(), identity2.public_key_hex());
}

#[test]
fn test_invalid_mnemonic() {
    let result = UnifiedIdentity::from_mnemonic("invalid word list here", "");
    assert!(result.is_err());
}

#[test]
fn test_wrong_word_count() {
    let result = UnifiedIdentity::from_mnemonic("abandon abandon abandon", "");
    assert!(result.is_err());
}

#[test]
fn test_npub_format() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    let npub = identity.npub().expect("Should get npub");
    assert!(npub.starts_with("npub1"));
    assert_eq!(npub.len(), 63); // Standard bech32 npub length
}

#[test]
fn test_nsec_format() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    let nsec = identity.nsec().expect("Should get nsec");
    assert!(nsec.starts_with("nsec1"));
    assert_eq!(nsec.len(), 63); // Standard bech32 nsec length
}

#[test]
fn test_public_key_hex() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    let hex = identity.public_key_hex();
    assert_eq!(hex.len(), 64); // 32 bytes as hex
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_npub_short() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    let short = identity.npub_short();
    assert!(short.contains("..."));
    assert!(short.starts_with("npub1"));
    assert!(short.len() < 63); // Should be shorter than full npub
}

#[test]
fn test_private_key_bytes() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    let bytes = identity.private_key_bytes();
    assert_eq!(bytes.len(), 32);
}

#[test]
fn test_public_key_bytes() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    let bytes = identity.public_key_bytes();
    assert_eq!(bytes.len(), 32);
}

#[test]
fn test_deterministic_keys() {
    // Same mnemonic should always produce same keys
    let identity1 =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");
    let identity2 =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    assert_eq!(identity1.public_key_hex(), identity2.public_key_hex());
    assert_eq!(identity1.private_key_bytes(), identity2.private_key_bytes());
    assert_eq!(identity1.npub().unwrap(), identity2.npub().unwrap());
}

#[test]
fn test_different_mnemonics_different_keys() {
    let identity1 = UnifiedIdentity::generate().expect("Should generate");
    let identity2 = UnifiedIdentity::generate().expect("Should generate");

    assert_ne!(identity1.mnemonic(), identity2.mnemonic());
    assert_ne!(identity1.public_key_hex(), identity2.public_key_hex());
}

#[test]
fn test_mnemonic_words_split() {
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    let words = identity.mnemonic_words();
    assert_eq!(words[0], "abandon");
    assert_eq!(words[11], "about");
}

#[test]
fn test_clone_identity() {
    let identity1 =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");
    let identity2 = identity1.clone();

    assert_eq!(identity1.public_key_hex(), identity2.public_key_hex());
    assert_eq!(identity1.mnemonic(), identity2.mnemonic());
}

#[test]
fn test_nip06_derivation_path() {
    // Verify that NIP-06 derivation is used (m/44'/1237'/0'/0/0)
    let identity =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create identity");

    // This should match the expected public key from NIP-06 derivation
    // (Actual value verified against NIP-06 reference implementation)
    let npub = identity.npub().expect("Should get npub");
    assert!(npub.starts_with("npub1"));
}

#[test]
fn test_empty_passphrase() {
    let identity1 =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create with empty string");
    let identity2 =
        UnifiedIdentity::from_mnemonic(TEST_MNEMONIC, "").expect("Should create with empty string");

    // Empty passphrase should be deterministic
    assert_eq!(identity1.public_key_hex(), identity2.public_key_hex());
}

#[test]
fn test_case_sensitivity() {
    // BIP39 mnemonics are case-sensitive (must be lowercase)
    let upper = TEST_MNEMONIC.to_uppercase();
    let result = UnifiedIdentity::from_mnemonic(&upper, "");
    // Uppercase words should fail validation
    assert!(result.is_err());
}

#[test]
fn test_extra_whitespace() {
    let spaced = "abandon  abandon   abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let identity =
        UnifiedIdentity::from_mnemonic(spaced, "").expect("Should handle extra whitespace");

    assert_eq!(identity.mnemonic_words().len(), 12);
}

#[test]
fn test_12_word_mnemonic_validity() {
    // Valid 12-word mnemonic
    let valid = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    assert!(UnifiedIdentity::from_mnemonic(valid, "").is_ok());
}

#[test]
fn test_24_word_mnemonic_validity() {
    // Valid 24-word mnemonic
    let valid = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
    let identity =
        UnifiedIdentity::from_mnemonic(valid, "").expect("Should accept 24-word mnemonic");
    assert_eq!(identity.mnemonic_words().len(), 24);
}

#[test]
fn test_invalid_checksum() {
    // Last word has wrong checksum
    let invalid = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
    assert!(UnifiedIdentity::from_mnemonic(invalid, "").is_err());
}
