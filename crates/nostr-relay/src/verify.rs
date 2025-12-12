//! Event verification for Nostr relay.
//!
//! Verifies that events have valid IDs and signatures according to NIP-01.

use nostr::Event;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur during verification.
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("invalid event id: computed {computed}, got {got}")]
    InvalidId { computed: String, got: String },

    #[error("invalid signature")]
    InvalidSignature,

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("invalid hex: {0}")]
    InvalidHex(String),
}

/// Verify an event's ID and signature.
///
/// Returns `Ok(())` if the event is valid, or an error describing the problem.
pub fn verify_event(event: &Event) -> Result<(), VerifyError> {
    // 1. Verify the event ID
    let computed_id = compute_event_id(event)?;
    if computed_id != event.id {
        return Err(VerifyError::InvalidId {
            computed: computed_id,
            got: event.id.clone(),
        });
    }

    // 2. Verify the signature
    // Note: Full signature verification requires secp256k1.
    // For WASM builds, we skip signature verification.
    #[cfg(feature = "native")]
    verify_signature(event)?;

    Ok(())
}

/// Compute the expected event ID (SHA256 of serialized event).
fn compute_event_id(event: &Event) -> Result<String, VerifyError> {
    // NIP-01 serialization: [0, pubkey, created_at, kind, tags, content]
    let serialized = serde_json::to_string(&(
        0,
        &event.pubkey,
        event.created_at,
        event.kind,
        &event.tags,
        &event.content,
    ))
    .map_err(|e| VerifyError::InvalidHex(e.to_string()))?;

    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let hash = hasher.finalize();

    Ok(hex::encode(hash))
}

/// Verify the event signature (Schnorr over secp256k1).
#[cfg(feature = "native")]
fn verify_signature(event: &Event) -> Result<(), VerifyError> {
    use bitcoin::key::Secp256k1;
    use bitcoin::secp256k1::{schnorr, Message, XOnlyPublicKey};

    let secp = Secp256k1::verification_only();

    // Parse public key
    let pubkey_bytes =
        hex::decode(&event.pubkey).map_err(|e| VerifyError::InvalidPublicKey(e.to_string()))?;
    let pubkey = XOnlyPublicKey::from_slice(&pubkey_bytes)
        .map_err(|e| VerifyError::InvalidPublicKey(e.to_string()))?;

    // Parse event ID as message
    let id_bytes =
        hex::decode(&event.id).map_err(|e| VerifyError::InvalidHex(e.to_string()))?;
    let message = Message::from_digest_slice(&id_bytes)
        .map_err(|_| VerifyError::InvalidHex("invalid message length".to_string()))?;

    // Parse signature
    let sig_bytes =
        hex::decode(&event.sig).map_err(|e| VerifyError::InvalidHex(e.to_string()))?;
    let signature = schnorr::Signature::from_slice(&sig_bytes)
        .map_err(|_| VerifyError::InvalidSignature)?;

    // Verify
    secp.verify_schnorr(&signature, &message, &pubkey)
        .map_err(|_| VerifyError::InvalidSignature)?;

    Ok(())
}

/// Verify just the event ID (for WASM where full signature verification may not be available).
pub fn verify_event_id(event: &Event) -> Result<(), VerifyError> {
    let computed_id = compute_event_id(event)?;
    if computed_id != event.id {
        return Err(VerifyError::InvalidId {
            computed: computed_id,
            got: event.id.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_event_id() {
        let event = Event {
            id: "unused".to_string(),
            pubkey: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                .to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Hello, world!".to_string(),
            sig: "unused".to_string(),
        };

        let id = compute_event_id(&event).unwrap();
        assert_eq!(id.len(), 64); // 32 bytes hex encoded
    }

    #[test]
    fn test_verify_event_id_mismatch() {
        let event = Event {
            id: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            pubkey: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                .to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "Hello, world!".to_string(),
            sig: "unused".to_string(),
        };

        let result = verify_event_id(&event);
        assert!(matches!(result, Err(VerifyError::InvalidId { .. })));
    }
}
