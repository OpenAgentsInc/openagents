//! Unified identity management
//!
//! Combines Nostr identity (NIP-06) with Bitcoin wallet (Spark)
//! Both derived from the same BIP39 mnemonic seed.

use anyhow::Result;
use bip39::Mnemonic;
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;
use std::str::FromStr;

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

        // Derive Nostr keys using NIP-06 path: m/44'/1237'/0'/0/0
        let nostr_path = DerivationPath::from_str("m/44'/1237'/0'/0/0")?;
        let secp = Secp256k1::new();
        let nostr_xpriv = Xpriv::new_master(Network::Bitcoin, &seed)?
            .derive_priv(&secp, &nostr_path)?;

        // Convert to Nostr keys (hex format for now)
        let nostr_secret_key = hex::encode(nostr_xpriv.private_key.secret_bytes());

        // Derive public key
        let public_key = nostr_xpriv.private_key.public_key(&secp);
        let nostr_public_key = hex::encode(public_key.serialize());

        // Derive Bitcoin keys using BIP44 path: m/44'/0'/0'/0/0
        let bitcoin_path = DerivationPath::from_str("m/44'/0'/0'/0/0")?;
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
    pub fn npub(&self) -> String {
        // For now, return hex format
        // TODO: Implement bech32 encoding for npub
        format!("npub1{}", &self.nostr_public_key[..56])
    }

    /// Get profile metadata (placeholder)
    pub async fn get_profile(&self) -> Result<Option<ProfileMetadata>> {
        // TODO: Fetch from Nostr relays
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
}
