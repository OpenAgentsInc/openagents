//! Agent identity types based on Nostr keypairs.
//!
//! Every agent in the OpenAgents ecosystem has a unique identity derived from
//! a Nostr keypair (NIP-06). This provides:
//!
//! - **Global uniqueness**: No central registry needed
//! - **Cryptographic verification**: Sign messages and prove ownership
//! - **Interoperability**: Works with the broader Nostr ecosystem
//! - **Self-sovereign**: Users control their own agent identities

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Errors that can occur with agent identity operations.
#[derive(Debug, Error)]
pub enum AgentIdError {
    #[error("invalid npub format: {0}")]
    InvalidNpub(String),

    #[error("invalid nsec format: {0}")]
    InvalidNsec(String),

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("key derivation error: {0}")]
    KeyDerivation(String),

    #[error("signing error: {0}")]
    Signing(String),
}

/// Universal agent identity based on Nostr public key.
///
/// This is the primary identifier for agents across the ecosystem.
/// It can be shared publicly and used to:
///
/// - Look up agents in registries
/// - Verify message signatures
/// - Route payments via NIP-90
/// - Reference agents in Nostr events
///
/// # Example
///
/// ```rust,ignore
/// use agents::AgentId;
///
/// // From npub string
/// let id = AgentId::from_npub("npub1...")?;
///
/// // Display as npub
/// println!("Agent: {}", id.npub());
///
/// // Use as hex for internal operations
/// println!("Hex: {}", id.to_hex());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentId {
    /// The 32-byte x-only public key
    pubkey: [u8; 32],
}

impl AgentId {
    /// Create an AgentId from raw 32-byte public key.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { pubkey: bytes }
    }

    /// Create an AgentId from a hex-encoded public key.
    pub fn from_hex(hex: &str) -> Result<Self, AgentIdError> {
        let bytes = hex::decode(hex).map_err(|e| AgentIdError::InvalidPublicKey(e.to_string()))?;

        if bytes.len() != 32 {
            return Err(AgentIdError::InvalidPublicKey(format!(
                "expected 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&bytes);
        Ok(Self { pubkey })
    }

    /// Create an AgentId from an npub (bech32-encoded public key).
    pub fn from_npub(npub: &str) -> Result<Self, AgentIdError> {
        use bech32::Hrp;

        let expected = Hrp::parse("npub").expect("valid hrp");
        let (hrp, data) =
            bech32::decode(npub).map_err(|e| AgentIdError::InvalidNpub(e.to_string()))?;

        if hrp != expected {
            return Err(AgentIdError::InvalidNpub(format!(
                "expected npub, got {}",
                hrp
            )));
        }

        let bytes: Vec<u8> = data;
        if bytes.len() != 32 {
            return Err(AgentIdError::InvalidNpub(format!(
                "expected 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(&bytes);
        Ok(Self { pubkey })
    }

    /// Get the raw 32-byte public key.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.pubkey
    }

    /// Get the public key as a hex string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.pubkey)
    }

    /// Get the public key as an npub (bech32-encoded).
    pub fn npub(&self) -> String {
        use bech32::{Bech32, Hrp};

        let hrp = Hrp::parse("npub").expect("valid hrp");
        bech32::encode::<Bech32>(hrp, &self.pubkey).expect("valid encoding")
    }

    /// Check if this is a valid agent ID (non-zero).
    pub fn is_valid(&self) -> bool {
        self.pubkey.iter().any(|&b| b != 0)
    }
}

impl fmt::Debug for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentId")
            .field("npub", &self.npub())
            .finish()
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Show truncated npub for readability
        let npub = self.npub();
        if npub.len() > 20 {
            write!(f, "{}...{}", &npub[..12], &npub[npub.len() - 8..])
        } else {
            write!(f, "{}", npub)
        }
    }
}

impl Serialize for AgentId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as npub for human-readable formats, hex for binary
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.npub())
        } else {
            serializer.serialize_bytes(&self.pubkey)
        }
    }
}

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = String::deserialize(deserializer)?;
            // Try npub first, then hex
            AgentId::from_npub(&s)
                .or_else(|_| AgentId::from_hex(&s))
                .map_err(serde::de::Error::custom)
        } else {
            let bytes = <[u8; 32]>::deserialize(deserializer)?;
            Ok(AgentId::from_bytes(bytes))
        }
    }
}

/// Agent keypair containing both private and public keys.
///
/// This is used for signing messages and proving ownership of an agent identity.
/// The private key should be stored securely and never shared.
///
/// # Example
///
/// ```rust,ignore
/// use agents::AgentKeypair;
///
/// // Generate a new random keypair
/// let keypair = AgentKeypair::generate()?;
///
/// // Or derive from mnemonic (NIP-06)
/// let keypair = AgentKeypair::from_mnemonic("word1 word2 ... word12")?;
///
/// // Get the public identity
/// let id = keypair.agent_id();
///
/// // Sign a message
/// let signature = keypair.sign(b"hello")?;
/// ```
#[derive(Clone)]
pub struct AgentKeypair {
    /// The 32-byte private key
    private_key: [u8; 32],
    /// The 32-byte public key
    public_key: [u8; 32],
}

impl AgentKeypair {
    /// Create a keypair from raw bytes.
    pub fn from_bytes(private_key: [u8; 32], public_key: [u8; 32]) -> Self {
        Self {
            private_key,
            public_key,
        }
    }

    /// Generate a new random keypair.
    #[cfg(feature = "rand")]
    pub fn generate() -> Result<Self, AgentIdError> {
        use rand::RngCore;

        let mut private_key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut private_key);

        Self::from_private_key(private_key)
    }

    /// Create a keypair from a private key.
    pub fn from_private_key(private_key: [u8; 32]) -> Result<Self, AgentIdError> {
        use bitcoin::key::Secp256k1;
        use bitcoin::secp256k1::{PublicKey, SecretKey};

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&private_key)
            .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?;

        let public_key_full = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_bytes = public_key_full.serialize();

        // Extract x-only public key (skip prefix byte)
        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&public_key_bytes[1..33]);

        Ok(Self {
            private_key,
            public_key,
        })
    }

    /// Derive a keypair from a BIP39 mnemonic (NIP-06).
    ///
    /// Uses derivation path `m/44'/1237'/0'/0/0` (Nostr coin type).
    pub fn from_mnemonic(mnemonic: &str) -> Result<Self, AgentIdError> {
        Self::from_mnemonic_with_account(mnemonic, 0)
    }

    /// Derive a keypair from a mnemonic with a specific account index.
    ///
    /// Uses derivation path `m/44'/1237'/<account>'/0/0`.
    pub fn from_mnemonic_with_account(mnemonic: &str, account: u32) -> Result<Self, AgentIdError> {
        Self::from_mnemonic_full(mnemonic, "", account)
    }

    /// Derive a keypair from a mnemonic with passphrase and account index.
    ///
    /// Uses derivation path `m/44'/1237'/<account>'/0/0`.
    ///
    /// The passphrase is used during BIP39 seed derivation (not for encryption).
    /// An empty passphrase is equivalent to no passphrase.
    pub fn from_mnemonic_full(
        mnemonic: &str,
        passphrase: &str,
        account: u32,
    ) -> Result<Self, AgentIdError> {
        use bip39::Mnemonic;
        use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv};
        use bitcoin::key::Secp256k1;
        use bitcoin::secp256k1::{PublicKey, SecretKey};
        use bitcoin::Network;

        const NOSTR_COIN_TYPE: u32 = 1237;

        // Parse mnemonic and derive seed
        let mnemonic = Mnemonic::parse(mnemonic)
            .map_err(|e| AgentIdError::KeyDerivation(format!("invalid mnemonic: {}", e)))?;
        let seed = mnemonic.to_seed(passphrase);

        let secp = Secp256k1::new();

        // Create master key
        let master = Xpriv::new_master(Network::Bitcoin, &seed)
            .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?;

        // Build derivation path: m/44'/1237'/<account>'/0/0
        let path = DerivationPath::from(vec![
            ChildNumber::from_hardened_idx(44)
                .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_hardened_idx(NOSTR_COIN_TYPE)
                .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_hardened_idx(account)
                .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_normal_idx(0)
                .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?,
            ChildNumber::from_normal_idx(0)
                .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?,
        ]);

        // Derive child key
        let derived = master
            .derive_priv(&secp, &path)
            .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?;

        let private_key: [u8; 32] = derived.private_key.secret_bytes();

        // Derive public key
        let secret_key = SecretKey::from_slice(&private_key)
            .map_err(|e| AgentIdError::KeyDerivation(e.to_string()))?;
        let public_key_full = PublicKey::from_secret_key(&secp, &secret_key);
        let public_key_bytes = public_key_full.serialize();

        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&public_key_bytes[1..33]);

        Ok(Self {
            private_key,
            public_key,
        })
    }

    /// Get the agent ID (public identity).
    pub fn agent_id(&self) -> AgentId {
        AgentId::from_bytes(self.public_key)
    }

    /// Get the private key as a hex string.
    pub fn private_key_hex(&self) -> String {
        hex::encode(self.private_key)
    }

    /// Get the private key as an nsec (bech32-encoded).
    pub fn nsec(&self) -> String {
        use bech32::{Bech32, Hrp};

        let hrp = Hrp::parse("nsec").expect("valid hrp");
        bech32::encode::<Bech32>(hrp, &self.private_key).expect("valid encoding")
    }

    /// Create from an nsec (bech32-encoded private key).
    pub fn from_nsec(nsec: &str) -> Result<Self, AgentIdError> {
        use bech32::Hrp;

        let expected = Hrp::parse("nsec").expect("valid hrp");
        let (hrp, data) =
            bech32::decode(nsec).map_err(|e| AgentIdError::InvalidNsec(e.to_string()))?;

        if hrp != expected {
            return Err(AgentIdError::InvalidNsec(format!(
                "expected nsec, got {}",
                hrp
            )));
        }

        let bytes: Vec<u8> = data;
        if bytes.len() != 32 {
            return Err(AgentIdError::InvalidNsec(format!(
                "expected 32 bytes, got {}",
                bytes.len()
            )));
        }

        let mut private_key = [0u8; 32];
        private_key.copy_from_slice(&bytes);

        Self::from_private_key(private_key)
    }

    /// Sign a message with this keypair.
    ///
    /// Returns a 64-byte Schnorr signature.
    pub fn sign(&self, message: &[u8]) -> Result<[u8; 64], AgentIdError> {
        use bitcoin::hashes::{sha256, Hash};
        use bitcoin::key::Secp256k1;
        use bitcoin::secp256k1::{Message, SecretKey};

        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&self.private_key)
            .map_err(|e| AgentIdError::Signing(e.to_string()))?;

        let keypair = bitcoin::secp256k1::Keypair::from_secret_key(&secp, &secret_key);

        // Hash the message (Nostr uses SHA256)
        let hash = sha256::Hash::hash(message);
        let msg = Message::from_digest(*hash.as_ref());

        // Create Schnorr signature
        let sig = secp.sign_schnorr_no_aux_rand(&msg, &keypair);

        Ok(*sig.as_ref())
    }

    /// Verify a signature against a public key.
    pub fn verify(public_key: &AgentId, message: &[u8], signature: &[u8; 64]) -> bool {
        use bitcoin::hashes::{sha256, Hash};
        use bitcoin::key::Secp256k1;
        use bitcoin::secp256k1::{schnorr::Signature, Message, XOnlyPublicKey};

        let secp = Secp256k1::new();

        let Ok(xonly) = XOnlyPublicKey::from_slice(public_key.as_bytes()) else {
            return false;
        };

        let hash = sha256::Hash::hash(message);
        let msg = Message::from_digest(*hash.as_ref());

        let Ok(sig) = Signature::from_slice(signature) else {
            return false;
        };

        secp.verify_schnorr(&sig, &msg, &xonly).is_ok()
    }
}

impl fmt::Debug for AgentKeypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentKeypair")
            .field("agent_id", &self.agent_id())
            .field("private_key", &"[redacted]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_from_hex() {
        let hex = "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
        let id = AgentId::from_hex(hex).unwrap();
        assert_eq!(id.to_hex(), hex);
    }

    #[test]
    fn test_agent_id_from_npub() {
        let npub = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
        let id = AgentId::from_npub(npub).unwrap();
        assert_eq!(id.npub(), npub);
    }

    #[test]
    fn test_agent_id_roundtrip() {
        let hex = "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
        let id = AgentId::from_hex(hex).unwrap();
        let npub = id.npub();
        let id2 = AgentId::from_npub(&npub).unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn test_keypair_from_mnemonic() {
        // NIP-06 test vector
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let keypair = AgentKeypair::from_mnemonic(mnemonic).unwrap();

        assert_eq!(
            keypair.private_key_hex(),
            "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a"
        );
        assert_eq!(
            keypair.agent_id().to_hex(),
            "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917"
        );
    }

    #[test]
    fn test_keypair_nsec_roundtrip() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let keypair = AgentKeypair::from_mnemonic(mnemonic).unwrap();
        let nsec = keypair.nsec();
        let keypair2 = AgentKeypair::from_nsec(&nsec).unwrap();
        assert_eq!(keypair.agent_id(), keypair2.agent_id());
    }

    #[test]
    fn test_sign_verify() {
        let mnemonic =
            "leader monkey parrot ring guide accident before fence cannon height naive bean";
        let keypair = AgentKeypair::from_mnemonic(mnemonic).unwrap();

        let message = b"Hello, OpenAgents!";
        let signature = keypair.sign(message).unwrap();

        assert!(AgentKeypair::verify(&keypair.agent_id(), message, &signature));
        assert!(!AgentKeypair::verify(
            &keypair.agent_id(),
            b"wrong message",
            &signature
        ));
    }

    #[test]
    fn test_agent_id_serialization() {
        let id =
            AgentId::from_hex("17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917")
                .unwrap();

        let json = serde_json::to_string(&id).unwrap();
        assert!(json.contains("npub"));

        let deserialized: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }
}
