//! Identity and signing abstractions.

use crate::error::Result;
use crate::types::AgentId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Public key wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicKey(Vec<u8>);

impl PublicKey {
    /// Create a public key from bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Return the public key as hex.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
}

/// Signature wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Signature(Vec<u8>);

impl Signature {
    /// Create a signature from bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Return the signature as hex.
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
}

/// Factotum-style signing service.
pub trait SigningService: Send + Sync {
    /// Get the public key for an agent.
    fn pubkey(&self, agent_id: &AgentId) -> Result<PublicKey>;

    /// Sign data (agent requests signature, never sees private key).
    fn sign(&self, agent_id: &AgentId, data: &[u8]) -> Result<Signature>;

    /// Verify a signature from any key.
    fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool;

    /// Encrypt to recipient.
    fn encrypt(&self, agent_id: &AgentId, recipient: &PublicKey, plaintext: &[u8]) -> Result<Vec<u8>>;

    /// Decrypt from sender.
    fn decrypt(&self, agent_id: &AgentId, sender: &PublicKey, ciphertext: &[u8]) -> Result<Vec<u8>>;
}

/// In-memory stub signer using deterministic hashes.
#[derive(Default)]
pub struct InMemorySigner;

impl InMemorySigner {
    /// Create a new stub signer.
    pub fn new() -> Self {
        Self
    }

    fn derive_pubkey(agent_id: &AgentId) -> PublicKey {
        let mut hasher = Sha256::new();
        hasher.update(agent_id.as_str().as_bytes());
        PublicKey::new(hasher.finalize().to_vec())
    }

    fn shared_key(a: &PublicKey, b: &PublicKey) -> Vec<u8> {
        let (left, right) = if a.as_bytes() <= b.as_bytes() {
            (a.as_bytes(), b.as_bytes())
        } else {
            (b.as_bytes(), a.as_bytes())
        };
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().to_vec()
    }

    fn xor(data: &[u8], key: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(idx, byte)| byte ^ key[idx % key.len()])
            .collect()
    }
}

impl SigningService for InMemorySigner {
    fn pubkey(&self, agent_id: &AgentId) -> Result<PublicKey> {
        Ok(Self::derive_pubkey(agent_id))
    }

    fn sign(&self, agent_id: &AgentId, data: &[u8]) -> Result<Signature> {
        let pubkey = Self::derive_pubkey(agent_id);
        let mut hasher = Sha256::new();
        hasher.update(pubkey.as_bytes());
        hasher.update(data);
        Ok(Signature::new(hasher.finalize().to_vec()))
    }

    fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(pubkey.as_bytes());
        hasher.update(data);
        sig.as_bytes() == hasher.finalize().as_slice()
    }

    fn encrypt(&self, agent_id: &AgentId, recipient: &PublicKey, plaintext: &[u8]) -> Result<Vec<u8>> {
        let sender_pubkey = Self::derive_pubkey(agent_id);
        let key = Self::shared_key(&sender_pubkey, recipient);
        Ok(Self::xor(plaintext, &key))
    }

    fn decrypt(&self, agent_id: &AgentId, sender: &PublicKey, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let receiver_pubkey = Self::derive_pubkey(agent_id);
        let key = Self::shared_key(&receiver_pubkey, sender);
        Ok(Self::xor(ciphertext, &key))
    }
}
