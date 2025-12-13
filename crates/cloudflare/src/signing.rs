//! Service identity and event signing for the DVM.
//!
//! Uses k256 for WASM-compatible Schnorr signatures (BIP-340).

use k256::schnorr::SigningKey;
use k256::schnorr::signature::Signer;
use k256::sha2::{Digest, Sha256};

/// Service identity for signing DVM events.
pub struct ServiceIdentity {
    signing_key: SigningKey,
    pubkey_hex: String,
}

impl ServiceIdentity {
    /// Create from hex-encoded private key (32 bytes = 64 hex chars).
    pub fn from_hex(privkey_hex: &str) -> Result<Self, String> {
        let bytes = hex::decode(privkey_hex).map_err(|e| format!("invalid hex: {}", e))?;

        let bytes_array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "private key must be 32 bytes")?;

        let signing_key =
            SigningKey::from_bytes(&bytes_array).map_err(|e| format!("invalid key: {}", e))?;

        let verifying_key = signing_key.verifying_key();
        let pubkey_hex = hex::encode(verifying_key.to_bytes());

        Ok(Self {
            signing_key,
            pubkey_hex,
        })
    }

    /// Get the public key as hex string (32 bytes / 64 hex chars).
    pub fn pubkey(&self) -> &str {
        &self.pubkey_hex
    }

    /// Sign an event and return the signature as hex (64 bytes / 128 hex chars).
    ///
    /// The event_id should be the SHA256 hash of the event serialization.
    pub fn sign_event(&self, event_id: &str) -> Result<String, String> {
        let id_bytes = hex::decode(event_id).map_err(|e| format!("invalid event id hex: {}", e))?;

        if id_bytes.len() != 32 {
            return Err("event id must be 32 bytes".to_string());
        }

        let signature = self.signing_key.sign(&id_bytes);
        Ok(hex::encode(signature.to_bytes()))
    }

    /// Compute event ID (SHA256 hash of serialized event).
    ///
    /// Per NIP-01, the serialization is: [0, pubkey, created_at, kind, tags, content]
    pub fn compute_event_id(
        pubkey: &str,
        created_at: u64,
        kind: u16,
        tags: &[Vec<String>],
        content: &str,
    ) -> Result<String, String> {
        let serialized = serde_json::to_string(&(0, pubkey, created_at, kind, tags, content))
            .map_err(|e| format!("serialization error: {}", e))?;

        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let hash = hasher.finalize();

        Ok(hex::encode(hash))
    }

    /// Create and sign a complete event.
    ///
    /// Returns (id, sig) tuple.
    pub fn finalize_event(
        &self,
        created_at: u64,
        kind: u16,
        tags: &[Vec<String>],
        content: &str,
    ) -> Result<(String, String), String> {
        let id = Self::compute_event_id(&self.pubkey_hex, created_at, kind, tags, content)?;
        let sig = self.sign_event(&id)?;
        Ok((id, sig))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_identity_from_hex() {
        // A valid 32-byte private key (64 hex chars)
        let privkey = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let identity = ServiceIdentity::from_hex(privkey).unwrap();

        // Public key should be 64 hex chars (32 bytes)
        assert_eq!(identity.pubkey().len(), 64);
    }

    #[test]
    fn test_compute_event_id() {
        let id = ServiceIdentity::compute_event_id(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            1234567890,
            1,
            &[],
            "Hello, world!",
        )
        .unwrap();

        // ID should be 64 hex chars (32 bytes SHA256)
        assert_eq!(id.len(), 64);
    }

    #[test]
    fn test_sign_and_finalize() {
        let privkey = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let identity = ServiceIdentity::from_hex(privkey).unwrap();

        let (id, sig) = identity
            .finalize_event(1234567890, 7000, &[], "test content")
            .unwrap();

        // ID should be 64 hex chars
        assert_eq!(id.len(), 64);
        // Signature should be 128 hex chars (64 bytes Schnorr)
        assert_eq!(sig.len(), 128);
    }
}
