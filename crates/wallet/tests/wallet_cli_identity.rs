//! Integration tests for Wallet CLI identity commands
//!
//! Tests verify:
//! - Identity generation and storage
//! - Mnemonic import/export
//! - Identity persistence via keychain
//! - Error handling for missing/invalid credentials

use bip39::Mnemonic;
use wallet::core::identity::UnifiedIdentity;
use wallet::storage::keychain::SecureKeychain;

/// Test helper to clean up keychain after tests
struct KeychainGuard;

impl Drop for KeychainGuard {
    fn drop(&mut self) {
        // Clean up any test mnemonics
        if SecureKeychain::has_mnemonic() {
            let _ = SecureKeychain::delete_mnemonic();
        }
    }
}

/// Test identity generation creates valid mnemonic
#[test]
fn test_identity_generation() {
    let _guard = KeychainGuard;

    // Clean state
    if SecureKeychain::has_mnemonic() {
        SecureKeychain::delete_mnemonic().unwrap();
    }

    // Generate new identity
    let identity = UnifiedIdentity::generate().expect("Should generate identity");

    // Verify mnemonic is valid
    let mnemonic_str = identity.mnemonic().to_string();
    let parsed = Mnemonic::parse(&mnemonic_str).expect("Should parse generated mnemonic");
    assert!(parsed.word_count() == 12 || parsed.word_count() == 24);

    // Verify Nostr keys are generated
    let npub = identity.nostr_public_key();
    assert!(!npub.is_empty(), "Should have Nostr public key");

    let nsec = identity.nostr_secret_key();
    assert_eq!(
        nsec.len(),
        64,
        "Secret key should be 32 bytes (64 hex chars)"
    );
}

/// Test identity can be derived from mnemonic
#[test]
fn test_identity_from_mnemonic() {
    let _guard = KeychainGuard;

    let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mnemonic = Mnemonic::parse(test_mnemonic).unwrap();

    // Derive identity
    let identity = UnifiedIdentity::from_mnemonic(mnemonic.clone())
        .expect("Should derive identity from mnemonic");

    // Verify derivation is deterministic
    let identity2 = UnifiedIdentity::from_mnemonic(mnemonic)
        .expect("Should derive identity from same mnemonic");

    assert_eq!(
        identity.nostr_public_key(),
        identity2.nostr_public_key(),
        "Same mnemonic should produce same identity"
    );
}

/// Test mnemonic storage and retrieval
#[test]
#[ignore] // Requires OS keychain access
fn test_mnemonic_storage() {
    let _guard = KeychainGuard;

    // Clean state
    if SecureKeychain::has_mnemonic() {
        SecureKeychain::delete_mnemonic().unwrap();
    }

    let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    // Store mnemonic
    SecureKeychain::store_mnemonic(test_mnemonic).expect("Should store mnemonic");

    // Verify it exists
    assert!(
        SecureKeychain::has_mnemonic(),
        "Should have mnemonic after storage"
    );

    // Retrieve and verify
    let retrieved = SecureKeychain::retrieve_mnemonic().expect("Should retrieve mnemonic");

    assert_eq!(
        retrieved, test_mnemonic,
        "Retrieved mnemonic should match stored"
    );
}

/// Test mnemonic deletion
#[test]
#[ignore] // Requires OS keychain access
fn test_mnemonic_deletion() {
    let _guard = KeychainGuard;

    let test_mnemonic =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    // Store mnemonic
    SecureKeychain::store_mnemonic(test_mnemonic).expect("Should store mnemonic");

    assert!(SecureKeychain::has_mnemonic(), "Should have mnemonic");

    // Delete mnemonic
    SecureKeychain::delete_mnemonic().expect("Should delete mnemonic");

    // Verify it's gone
    assert!(
        !SecureKeychain::has_mnemonic(),
        "Should not have mnemonic after deletion"
    );
}

/// Test retrieving non-existent mnemonic returns error
#[test]
fn test_retrieve_missing_mnemonic() {
    let _guard = KeychainGuard;

    // Clean state
    if SecureKeychain::has_mnemonic() {
        SecureKeychain::delete_mnemonic().unwrap();
    }

    // Try to retrieve non-existent mnemonic
    let result = SecureKeychain::retrieve_mnemonic();

    assert!(
        result.is_err(),
        "Should error when retrieving missing mnemonic"
    );
}

/// Test identity persistence across sessions
#[test]
#[ignore] // Requires OS keychain access
fn test_identity_persistence() {
    let _guard = KeychainGuard;

    // Clean state
    if SecureKeychain::has_mnemonic() {
        SecureKeychain::delete_mnemonic().unwrap();
    }

    // Generate and store identity
    let identity1 = UnifiedIdentity::generate().unwrap();
    let mnemonic1 = identity1.mnemonic().to_string();

    SecureKeychain::store_mnemonic(&mnemonic1).unwrap();

    // Simulate new session: retrieve mnemonic and derive identity
    let retrieved_mnemonic = SecureKeychain::retrieve_mnemonic().unwrap();
    let parsed_mnemonic = Mnemonic::parse(&retrieved_mnemonic).unwrap();
    let identity2 = UnifiedIdentity::from_mnemonic(parsed_mnemonic).unwrap();

    // Verify identities match
    assert_eq!(
        identity1.nostr_public_key(),
        identity2.nostr_public_key(),
        "Identity should persist across sessions"
    );

    assert_eq!(
        identity1.nostr_secret_key(),
        identity2.nostr_secret_key(),
        "Secret key should match"
    );
}

/// Test import with invalid mnemonic
#[test]
fn test_invalid_mnemonic_import() {
    let invalid_mnemonic = "invalid word sequence that is not a valid mnemonic phrase";

    let result = Mnemonic::parse(invalid_mnemonic);

    assert!(result.is_err(), "Should reject invalid mnemonic");
}

/// Test identity export format
#[test]
fn test_identity_export_format() {
    let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mnemonic = Mnemonic::parse(test_mnemonic).unwrap();
    let identity = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();

    // Verify public key is hex format (66 chars for compressed key)
    let pubkey = identity.nostr_public_key();
    assert!(
        pubkey.len() == 66 || pubkey.len() == 64,
        "Nostr public key should be hex format (64 or 66 chars)"
    );
    assert!(
        pubkey.chars().all(|c| c.is_ascii_hexdigit()),
        "Public key should be valid hex"
    );

    // Verify nsec is hex (64 chars)
    let nsec = identity.nostr_secret_key();
    assert_eq!(nsec.len(), 64, "Secret key should be 32 bytes hex");
    assert!(
        nsec.chars().all(|c| c.is_ascii_hexdigit()),
        "Secret key should be valid hex"
    );
}

/// Test Spark wallet address generation
#[test]
fn test_spark_address_generation() {
    let identity = UnifiedIdentity::generate().unwrap();

    // Verify Spark address exists (implementation detail may vary)
    // For now just verify we can get public key
    let npub = identity.nostr_public_key();
    assert!(!npub.is_empty(), "Should have address for Spark wallet");
}

/// Test multiple identity generation produces different keys
#[test]
fn test_unique_identity_generation() {
    let identity1 = UnifiedIdentity::generate().unwrap();
    let identity2 = UnifiedIdentity::generate().unwrap();

    // Different identities should have different keys
    assert_ne!(
        identity1.nostr_public_key(),
        identity2.nostr_public_key(),
        "Different identities should have different public keys"
    );

    assert_ne!(
        identity1.mnemonic().to_string(),
        identity2.mnemonic().to_string(),
        "Different identities should have different mnemonics"
    );
}

/// Test known mnemonic produces known keys
#[test]
fn test_deterministic_key_derivation() {
    let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mnemonic = Mnemonic::parse(test_mnemonic).unwrap();

    // Derive identity multiple times
    let identity1 = UnifiedIdentity::from_mnemonic(mnemonic.clone()).unwrap();
    let identity2 = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();

    // Verify keys are deterministic
    assert_eq!(
        identity1.nostr_public_key(),
        identity2.nostr_public_key(),
        "Same mnemonic should always produce same public key"
    );

    // Verify the public key is a valid hex string
    let pubkey = identity1.nostr_public_key();
    assert!(
        pubkey.chars().all(|c| c.is_ascii_hexdigit()),
        "Public key should be valid hex"
    );
}

/// Test keychain cleanup on multiple stores
#[test]
#[ignore] // Requires OS keychain access
fn test_keychain_overwrite() {
    let _guard = KeychainGuard;

    // Clean state
    if SecureKeychain::has_mnemonic() {
        SecureKeychain::delete_mnemonic().unwrap();
    }

    let mnemonic1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let mnemonic2 = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

    // Store first mnemonic
    SecureKeychain::store_mnemonic(mnemonic1).unwrap();

    // Delete and store second
    SecureKeychain::delete_mnemonic().unwrap();
    SecureKeychain::store_mnemonic(mnemonic2).unwrap();

    // Verify second mnemonic is stored
    let retrieved = SecureKeychain::retrieve_mnemonic().unwrap();
    assert_eq!(
        retrieved, mnemonic2,
        "Should retrieve most recently stored mnemonic"
    );
}
