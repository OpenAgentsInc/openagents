//! NIP-06: Basic key derivation from mnemonic seed phrase.
//!
//! This module implements key derivation according to NIP-06:
//! - BIP39 is used to generate mnemonic seed words and derive a binary seed
//! - BIP32 is used to derive the path `m/44'/1237'/<account>'/0/0`
//!
//! The coin type 1237 is registered in SLIP-0044 for Nostr.

use bip39::Mnemonic;
use bitcoin::Network;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::{PublicKey, SecretKey};
use thiserror::Error;

/// Nostr coin type as registered in SLIP-0044
const NOSTR_COIN_TYPE: u32 = 1237;

/// Human-readable part for nsec (private key)
const NSEC_HRP: &str = "nsec";

/// Human-readable part for npub (public key)
const NPUB_HRP: &str = "npub";

/// Errors that can occur during NIP-06 operations.
#[derive(Debug, Error)]
pub enum Nip06Error {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("key derivation error: {0}")]
    KeyDerivation(String),

    #[error("bech32 encoding error: {0}")]
    Bech32Encode(String),

    #[error("bech32 decoding error: {0}")]
    Bech32Decode(String),

    #[error("invalid key format: {0}")]
    InvalidKeyFormat(String),

    #[error("invalid hrp: expected {expected}, got {got}")]
    InvalidHrp { expected: String, got: String },
}

/// A Nostr keypair containing both private and public keys.
#[derive(Clone)]
pub struct Keypair {
    /// The 32-byte private key
    pub private_key: [u8; 32],
    /// The 32-byte x-only public key (for Nostr, we use only the x-coordinate)
    pub public_key: [u8; 32],
}

impl Keypair {
    /// Get the private key as a hex string.
    pub fn private_key_hex(&self) -> String {
        hex::encode(self.private_key)
    }

    /// Get the public key as a hex string.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }

    /// Get the nsec (bech32-encoded private key).
    pub fn nsec(&self) -> Result<String, Nip06Error> {
        private_key_to_nsec(&self.private_key)
    }

    /// Get the npub (bech32-encoded public key).
    pub fn npub(&self) -> Result<String, Nip06Error> {
        public_key_to_npub(&self.public_key)
    }
}

impl std::fmt::Debug for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keypair")
            .field("public_key", &self.public_key_hex())
            .field("private_key", &"[redacted]")
            .finish()
    }
}

/// Convert a mnemonic phrase to a BIP39 seed.
///
/// The passphrase is optional and defaults to empty string per BIP39 spec.
pub fn mnemonic_to_seed(mnemonic: &str, passphrase: &str) -> Result<[u8; 64], Nip06Error> {
    let mnemonic =
        Mnemonic::parse(mnemonic).map_err(|e| Nip06Error::InvalidMnemonic(e.to_string()))?;

    let seed = mnemonic.to_seed(passphrase);
    Ok(seed)
}

/// Derive a Nostr keypair from a mnemonic using account index 0.
///
/// This is the standard NIP-06 derivation path: `m/44'/1237'/0'/0/0`
pub fn derive_keypair(mnemonic: &str) -> Result<Keypair, Nip06Error> {
    derive_keypair_with_account(mnemonic, 0)
}

/// Derive a Nostr keypair from a mnemonic with a specific account index.
///
/// Uses derivation path: `m/44'/1237'/<account>'/0/0`
pub fn derive_keypair_with_account(mnemonic: &str, account: u32) -> Result<Keypair, Nip06Error> {
    derive_keypair_full(mnemonic, "", account)
}

/// Derive an agent keypair from a mnemonic using agent index.
///
/// Agent index 0 maps to account 1, matching the path `m/44'/1237'/{n+1}'/0/0`.
pub fn derive_agent_keypair(mnemonic: &str, agent_id: u32) -> Result<Keypair, Nip06Error> {
    let account = agent_id
        .checked_add(1)
        .ok_or_else(|| Nip06Error::KeyDerivation("agent index overflow".to_string()))?;
    derive_keypair_with_account(mnemonic, account)
}

/// Derive a Nostr keypair from a mnemonic with passphrase and account index.
///
/// Uses derivation path: `m/44'/1237'/<account>'/0/0`
///
/// The passphrase is used during BIP39 seed derivation (not for encryption).
/// An empty passphrase is equivalent to no passphrase.
pub fn derive_keypair_full(
    mnemonic: &str,
    passphrase: &str,
    account: u32,
) -> Result<Keypair, Nip06Error> {
    let seed = mnemonic_to_seed(mnemonic, passphrase)?;
    derive_keypair_from_seed(&seed, account)
}

/// Derive a Nostr keypair from a 64-byte seed with a specific account index.
fn derive_keypair_from_seed(seed: &[u8; 64], account: u32) -> Result<Keypair, Nip06Error> {
    let secp = Secp256k1::new();

    // Create master key from seed (using Bitcoin mainnet - the network doesn't affect the key derivation)
    let master = Xpriv::new_master(Network::Bitcoin, seed)
        .map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?;

    // Build derivation path: m/44'/1237'/<account>'/0/0
    let path = DerivationPath::from(vec![
        ChildNumber::from_hardened_idx(44).map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?,
        ChildNumber::from_hardened_idx(NOSTR_COIN_TYPE)
            .map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?,
        ChildNumber::from_hardened_idx(account)
            .map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?,
        ChildNumber::from_normal_idx(0).map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?,
        ChildNumber::from_normal_idx(0).map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?,
    ]);

    // Derive the child key
    let derived = master
        .derive_priv(&secp, &path)
        .map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?;

    // Get the private key bytes
    let private_key: [u8; 32] = derived.private_key.secret_bytes();

    // Get the public key (x-only, 32 bytes)
    let secret_key = SecretKey::from_slice(&private_key)
        .map_err(|e| Nip06Error::KeyDerivation(e.to_string()))?;
    let public_key_full = PublicKey::from_secret_key(&secp, &secret_key);

    // For Nostr, we use x-only public keys (just the x-coordinate, 32 bytes)
    // The full public key is 33 bytes (1 byte prefix + 32 bytes x-coordinate)
    // We strip the prefix byte
    let public_key_bytes = public_key_full.serialize();
    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&public_key_bytes[1..33]);

    Ok(Keypair {
        private_key,
        public_key,
    })
}

/// Encode a 32-byte private key as an nsec bech32 string.
pub fn private_key_to_nsec(private_key: &[u8; 32]) -> Result<String, Nip06Error> {
    encode_bech32(NSEC_HRP, private_key)
}

/// Encode a 32-byte public key as an npub bech32 string.
pub fn public_key_to_npub(public_key: &[u8; 32]) -> Result<String, Nip06Error> {
    encode_bech32(NPUB_HRP, public_key)
}

/// Decode an nsec bech32 string to a 32-byte private key.
pub fn nsec_to_private_key(nsec: &str) -> Result<[u8; 32], Nip06Error> {
    decode_bech32(NSEC_HRP, nsec)
}

/// Decode an npub bech32 string to a 32-byte public key.
pub fn npub_to_public_key(npub: &str) -> Result<[u8; 32], Nip06Error> {
    decode_bech32(NPUB_HRP, npub)
}

/// Encode bytes as bech32 with the given human-readable part.
fn encode_bech32(hrp: &str, data: &[u8; 32]) -> Result<String, Nip06Error> {
    use bech32::{Bech32, Hrp};

    let hrp = Hrp::parse(hrp).map_err(|e| Nip06Error::Bech32Encode(e.to_string()))?;

    bech32::encode::<Bech32>(hrp, data).map_err(|e| Nip06Error::Bech32Encode(e.to_string()))
}

/// Decode bech32 string with expected human-readable part.
fn decode_bech32(expected_hrp: &str, encoded: &str) -> Result<[u8; 32], Nip06Error> {
    use bech32::Hrp;

    let expected = Hrp::parse(expected_hrp).map_err(|e| Nip06Error::Bech32Decode(e.to_string()))?;

    let (hrp, data) =
        bech32::decode(encoded).map_err(|e| Nip06Error::Bech32Decode(e.to_string()))?;

    if hrp != expected {
        return Err(Nip06Error::InvalidHrp {
            expected: expected_hrp.to_string(),
            got: hrp.to_string(),
        });
    }

    let bytes: Vec<u8> = data;
    if bytes.len() != 32 {
        return Err(Nip06Error::InvalidKeyFormat(format!(
            "expected 32 bytes, got {}",
            bytes.len()
        )));
    }

    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// NIP-06 Test Vector 1
    /// mnemonic: leader monkey parrot ring guide accident before fence cannon height naive bean
    /// private key (hex): 7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a
    /// nsec: nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp
    /// public key (hex): 17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917
    /// npub: npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu
    #[test]
    fn test_nip06_vector_1() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let expected_private_key =
            "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a";
        let expected_nsec = "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp";
        let expected_public_key =
            "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
        let expected_npub = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";

        let keypair = derive_keypair(mnemonic).expect("should derive keypair");

        assert_eq!(keypair.private_key_hex(), expected_private_key);
        assert_eq!(keypair.public_key_hex(), expected_public_key);
        assert_eq!(keypair.nsec().unwrap(), expected_nsec);
        assert_eq!(keypair.npub().unwrap(), expected_npub);
    }

    /// NIP-06 Test Vector 2
    /// mnemonic: what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade
    /// private key (hex): c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add
    /// nsec: nsec1c9wh8xy5eqdzln7n5t0ctgxjcrdug73gp5yj0x03gntn67h83twssdfhel
    /// public key (hex): d41b22899549e1f3d335a31002cfd382174006e166d3e658e3a5eecdb6463573
    /// npub: npub16sdj9zv4f8sl85e45vgq9n7nsgt5qphpvmf7vk8r5hhvmdjxx4es8rq74h
    #[test]
    fn test_nip06_vector_2() {
        let mnemonic = "what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade";
        let expected_private_key =
            "c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add";
        let expected_nsec = "nsec1c9wh8xy5eqdzln7n5t0ctgxjcrdug73gp5yj0x03gntn67h83twssdfhel";
        let expected_public_key =
            "d41b22899549e1f3d335a31002cfd382174006e166d3e658e3a5eecdb6463573";
        let expected_npub = "npub16sdj9zv4f8sl85e45vgq9n7nsgt5qphpvmf7vk8r5hhvmdjxx4es8rq74h";

        let keypair = derive_keypair(mnemonic).expect("should derive keypair");

        assert_eq!(keypair.private_key_hex(), expected_private_key);
        assert_eq!(keypair.public_key_hex(), expected_public_key);
        assert_eq!(keypair.nsec().unwrap(), expected_nsec);
        assert_eq!(keypair.npub().unwrap(), expected_npub);
    }

    #[test]
    fn test_mnemonic_to_seed() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let seed = mnemonic_to_seed(mnemonic, "").expect("should parse mnemonic");
        assert_eq!(seed.len(), 64);
    }

    #[test]
    fn test_mnemonic_to_seed_with_passphrase() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let seed_without = mnemonic_to_seed(mnemonic, "").expect("should parse mnemonic");
        let seed_with = mnemonic_to_seed(mnemonic, "my passphrase").expect("should parse mnemonic");

        // Seeds should be different with different passphrases
        assert_ne!(seed_without, seed_with);
    }

    #[test]
    fn test_invalid_mnemonic() {
        let invalid = "invalid mnemonic words that are not valid";
        let result = derive_keypair(invalid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip06Error::InvalidMnemonic(_)
        ));
    }

    #[test]
    fn test_account_derivation() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let keypair_0 = derive_keypair_with_account(mnemonic, 0).expect("should derive account 0");
        let keypair_1 = derive_keypair_with_account(mnemonic, 1).expect("should derive account 1");
        let keypair_2 = derive_keypair_with_account(mnemonic, 2).expect("should derive account 2");

        // All accounts should have different keys
        assert_ne!(keypair_0.private_key, keypair_1.private_key);
        assert_ne!(keypair_1.private_key, keypair_2.private_key);
        assert_ne!(keypair_0.private_key, keypair_2.private_key);

        assert_ne!(keypair_0.public_key, keypair_1.public_key);
        assert_ne!(keypair_1.public_key, keypair_2.public_key);
        assert_ne!(keypair_0.public_key, keypair_2.public_key);
    }

    #[test]
    fn test_agent_derivation_account_offset() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let agent_keypair = derive_agent_keypair(mnemonic, 0).expect("should derive agent 0");
        let account_keypair =
            derive_keypair_with_account(mnemonic, 1).expect("should derive account 1");

        assert_eq!(agent_keypair.private_key, account_keypair.private_key);
        assert_eq!(agent_keypair.public_key, account_keypair.public_key);
    }

    #[test]
    fn test_nsec_roundtrip() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let keypair = derive_keypair(mnemonic).expect("should derive keypair");

        let nsec = keypair.nsec().expect("should encode nsec");
        let decoded = nsec_to_private_key(&nsec).expect("should decode nsec");

        assert_eq!(keypair.private_key, decoded);
    }

    #[test]
    fn test_npub_roundtrip() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let keypair = derive_keypair(mnemonic).expect("should derive keypair");

        let npub = keypair.npub().expect("should encode npub");
        let decoded = npub_to_public_key(&npub).expect("should decode npub");

        assert_eq!(keypair.public_key, decoded);
    }

    #[test]
    fn test_decode_nsec_wrong_hrp() {
        // This is an npub, not an nsec
        let npub = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
        let result = nsec_to_private_key(npub);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip06Error::InvalidHrp { .. }));
    }

    #[test]
    fn test_decode_npub_wrong_hrp() {
        // This is an nsec, not an npub
        let nsec = "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp";
        let result = npub_to_public_key(nsec);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Nip06Error::InvalidHrp { .. }));
    }

    #[test]
    fn test_decode_invalid_bech32() {
        let invalid = "nsec1invalid";
        let result = nsec_to_private_key(invalid);

        assert!(result.is_err());
    }

    #[test]
    fn test_private_key_to_nsec_direct() {
        let private_key_hex = "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a";
        let expected_nsec = "nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp";

        let private_key_bytes = hex::decode(private_key_hex).expect("valid hex");
        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(&private_key_bytes);

        let nsec = private_key_to_nsec(&private_key).expect("should encode");
        assert_eq!(nsec, expected_nsec);
    }

    #[test]
    fn test_public_key_to_npub_direct() {
        let public_key_hex = "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
        let expected_npub = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";

        let public_key_bytes = hex::decode(public_key_hex).expect("valid hex");
        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&public_key_bytes);

        let npub = public_key_to_npub(&public_key).expect("should encode");
        assert_eq!(npub, expected_npub);
    }

    #[test]
    fn test_keypair_debug_redacts_private_key() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let keypair = derive_keypair(mnemonic).expect("should derive keypair");

        let debug_output = format!("{:?}", keypair);
        assert!(debug_output.contains("[redacted]"));
        assert!(!debug_output.contains(&keypair.private_key_hex()));
    }

    proptest! {
        #[test]
        fn prop_nsec_roundtrip(private_key in prop::array::uniform32(any::<u8>())) {
            let nsec = private_key_to_nsec(&private_key).expect("encode nsec");
            let decoded = nsec_to_private_key(&nsec).expect("decode nsec");
            prop_assert_eq!(decoded, private_key);
        }

        #[test]
        fn prop_npub_roundtrip(public_key in prop::array::uniform32(any::<u8>())) {
            let npub = public_key_to_npub(&public_key).expect("encode npub");
            let decoded = npub_to_public_key(&npub).expect("decode npub");
            prop_assert_eq!(decoded, public_key);
        }
    }

    #[test]
    fn test_12_word_mnemonic() {
        // 12-word mnemonic (128-bit entropy)
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let keypair = derive_keypair(mnemonic).expect("should derive keypair");

        // Just verify it works - this is a well-known test vector mnemonic
        assert_eq!(keypair.private_key.len(), 32);
        assert_eq!(keypair.public_key.len(), 32);
    }

    #[test]
    fn test_24_word_mnemonic() {
        // 24-word mnemonic (256-bit entropy)
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        let keypair = derive_keypair(mnemonic).expect("should derive keypair");

        assert_eq!(keypair.private_key.len(), 32);
        assert_eq!(keypair.public_key.len(), 32);
    }

    #[test]
    fn test_deterministic_derivation() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let keypair1 = derive_keypair(mnemonic).expect("should derive keypair");
        let keypair2 = derive_keypair(mnemonic).expect("should derive keypair");

        // Same mnemonic should always produce the same keys
        assert_eq!(keypair1.private_key, keypair2.private_key);
        assert_eq!(keypair1.public_key, keypair2.public_key);
    }

    #[test]
    fn test_different_mnemonics_produce_different_keys() {
        let mnemonic1 =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let mnemonic2 = "what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade";

        let keypair1 = derive_keypair(mnemonic1).expect("should derive keypair");
        let keypair2 = derive_keypair(mnemonic2).expect("should derive keypair");

        assert_ne!(keypair1.private_key, keypair2.private_key);
        assert_ne!(keypair1.public_key, keypair2.public_key);
    }

    #[test]
    fn test_high_account_index() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";

        // Test with high account index (but within valid range for hardened derivation)
        let keypair = derive_keypair_with_account(mnemonic, 0x7FFFFFFF)
            .expect("should derive with max account index");

        assert_eq!(keypair.private_key.len(), 32);
        assert_eq!(keypair.public_key.len(), 32);
    }

    // =========================================================================
    // nostr-tools test vectors (from nip06.test.ts)
    // https://github.com/nbd-wtf/nostr-tools
    // =========================================================================

    /// nostr-tools: generate private key from a mnemonic
    /// mnemonic: zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong
    /// account: 0, passphrase: none
    #[test]
    fn test_nostr_tools_zoo_mnemonic_account_0() {
        let mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let expected_private_key =
            "c26cf31d8ba425b555ca27d00ca71b5008004f2f662470f8c8131822ec129fe2";

        let keypair = derive_keypair(mnemonic).expect("should derive keypair");
        assert_eq!(keypair.private_key_hex(), expected_private_key);
    }

    /// nostr-tools: generate private key for account 1 from a mnemonic
    /// mnemonic: zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong
    /// account: 1, passphrase: none
    #[test]
    fn test_nostr_tools_zoo_mnemonic_account_1() {
        let mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let expected_private_key =
            "b5fc7f229de3fb5c189063e3b3fc6c921d8f4366cff5bd31c6f063493665eb2b";

        let keypair = derive_keypair_with_account(mnemonic, 1).expect("should derive keypair");
        assert_eq!(keypair.private_key_hex(), expected_private_key);
    }

    /// nostr-tools: generate private key from a mnemonic and passphrase
    /// mnemonic: zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong
    /// account: 0, passphrase: "123"
    #[test]
    fn test_nostr_tools_zoo_mnemonic_with_passphrase() {
        let mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let passphrase = "123";
        let expected_private_key =
            "55a22b8203273d0aaf24c22c8fbe99608e70c524b17265641074281c8b978ae4";

        let keypair = derive_keypair_full(mnemonic, passphrase, 0).expect("should derive keypair");
        assert_eq!(keypair.private_key_hex(), expected_private_key);
    }

    /// nostr-tools: generate private key for account 1 from a mnemonic and passphrase
    /// mnemonic: zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong
    /// account: 1, passphrase: "123"
    #[test]
    fn test_nostr_tools_zoo_mnemonic_account_1_with_passphrase() {
        let mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let passphrase = "123";
        let expected_private_key =
            "2e0f7bd9e3c3ebcdff1a90fb49c913477e7c055eba1a415d571b6a8c714c7135";

        let keypair = derive_keypair_full(mnemonic, passphrase, 1).expect("should derive keypair");
        assert_eq!(keypair.private_key_hex(), expected_private_key);
    }

    /// nostr-tools: generate private and public key for account 1 from a mnemonic and passphrase
    /// mnemonic: zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong
    /// account: 1, passphrase: "123"
    /// Verifies both private and public key
    #[test]
    fn test_nostr_tools_zoo_mnemonic_account_1_with_passphrase_full() {
        let mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let passphrase = "123";
        let expected_private_key =
            "2e0f7bd9e3c3ebcdff1a90fb49c913477e7c055eba1a415d571b6a8c714c7135";
        let expected_public_key =
            "13f55f4f01576570ea342eb7d2b611f9dc78f8dc601aeb512011e4e73b90cf0a";

        let keypair = derive_keypair_full(mnemonic, passphrase, 1).expect("should derive keypair");
        assert_eq!(keypair.private_key_hex(), expected_private_key);
        assert_eq!(keypair.public_key_hex(), expected_public_key);
    }

    /// nostr-tools: generate account from extended private key (derived keys test)
    /// This tests that the abandon...about mnemonic produces the expected keys
    /// xprv derived from m/44'/1237'/0' produces specific keys at /0/0
    #[test]
    fn test_nostr_tools_abandon_mnemonic_extended_key_derivation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let expected_private_key =
            "5f29af3b9676180290e77a4efad265c4c2ff28a5302461f73597fda26bb25731";
        let expected_public_key =
            "e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f";

        let keypair = derive_keypair(mnemonic).expect("should derive keypair");
        assert_eq!(keypair.private_key_hex(), expected_private_key);
        assert_eq!(keypair.public_key_hex(), expected_public_key);
    }
}
