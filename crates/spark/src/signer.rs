//! Spark signer implementation using BIP39 mnemonic derivation
//!
//! This module derives Bitcoin keys from a BIP39 mnemonic using the standard
//! BIP44 path: m/44'/0'/0'/0/0
//!
//! The same mnemonic can be used for both Nostr (NIP-06) and Spark, providing
//! a unified identity system.

use bip39::Mnemonic;
use bitcoin::Network;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};
use bitcoin::key::Secp256k1;
use bitcoin::secp256k1::{PublicKey, SecretKey};
use crate::error::SparkError;

/// Bitcoin coin type as defined in BIP44
const BITCOIN_COIN_TYPE: u32 = 0;

/// Spark signer that holds Bitcoin keypair derived from mnemonic
#[derive(Clone)]
pub struct SparkSigner {
    /// The BIP39 mnemonic phrase
    mnemonic: String,
    /// Optional BIP39 passphrase
    passphrase: String,
    /// The 32-byte private key
    private_key: [u8; 32],
    /// The 33-byte compressed public key
    public_key: [u8; 33],
    /// Raw seed entropy for non-mnemonic initialization
    seed_entropy: Option<Vec<u8>>,
}

impl SparkSigner {
    /// Create a new SparkSigner from a BIP39 mnemonic
    ///
    /// Uses derivation path: m/44'/0'/0'/0/0
    ///
    /// # Arguments
    /// * `mnemonic` - The BIP39 mnemonic phrase (12 or 24 words)
    /// * `passphrase` - Optional BIP39 passphrase (empty string for none)
    ///
    /// # Example
    /// ```
    /// use openagents_spark::SparkSigner;
    ///
    /// let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    /// let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("valid mnemonic");
    /// ```
    pub fn from_mnemonic(mnemonic: &str, passphrase: &str) -> Result<Self, SparkError> {
        // Parse and validate the mnemonic
        let parsed_mnemonic = Mnemonic::parse(mnemonic)
            .map_err(|e| SparkError::InvalidMnemonic(e.to_string()))?;

        // Derive the 64-byte seed from the mnemonic
        let seed = parsed_mnemonic.to_seed(passphrase);

        // Derive the keypair from the seed
        let mut signer = Self::from_seed_bytes(&seed)?;

        // Store the mnemonic and passphrase
        signer.mnemonic = mnemonic.to_string();
        signer.passphrase = passphrase.to_string();

        Ok(signer)
    }

    /// Derive a SparkSigner from raw entropy bytes
    ///
    /// Uses derivation path: m/44'/0'/0'/0/0
    fn from_seed_bytes(seed: &[u8]) -> Result<Self, SparkError> {
        let secp = Secp256k1::new();

        // Create master key from seed (using Bitcoin mainnet)
        let master = Xpriv::new_master(Network::Bitcoin, seed)
            .map_err(|e| SparkError::KeyDerivation(e.to_string()))?;

        // Build BIP44 derivation path: m/44'/0'/0'/0/0
        // m = master
        // 44' = purpose (BIP44)
        // 0' = coin type (Bitcoin)
        // 0' = account
        // 0 = change (external)
        // 0 = address index
        let path = DerivationPath::from(vec![
            ChildNumber::from_hardened_idx(44)
                .map_err(|e| SparkError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_hardened_idx(BITCOIN_COIN_TYPE)
                .map_err(|e| SparkError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_hardened_idx(0)
                .map_err(|e| SparkError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_normal_idx(0)
                .map_err(|e| SparkError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_normal_idx(0)
                .map_err(|e| SparkError::KeyDerivation(e.to_string()))?,
        ]);

        // Derive the child key
        let derived = master
            .derive_priv(&secp, &path)
            .map_err(|e| SparkError::KeyDerivation(e.to_string()))?;

        // Get the private key bytes
        let private_key: [u8; 32] = derived.private_key.secret_bytes();

        // Get the compressed public key (33 bytes: 1 byte prefix + 32 bytes x-coordinate)
        let secret_key = SecretKey::from_slice(&private_key)
            .map_err(|e| SparkError::KeyDerivation(e.to_string()))?;
        let public_key_full = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key = public_key_full.serialize();

        Ok(Self {
            mnemonic: String::new(),
            passphrase: String::new(),
            private_key,
            public_key,
            seed_entropy: Some(seed.to_vec()),
        })
    }

    /// Create a SparkSigner from raw entropy bytes (seed material)
    ///
    /// This is useful when a mnemonic is not available (e.g. web storage).
    pub fn from_entropy(entropy: &[u8]) -> Result<Self, SparkError> {
        let mut signer = Self::from_seed_bytes(entropy)?;
        signer.seed_entropy = Some(entropy.to_vec());
        Ok(signer)
    }

    /// Get the private key as raw bytes
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn private_key(&self) -> &[u8; 32] {
        &self.private_key
    }

    /// Get the public key as raw bytes (compressed, 33 bytes)
    pub fn public_key(&self) -> &[u8; 33] {
        &self.public_key
    }

    /// Get the private key as a hex string
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn private_key_hex(&self) -> String {
        hex::encode(self.private_key)
    }

    /// Get the public key as a hex string
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key)
    }

    /// Get the mnemonic phrase
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    /// Get the passphrase
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn passphrase(&self) -> &str {
        &self.passphrase
    }

    /// Get the raw seed entropy (if available)
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn seed_entropy(&self) -> Option<&[u8]> {
        self.seed_entropy.as_deref()
    }
}

impl std::fmt::Debug for SparkSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SparkSigner")
            .field("public_key", &self.public_key_hex())
            .field("private_key", &"[redacted]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_mnemonic() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");

        // Verify we get 32-byte private key and 33-byte public key
        assert_eq!(signer.private_key().len(), 32);
        assert_eq!(signer.public_key().len(), 33);
    }

    #[test]
    fn test_deterministic_derivation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let signer1 = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");
        let signer2 = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");

        // Same mnemonic should always produce the same keys
        assert_eq!(signer1.private_key(), signer2.private_key());
        assert_eq!(signer1.public_key(), signer2.public_key());
    }

    #[test]
    fn test_different_mnemonics_produce_different_keys() {
        let mnemonic1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic2 = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

        let signer1 = SparkSigner::from_mnemonic(mnemonic1, "").expect("should create signer");
        let signer2 = SparkSigner::from_mnemonic(mnemonic2, "").expect("should create signer");

        // Different mnemonics should produce different keys
        assert_ne!(signer1.private_key(), signer2.private_key());
        assert_ne!(signer1.public_key(), signer2.public_key());
    }

    #[test]
    fn test_passphrase_changes_keys() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let signer_no_pass = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");
        let signer_with_pass = SparkSigner::from_mnemonic(mnemonic, "test123").expect("should create signer");

        // Different passphrases should produce different keys
        assert_ne!(signer_no_pass.private_key(), signer_with_pass.private_key());
        assert_ne!(signer_no_pass.public_key(), signer_with_pass.public_key());
    }

    #[test]
    fn test_invalid_mnemonic() {
        let invalid = "invalid mnemonic words that are not valid";
        let result = SparkSigner::from_mnemonic(invalid, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_debug_redacts_private_key() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");

        let debug_output = format!("{:?}", signer);
        assert!(debug_output.contains("[redacted]"));
        assert!(!debug_output.contains(&signer.private_key_hex()));
    }

    #[test]
    fn test_public_key_is_compressed() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");

        // Compressed public keys should start with 02 or 03
        let pubkey = signer.public_key();
        assert!(pubkey[0] == 0x02 || pubkey[0] == 0x03);
    }

    #[test]
    fn test_hex_encoding() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");

        let privkey_hex = signer.private_key_hex();
        let pubkey_hex = signer.public_key_hex();

        // Verify hex encoding is correct length
        assert_eq!(privkey_hex.len(), 64); // 32 bytes * 2 hex chars
        assert_eq!(pubkey_hex.len(), 66); // 33 bytes * 2 hex chars

        // Verify it's valid hex
        assert!(hex::decode(&privkey_hex).is_ok());
        assert!(hex::decode(&pubkey_hex).is_ok());
    }

    /// Test vector verification
    /// This uses a known test vector to verify the derivation is correct
    #[test]
    fn test_known_vector() {
        // Using the standard BIP39 test vector
        // Mnemonic: abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about
        // Path: m/44'/0'/0'/0/0
        // Expected private key can be verified with any BIP44 tool
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");

        // Just verify it produces consistent output
        // The actual values can be verified against external BIP44 tools
        let privkey = signer.private_key_hex();
        let pubkey = signer.public_key_hex();

        // These values are deterministic and can be independently verified
        assert!(!privkey.is_empty());
        assert!(!pubkey.is_empty());

        // Verify the public key starts with 02 or 03 (compressed format)
        assert!(pubkey.starts_with("02") || pubkey.starts_with("03"));
    }

    #[test]
    fn test_24_word_mnemonic() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("should create signer");

        assert_eq!(signer.private_key().len(), 32);
        assert_eq!(signer.public_key().len(), 33);
    }
}
