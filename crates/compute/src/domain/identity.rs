//! Unified identity management using BIP32 seed phrase
//!
//! The same seed phrase is used to derive:
//! - Nostr keypair via NIP-06 (m/44'/1237'/0'/0/0)
//! - Spark wallet signer via BIP44 (m/44'/0'/0'/0/0)

use bip39::Mnemonic;
use nostr::{Keypair, Nip06Error, derive_keypair_full};
use spark::{SparkSigner, SparkError};
use thiserror::Error;

/// Errors that can occur during identity operations
#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("bech32 encoding failed: {0}")]
    Bech32(String),

    #[error("spark wallet error: {0}")]
    SparkWallet(String),
}

impl From<Nip06Error> for IdentityError {
    fn from(e: Nip06Error) -> Self {
        match e {
            Nip06Error::InvalidMnemonic(s) => IdentityError::InvalidMnemonic(s),
            Nip06Error::KeyDerivation(s) => IdentityError::KeyDerivation(s),
            Nip06Error::Bech32Encode(s) | Nip06Error::Bech32Decode(s) => {
                IdentityError::Bech32(s)
            }
            Nip06Error::InvalidKeyFormat(s) => IdentityError::KeyDerivation(s),
            Nip06Error::InvalidHrp { expected, got } => {
                IdentityError::Bech32(format!("expected {expected}, got {got}"))
            }
        }
    }
}

impl From<SparkError> for IdentityError {
    fn from(e: SparkError) -> Self {
        match e {
            SparkError::InvalidMnemonic(s) => IdentityError::InvalidMnemonic(s),
            SparkError::KeyDerivation(s) => IdentityError::KeyDerivation(s),
            _ => IdentityError::SparkWallet(e.to_string()),
        }
    }
}

/// Unified identity that manages both Nostr and Bitcoin/Spark keys from a single seed
#[derive(Clone)]
pub struct UnifiedIdentity {
    /// The BIP39 mnemonic (12 or 24 words)
    mnemonic: String,
    /// Nostr keypair derived via NIP-06 (m/44'/1237'/0'/0/0)
    nostr_keypair: Keypair,
    /// Spark signer derived via BIP44 (m/44'/0'/0'/0/0)
    spark_signer: SparkSigner,
}

impl UnifiedIdentity {
    /// Generate a new identity with a fresh 12-word mnemonic
    pub fn generate() -> Result<Self, IdentityError> {
        use rand::RngCore;
        let mut entropy = [0u8; 16]; // 128 bits for 12 words
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| IdentityError::InvalidMnemonic(e.to_string()))?;

        Self::from_mnemonic(&mnemonic.to_string(), "")
    }

    /// Generate a new identity with a 24-word mnemonic (more secure)
    pub fn generate_24_words() -> Result<Self, IdentityError> {
        use rand::RngCore;
        let mut entropy = [0u8; 32]; // 256 bits for 24 words
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| IdentityError::InvalidMnemonic(e.to_string()))?;

        Self::from_mnemonic(&mnemonic.to_string(), "")
    }

    /// Create identity from an existing mnemonic
    ///
    /// The passphrase is optional (empty string for none) and is used
    /// during BIP39 seed derivation, not for encryption.
    pub fn from_mnemonic(mnemonic: &str, passphrase: &str) -> Result<Self, IdentityError> {
        // Validate the mnemonic
        let _parsed = Mnemonic::parse(mnemonic)
            .map_err(|e| IdentityError::InvalidMnemonic(e.to_string()))?;

        // Derive Nostr keypair using NIP-06 (m/44'/1237'/0'/0/0)
        let nostr_keypair = derive_keypair_full(mnemonic, passphrase, 0)?;

        // Derive Spark signer using BIP44 (m/44'/0'/0'/0/0)
        let spark_signer = SparkSigner::from_mnemonic(mnemonic, passphrase)?;

        Ok(Self {
            mnemonic: mnemonic.to_string(),
            nostr_keypair,
            spark_signer,
        })
    }

    /// Get the mnemonic words (for backup display)
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn mnemonic_words(&self) -> Vec<&str> {
        self.mnemonic.split_whitespace().collect()
    }

    /// Get the raw mnemonic string
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    /// Get the Nostr public key in bech32 format (npub1...)
    pub fn npub(&self) -> Result<String, IdentityError> {
        Ok(self.nostr_keypair.npub()?)
    }

    /// Get the Nostr private key in bech32 format (nsec1...)
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn nsec(&self) -> Result<String, IdentityError> {
        Ok(self.nostr_keypair.nsec()?)
    }

    /// Get the Nostr public key as hex
    pub fn public_key_hex(&self) -> String {
        self.nostr_keypair.public_key_hex()
    }

    /// Get the Nostr private key as raw bytes
    ///
    /// WARNING: Handle with care - this is sensitive data
    pub fn private_key_bytes(&self) -> &[u8; 32] {
        &self.nostr_keypair.private_key
    }

    /// Get the Nostr public key as raw bytes
    pub fn public_key_bytes(&self) -> &[u8; 32] {
        &self.nostr_keypair.public_key
    }

    /// Get a truncated npub for display (npub1abc...xyz)
    pub fn npub_short(&self) -> String {
        match self.npub() {
            Ok(npub) => {
                if npub.len() > 16 {
                    format!("{}...{}", &npub[..12], &npub[npub.len() - 4..])
                } else {
                    npub
                }
            }
            Err(_) => "unknown".to_string(),
        }
    }

    /// Get the Nostr keypair for signing events
    pub fn keypair(&self) -> &Keypair {
        &self.nostr_keypair
    }

    /// Get the Spark signer for Bitcoin payments
    pub fn spark_signer(&self) -> &SparkSigner {
        &self.spark_signer
    }

    /// Get the Spark public key as hex (for display)
    pub fn spark_public_key_hex(&self) -> String {
        self.spark_signer.public_key_hex()
    }
}

impl std::fmt::Debug for UnifiedIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedIdentity")
            .field("npub", &self.npub_short())
            .field("mnemonic", &"[redacted]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let identity = UnifiedIdentity::generate().expect("should generate identity");

        // Should have 12 words
        assert_eq!(identity.mnemonic_words().len(), 12);

        // Should produce valid npub
        let npub = identity.npub().expect("should get npub");
        assert!(npub.starts_with("npub1"));

        // Should produce valid nsec
        let nsec = identity.nsec().expect("should get nsec");
        assert!(nsec.starts_with("nsec1"));
    }

    #[test]
    fn test_generate_24_words() {
        let identity = UnifiedIdentity::generate_24_words().expect("should generate identity");
        assert_eq!(identity.mnemonic_words().len(), 24);
    }

    #[test]
    fn test_from_known_mnemonic() {
        // Using NIP-06 test vector
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "").expect("should create identity");

        let expected_npub = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
        assert_eq!(identity.npub().unwrap(), expected_npub);
    }

    #[test]
    fn test_deterministic_derivation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let identity1 = UnifiedIdentity::from_mnemonic(mnemonic, "").unwrap();
        let identity2 = UnifiedIdentity::from_mnemonic(mnemonic, "").unwrap();

        assert_eq!(identity1.npub().unwrap(), identity2.npub().unwrap());
    }

    #[test]
    fn test_invalid_mnemonic() {
        let result = UnifiedIdentity::from_mnemonic("invalid mnemonic words", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_npub_short() {
        let identity = UnifiedIdentity::generate().expect("should generate identity");
        let short = identity.npub_short();

        assert!(short.starts_with("npub1"));
        assert!(short.contains("..."));
        assert!(short.len() < 20);
    }

    #[test]
    fn test_spark_integration() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "").expect("should create identity");

        // Should have spark signer
        let signer = identity.spark_signer();
        assert_eq!(signer.public_key().len(), 33);

        // Should produce hex public key
        let pubkey_hex = identity.spark_public_key_hex();
        assert_eq!(pubkey_hex.len(), 66); // 33 bytes * 2 hex chars

        // Should start with 02 or 03 (compressed format)
        assert!(pubkey_hex.starts_with("02") || pubkey_hex.starts_with("03"));
    }

    #[test]
    fn test_spark_deterministic_derivation() {
        let mnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean";

        let identity1 = UnifiedIdentity::from_mnemonic(mnemonic, "").unwrap();
        let identity2 = UnifiedIdentity::from_mnemonic(mnemonic, "").unwrap();

        // Same mnemonic should produce same Spark keys
        assert_eq!(
            identity1.spark_public_key_hex(),
            identity2.spark_public_key_hex()
        );
    }

    #[test]
    fn test_unified_identity_both_keys() {
        let mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let identity = UnifiedIdentity::from_mnemonic(mnemonic, "").expect("should create identity");

        // Should have both Nostr and Spark keys
        let npub = identity.npub().expect("should have npub");
        assert!(npub.starts_with("npub1"));

        let spark_pubkey = identity.spark_public_key_hex();
        assert!(spark_pubkey.starts_with("02") || spark_pubkey.starts_with("03"));

        // The keys should be different (different derivation paths)
        let nostr_hex = identity.public_key_hex();
        assert_ne!(nostr_hex, spark_pubkey);
    }
}
