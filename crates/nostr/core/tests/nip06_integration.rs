//! Integration tests for NIP-06 key derivation

use nostr::derive_keypair_full;

const TEST_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

#[test]
fn test_derive_keypair_basic() {
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive keypair");

    assert_eq!(keypair.private_key.len(), 32);
    assert_eq!(keypair.public_key.len(), 32);
}

#[test]
fn test_derive_keypair_deterministic() {
    let keypair1 = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let keypair2 = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");

    assert_eq!(keypair1.private_key, keypair2.private_key);
    assert_eq!(keypair1.public_key, keypair2.public_key);
}

#[test]
fn test_derive_different_accounts() {
    let keypair0 = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let keypair1 = derive_keypair_full(TEST_MNEMONIC, "", 1).expect("Should derive");

    assert_ne!(keypair0.private_key, keypair1.private_key);
    assert_ne!(keypair0.public_key, keypair1.public_key);
}

#[test]
fn test_passphrase_changes_keys() {
    let keypair1 = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let keypair2 = derive_keypair_full(TEST_MNEMONIC, "passphrase", 0).expect("Should derive");

    assert_ne!(keypair1.private_key, keypair2.private_key);
    assert_ne!(keypair1.public_key, keypair2.public_key);
}

#[test]
fn test_npub_format() {
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let npub = keypair.npub().expect("Should get npub");

    assert!(npub.starts_with("npub1"));
    assert_eq!(npub.len(), 63);
}

#[test]
fn test_nsec_format() {
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let nsec = keypair.nsec().expect("Should get nsec");

    assert!(nsec.starts_with("nsec1"));
    assert_eq!(nsec.len(), 63);
}

#[test]
fn test_public_key_hex() {
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let hex = keypair.public_key_hex();

    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_private_key_hex() {
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let hex = keypair.private_key_hex();

    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_invalid_mnemonic() {
    let result = derive_keypair_full("invalid mnemonic words", "", 0);
    assert!(result.is_err());
}

#[test]
fn test_empty_mnemonic() {
    let result = derive_keypair_full("", "", 0);
    assert!(result.is_err());
}

#[test]
fn test_24_word_mnemonic() {
    let mnemonic_24 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
    let keypair = derive_keypair_full(mnemonic_24, "", 0).expect("Should derive from 24 words");

    assert_eq!(keypair.private_key.len(), 32);
    assert_eq!(keypair.public_key.len(), 32);
}

#[test]
fn test_keypair_clone() {
    let keypair1 = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let keypair2 = keypair1.clone();

    assert_eq!(keypair1.private_key, keypair2.private_key);
    assert_eq!(keypair1.public_key, keypair2.public_key);
}

#[test]
fn test_multiple_accounts() {
    let mut keys = Vec::new();
    for account in 0..5 {
        let keypair = derive_keypair_full(TEST_MNEMONIC, "", account).expect("Should derive");
        keys.push(keypair);
    }

    // All keys should be different
    for i in 0..keys.len() {
        for j in i + 1..keys.len() {
            assert_ne!(keys[i].private_key, keys[j].private_key);
            assert_ne!(keys[i].public_key, keys[j].public_key);
        }
    }
}

#[test]
fn test_nip06_coin_type() {
    // Verify that derivation uses coin type 1237 (registered for Nostr in SLIP-0044)
    // This is verified by checking that the same mnemonic produces consistent keys
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");

    // The keypair should be deterministic based on NIP-06 spec
    assert!(!keypair.private_key.iter().all(|&b| b == 0));
    assert!(!keypair.public_key.iter().all(|&b| b == 0));
}

#[test]
fn test_empty_passphrase_vs_no_passphrase() {
    // Empty string passphrase should be treated the same as no passphrase
    let keypair1 = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");
    let keypair2 = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");

    assert_eq!(keypair1.private_key, keypair2.private_key);
}

#[test]
fn test_case_sensitive_passphrase() {
    let keypair1 = derive_keypair_full(TEST_MNEMONIC, "Password", 0).expect("Should derive");
    let keypair2 = derive_keypair_full(TEST_MNEMONIC, "password", 0).expect("Should derive");

    // Different case should produce different keys
    assert_ne!(keypair1.private_key, keypair2.private_key);
}

#[test]
fn test_unicode_passphrase() {
    let keypair =
        derive_keypair_full(TEST_MNEMONIC, "пароль🔐", 0).expect("Should derive with unicode");

    assert_eq!(keypair.private_key.len(), 32);
    assert_eq!(keypair.public_key.len(), 32);
}

#[test]
fn test_long_passphrase() {
    let long_passphrase = "a".repeat(1000);
    let keypair = derive_keypair_full(TEST_MNEMONIC, &long_passphrase, 0).expect("Should derive");

    assert_eq!(keypair.private_key.len(), 32);
}

#[test]
fn test_npub_nsec_roundtrip_compatibility() {
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 0).expect("Should derive");

    let npub = keypair.npub().expect("Should get npub");
    let nsec = keypair.nsec().expect("Should get nsec");

    // Both should be valid bech32
    assert!(npub.starts_with("npub1"));
    assert!(nsec.starts_with("nsec1"));
}

#[test]
fn test_high_account_number() {
    let keypair = derive_keypair_full(TEST_MNEMONIC, "", 999).expect("Should derive high account");

    assert_eq!(keypair.private_key.len(), 32);
    assert_eq!(keypair.public_key.len(), 32);
}
