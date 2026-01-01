//! Identity and signing abstractions.

use crate::error::Result;
use crate::types::AgentId;
#[cfg(not(target_arch = "wasm32"))]
use base64::Engine;
#[cfg(not(target_arch = "wasm32"))]
use base64::engine::general_purpose::STANDARD;
#[cfg(not(target_arch = "wasm32"))]
use bitcoin::secp256k1::{Keypair, Message, Secp256k1, SecretKey, XOnlyPublicKey, schnorr};
#[cfg(not(target_arch = "wasm32"))]
use compute::domain::UnifiedIdentity;
#[cfg(not(target_arch = "wasm32"))]
use nostr::{decrypt as decrypt_v1, decrypt_v2, encrypt_v2, get_public_key};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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
    fn encrypt(
        &self,
        agent_id: &AgentId,
        recipient: &PublicKey,
        plaintext: &[u8],
    ) -> Result<Vec<u8>>;

    /// Decrypt from sender.
    fn decrypt(&self, agent_id: &AgentId, sender: &PublicKey, ciphertext: &[u8])
    -> Result<Vec<u8>>;
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

    fn encrypt(
        &self,
        agent_id: &AgentId,
        recipient: &PublicKey,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let sender_pubkey = Self::derive_pubkey(agent_id);
        let key = Self::shared_key(&sender_pubkey, recipient);
        Ok(Self::xor(plaintext, &key))
    }

    fn decrypt(
        &self,
        agent_id: &AgentId,
        sender: &PublicKey,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>> {
        let receiver_pubkey = Self::derive_pubkey(agent_id);
        let key = Self::shared_key(&receiver_pubkey, sender);
        Ok(Self::xor(ciphertext, &key))
    }
}

/// Real signing service backed by Nostr-compatible keys (in-memory for local dev).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct NostrSigner {
    keys: Arc<RwLock<HashMap<AgentId, [u8; 32]>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl NostrSigner {
    /// Create a new signer with empty key cache.
    pub fn new() -> Self {
        Self::default()
    }

    fn secret_for(&self, agent_id: &AgentId) -> Result<[u8; 32]> {
        let mut guard = self.keys.write().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(agent_id.clone())
            .or_insert_with(nostr::generate_secret_key);
        Ok(*entry)
    }

    fn digest(data: &[u8]) -> [u8; 32] {
        if data.len() == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(data);
            return out;
        }
        let hash = Sha256::digest(data);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash);
        out
    }

    fn compressed_pubkey(pubkey: &PublicKey) -> Result<[u8; 33]> {
        let bytes = pubkey.as_bytes();
        if bytes.len() != 32 {
            return Err("nostr pubkey must be 32 bytes".into());
        }
        let mut out = [0u8; 33];
        out[0] = 0x02;
        out[1..].copy_from_slice(bytes);
        Ok(out)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SigningService for NostrSigner {
    fn pubkey(&self, agent_id: &AgentId) -> Result<PublicKey> {
        let secret = self.secret_for(agent_id)?;
        let pubkey = get_public_key(&secret).map_err(|err| err.to_string())?;
        Ok(PublicKey::new(pubkey.to_vec()))
    }

    fn sign(&self, agent_id: &AgentId, data: &[u8]) -> Result<Signature> {
        let digest = Self::digest(data);
        let message = Message::from_digest_slice(&digest).map_err(|err| err.to_string())?;
        let secret = self.secret_for(agent_id)?;
        let sk = SecretKey::from_slice(&secret).map_err(|err| err.to_string())?;
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(&secp, &sk);
        let sig = secp.sign_schnorr_no_aux_rand(&message, &keypair);
        Ok(Signature::new(sig.serialize().to_vec()))
    }

    fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool {
        let digest = Self::digest(data);
        let message = match Message::from_digest_slice(&digest) {
            Ok(message) => message,
            Err(_) => return false,
        };
        let xonly = match XOnlyPublicKey::from_slice(pubkey.as_bytes()) {
            Ok(key) => key,
            Err(_) => return false,
        };
        let signature = match schnorr::Signature::from_slice(sig.as_bytes()) {
            Ok(signature) => signature,
            Err(_) => return false,
        };
        let secp = Secp256k1::verification_only();
        secp.verify_schnorr(&signature, &message, &xonly).is_ok()
    }

    fn encrypt(
        &self,
        agent_id: &AgentId,
        recipient: &PublicKey,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let secret = self.secret_for(agent_id)?;
        let recipient = Self::compressed_pubkey(recipient)?;
        let encoded = STANDARD.encode(plaintext);
        let encrypted = encrypt_v2(&secret, &recipient, &encoded).map_err(|err| err.to_string())?;
        Ok(encrypted.into_bytes())
    }

    fn decrypt(
        &self,
        agent_id: &AgentId,
        sender: &PublicKey,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>> {
        let secret = self.secret_for(agent_id)?;
        let sender = Self::compressed_pubkey(sender)?;
        let cipher_str = std::str::from_utf8(ciphertext).map_err(|err| err.to_string())?;
        let decrypted = decrypt_v2(&secret, &sender, cipher_str)
            .or_else(|_| decrypt_v1(&secret, &sender, cipher_str))
            .map_err(|err| err.to_string())?;
        let decoded = STANDARD.decode(decrypted.as_bytes());
        Ok(decoded.unwrap_or_else(|_| decrypted.into_bytes()))
    }
}

/// Signing service backed by a UnifiedIdentity keypair.
#[cfg(not(target_arch = "wasm32"))]
pub struct UnifiedIdentitySigner {
    identity: Arc<UnifiedIdentity>,
}

#[cfg(not(target_arch = "wasm32"))]
impl UnifiedIdentitySigner {
    /// Create a signer from a UnifiedIdentity.
    pub fn new(identity: Arc<UnifiedIdentity>) -> Self {
        Self { identity }
    }

    fn digest(data: &[u8]) -> [u8; 32] {
        if data.len() == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(data);
            return out;
        }
        let hash = Sha256::digest(data);
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash);
        out
    }

    fn compressed_pubkey(pubkey: &PublicKey) -> Result<[u8; 33]> {
        let bytes = pubkey.as_bytes();
        if bytes.len() != 32 {
            return Err("nostr pubkey must be 32 bytes".into());
        }
        let mut out = [0u8; 33];
        out[0] = 0x02;
        out[1..].copy_from_slice(bytes);
        Ok(out)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SigningService for UnifiedIdentitySigner {
    fn pubkey(&self, _agent_id: &AgentId) -> Result<PublicKey> {
        Ok(PublicKey::new(self.identity.public_key_bytes().to_vec()))
    }

    fn sign(&self, _agent_id: &AgentId, data: &[u8]) -> Result<Signature> {
        let digest = Self::digest(data);
        let message = Message::from_digest_slice(&digest).map_err(|err| err.to_string())?;
        let sk = SecretKey::from_slice(self.identity.private_key_bytes())
            .map_err(|err| err.to_string())?;
        let secp = Secp256k1::new();
        let keypair = Keypair::from_secret_key(&secp, &sk);
        let sig = secp.sign_schnorr_no_aux_rand(&message, &keypair);
        Ok(Signature::new(sig.serialize().to_vec()))
    }

    fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool {
        let digest = Self::digest(data);
        let message = match Message::from_digest_slice(&digest) {
            Ok(message) => message,
            Err(_) => return false,
        };
        let xonly = match XOnlyPublicKey::from_slice(pubkey.as_bytes()) {
            Ok(key) => key,
            Err(_) => return false,
        };
        let signature = match schnorr::Signature::from_slice(sig.as_bytes()) {
            Ok(signature) => signature,
            Err(_) => return false,
        };
        let secp = Secp256k1::verification_only();
        secp.verify_schnorr(&signature, &message, &xonly).is_ok()
    }

    fn encrypt(
        &self,
        _agent_id: &AgentId,
        recipient: &PublicKey,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let recipient = Self::compressed_pubkey(recipient)?;
        let encoded = STANDARD.encode(plaintext);
        let encrypted = encrypt_v2(self.identity.private_key_bytes(), &recipient, &encoded)
            .map_err(|err| err.to_string())?;
        Ok(encrypted.into_bytes())
    }

    fn decrypt(
        &self,
        _agent_id: &AgentId,
        sender: &PublicKey,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>> {
        let sender = Self::compressed_pubkey(sender)?;
        let cipher_str = std::str::from_utf8(ciphertext).map_err(|err| err.to_string())?;
        let decrypted = decrypt_v2(self.identity.private_key_bytes(), &sender, cipher_str)
            .or_else(|_| decrypt_v1(self.identity.private_key_bytes(), &sender, cipher_str))
            .map_err(|err| err.to_string())?;
        let decoded = STANDARD.decode(decrypted.as_bytes());
        Ok(decoded.unwrap_or_else(|_| decrypted.into_bytes()))
    }
}
