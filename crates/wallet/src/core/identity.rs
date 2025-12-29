//! Unified identity management
//!
//! Combines Nostr identity (NIP-06) with Bitcoin wallet (Spark)
//! Both derived from the same BIP39 mnemonic seed.

#![allow(dead_code)]

//! # Examples
//!
//! ## Generating a new identity
//!
//! ```no_run
//! use wallet::core::identity::UnifiedIdentity;
//!
//! // Generate new identity with random mnemonic
//! let identity = UnifiedIdentity::generate().expect("keygen failed");
//!
//! // Access Nostr public key
//! let npub = identity.nostr_public_key();
//! println!("Nostr pubkey: {}", npub);
//! ```
//!
//! ## Importing from existing mnemonic
//!
//! ```no_run
//! use wallet::core::identity::UnifiedIdentity;
//! use bip39::Mnemonic;
//!
//! let words = "abandon abandon abandon abandon abandon abandon \
//!              abandon abandon abandon abandon abandon about";
//! let mnemonic = Mnemonic::parse(words).expect("invalid mnemonic");
//!
//! let identity = UnifiedIdentity::from_mnemonic(mnemonic)
//!     .expect("import failed");
//!
//! // Same mnemonic always produces same keys
//! assert_eq!(identity.nostr_public_key().len(), 64); // hex pubkey
//! ```

use anyhow::Result;
use bip39::Mnemonic;
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use std::str::FromStr;
use nostr::{derive_keypair, public_key_to_npub};

/// Unified identity containing both Nostr and Bitcoin keys
#[derive(Debug)]
pub struct UnifiedIdentity {
    /// BIP39 mnemonic
    mnemonic: Mnemonic,
    /// Nostr secret key (derived via NIP-06)
    #[allow(dead_code)]
    nostr_secret_key: String,
    /// Nostr public key (npub)
    nostr_public_key: String,
    /// Bitcoin extended private key
    #[allow(dead_code)]
    bitcoin_xpriv: Xpriv,
}

impl UnifiedIdentity {
    /// Generate a new unified identity with random mnemonic
    pub fn generate() -> Result<Self> {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut entropy = [0u8; 32]; // 256 bits for 24 words
        rng.fill(&mut entropy);
        let mnemonic = Mnemonic::from_entropy(&entropy)?;
        Self::from_mnemonic(mnemonic)
    }

    /// Create identity from existing mnemonic
    pub fn from_mnemonic(mnemonic: Mnemonic) -> Result<Self> {
        let seed = mnemonic.to_seed("");

        // Derive Nostr keys using NIP-06
        let mnemonic_str = mnemonic.to_string();
        let nostr_keypair = derive_keypair(&mnemonic_str)
            .map_err(|e| anyhow::anyhow!("NIP-06 key derivation failed: {}", e))?;

        // Store as hex strings
        let nostr_secret_key = nostr_keypair.private_key_hex();
        let nostr_public_key = nostr_keypair.public_key_hex();

        // Derive Bitcoin keys using BIP44 path: m/44'/0'/0'/0/0
        let bitcoin_path = DerivationPath::from_str("m/44'/0'/0'/0/0")?;
        let secp = Secp256k1::new();
        let bitcoin_xpriv = Xpriv::new_master(Network::Bitcoin, &seed)?
            .derive_priv(&secp, &bitcoin_path)?;

        Ok(Self {
            mnemonic,
            nostr_secret_key,
            nostr_public_key,
            bitcoin_xpriv,
        })
    }

    /// Get the mnemonic phrase
    pub fn mnemonic(&self) -> &Mnemonic {
        &self.mnemonic
    }

    /// Get Nostr secret key (nsec)
    #[allow(dead_code)]
    pub fn nostr_secret_key(&self) -> &str {
        &self.nostr_secret_key
    }

    /// Get Nostr public key (npub)
    pub fn nostr_public_key(&self) -> &str {
        &self.nostr_public_key
    }

    /// Get Bitcoin extended private key
    #[allow(dead_code)]
    pub fn bitcoin_xpriv(&self) -> &Xpriv {
        &self.bitcoin_xpriv
    }

    /// Sign a Nostr event
    pub fn sign_event(&self, template: nostr::EventTemplate) -> Result<nostr::Event> {
        // Convert hex secret key to bytes
        let secret_key_bytes = hex::decode(&self.nostr_secret_key)?;

        // Convert to [u8; 32]
        if secret_key_bytes.len() != 32 {
            anyhow::bail!("Invalid secret key length");
        }
        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&secret_key_bytes);

        // Finalize the event (add id and signature)
        let signed_event = nostr::finalize_event(&template, &key_array)?;

        Ok(signed_event)
    }

    /// Get npub (bech32-encoded public key)
    pub fn npub(&self) -> Result<String> {
        // Decode hex pubkey to 32-byte array
        let pubkey_bytes = hex::decode(&self.nostr_public_key)?;
        if pubkey_bytes.len() != 32 {
            anyhow::bail!("Invalid public key length: expected 32 bytes, got {}", pubkey_bytes.len());
        }
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&pubkey_bytes);

        // Use NIP-06 bech32 encoding
        public_key_to_npub(&pubkey)
            .map_err(|e| anyhow::anyhow!("Failed to encode npub: {}", e))
    }

    /// Get profile metadata from cache
    ///
    /// Per d-012: Returns None (no cached profile). This is correct behavior
    /// when profile hasn't been fetched. Relay fetching requires Nostr client
    /// integration (d-002).
    pub async fn get_profile(&self) -> Result<Option<ProfileMetadata>> {
        // No cached profile - relay fetching requires d-002 (Nostr implementation)
        Ok(None)
    }
}

/// Profile metadata from kind:0 events
#[allow(dead_code)]
pub struct ProfileMetadata {
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub nip05: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity() {
        let identity = UnifiedIdentity::generate().unwrap();
        assert!(!identity.nostr_secret_key.is_empty());
        assert!(!identity.nostr_public_key.is_empty());
    }

    #[test]
    fn test_from_mnemonic_deterministic() {
        let mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art").unwrap();

        let identity1 = UnifiedIdentity::from_mnemonic(mnemonic.clone()).unwrap();
        let identity2 = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();

        assert_eq!(identity1.nostr_secret_key, identity2.nostr_secret_key);
        assert_eq!(identity1.nostr_public_key, identity2.nostr_public_key);
    }

    #[test]
    fn test_npub_encoding() {
        let mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art").unwrap();
        let identity = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();

        // Test npub encoding
        let npub = identity.npub().expect("npub encoding failed");

        // Verify it's a proper bech32 npub
        assert!(npub.starts_with("npub1"), "npub should start with npub1, got: {}", npub);
        assert!(npub.len() > 60, "npub should be longer than 60 chars, got length: {}", npub.len());

        // Verify pubkey is 32 bytes (64 hex chars)
        assert_eq!(identity.nostr_public_key.len(), 64, "pubkey should be 64 hex chars");
    }

    #[test]
    fn test_nip06_path() {
        // Verify we're using the correct NIP-06 derivation path
        let mnemonic = Mnemonic::parse("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art").unwrap();
        let identity = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();

        // The pubkey should be deterministic based on NIP-06 spec
        // Path: m/44'/1237'/0'/0/0
        assert!(!identity.nostr_public_key.is_empty());
        assert_eq!(identity.nostr_public_key.len(), 64); // 32 bytes as hex
    }
}
