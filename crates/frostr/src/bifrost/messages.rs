//! Bifrost protocol message types for threshold operations
//!
//! Bifrost is the coordination protocol for threshold signing and ECDH
//! operations among FROST participants. Messages are exchanged via Nostr
//! encrypted NIP-04 or NIP-44 channels.

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Bifrost protocol message wrapper
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum BifrostMessage {
    /// Request to initiate threshold signing
    #[serde(rename = "/sign/req")]
    SignRequest(SignRequest),

    /// Response with partial signature
    #[serde(rename = "/sign/res")]
    SignResponse(SignResponse),

    /// Final aggregated signature result
    #[serde(rename = "/sign/result")]
    SignResult(SignResult),

    /// Signing error
    #[serde(rename = "/sign/error")]
    SignError(SignError),

    /// Request threshold ECDH computation
    #[serde(rename = "/ecdh/req")]
    EcdhRequest(EcdhRequest),

    /// Response with partial ECDH result
    #[serde(rename = "/ecdh/res")]
    EcdhResponse(EcdhResponse),

    /// Final aggregated ECDH shared secret
    #[serde(rename = "/ecdh/result")]
    EcdhResult(EcdhResult),

    /// ECDH computation error
    #[serde(rename = "/ecdh/error")]
    EcdhError(EcdhError),
}

/// Request to sign an event hash
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignRequest {
    /// Event hash to sign (32 bytes)
    pub event_hash: [u8; 32],

    /// Nonce commitment (33 bytes compressed point)
    #[serde_as(as = "[_; 33]")]
    pub nonce_commitment: [u8; 33],

    /// Session ID for correlating messages
    pub session_id: String,

    /// Participant identifiers expected to sign
    pub participants: Vec<u8>,
}

/// Partial signature response from a participant
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignResponse {
    /// Session ID from the request
    pub session_id: String,

    /// Participant ID
    pub participant_id: u8,

    /// Partial signature (32 bytes)
    pub partial_sig: [u8; 32],

    /// Nonce share (33 bytes compressed point)
    #[serde_as(as = "[_; 33]")]
    pub nonce_share: [u8; 33],
}

/// Final aggregated signature
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignResult {
    /// Session ID
    pub session_id: String,

    /// Complete Schnorr signature (64 bytes: R || s)
    #[serde_as(as = "[_; 64]")]
    pub signature: [u8; 64],
}

/// Signing error message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignError {
    /// Session ID
    pub session_id: String,

    /// Error description
    pub reason: String,

    /// Optional error code
    pub code: Option<String>,
}

/// Request for threshold ECDH computation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EcdhRequest {
    /// Target peer public key (32 bytes x-only)
    pub target_pubkey: [u8; 32],

    /// Session ID for correlating messages
    pub session_id: String,

    /// Participant identifiers expected to participate
    pub participants: Vec<u8>,
}

/// Partial ECDH result from a participant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EcdhResponse {
    /// Session ID from the request
    pub session_id: String,

    /// Participant ID
    pub participant_id: u8,

    /// Partial ECDH computation (32 bytes)
    pub partial_ecdh: [u8; 32],
}

/// Final aggregated ECDH shared secret
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EcdhResult {
    /// Session ID
    pub session_id: String,

    /// Shared secret for NIP-44 encryption (32 bytes)
    pub shared_secret: [u8; 32],
}

/// ECDH computation error message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EcdhError {
    /// Session ID
    pub session_id: String,

    /// Error description
    pub reason: String,

    /// Optional error code
    pub code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_request_serialize() {
        let msg = BifrostMessage::SignRequest(SignRequest {
            event_hash: [0x42; 32],
            nonce_commitment: [0x99; 33],
            session_id: "test-session-1".to_string(),
            participants: vec![1, 2, 3],
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/sign/req""#));
        assert!(json.contains("test-session-1"));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_sign_response_serialize() {
        let msg = BifrostMessage::SignResponse(SignResponse {
            session_id: "test-session-1".to_string(),
            participant_id: 2,
            partial_sig: [0xAB; 32],
            nonce_share: [0xCD; 33],
        });

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_sign_result_serialize() {
        let msg = BifrostMessage::SignResult(SignResult {
            session_id: "test-session-1".to_string(),
            signature: [0xFF; 64],
        });

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_sign_error_serialize() {
        let msg = BifrostMessage::SignError(SignError {
            session_id: "test-session-1".to_string(),
            reason: "Insufficient participants".to_string(),
            code: Some("ERR_THRESHOLD".to_string()),
        });

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_ecdh_request_serialize() {
        let msg = BifrostMessage::EcdhRequest(EcdhRequest {
            target_pubkey: [0x12; 32],
            session_id: "ecdh-session-1".to_string(),
            participants: vec![1, 2],
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/ecdh/req""#));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_ecdh_response_serialize() {
        let msg = BifrostMessage::EcdhResponse(EcdhResponse {
            session_id: "ecdh-session-1".to_string(),
            participant_id: 1,
            partial_ecdh: [0x34; 32],
        });

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_invalid_message_type() {
        let json = r#"{"type":"/invalid/type","data":{}}"#;
        let result: Result<BifrostMessage, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_round_trip_all_messages() {
        let messages = vec![
            BifrostMessage::SignRequest(SignRequest {
                event_hash: [1; 32],
                nonce_commitment: [2; 33],
                session_id: "s1".into(),
                participants: vec![1, 2],
            }),
            BifrostMessage::SignResponse(SignResponse {
                session_id: "s1".into(),
                participant_id: 1,
                partial_sig: [3; 32],
                nonce_share: [4; 33],
            }),
            BifrostMessage::SignResult(SignResult {
                session_id: "s1".into(),
                signature: [5; 64],
            }),
            BifrostMessage::SignError(SignError {
                session_id: "s1".into(),
                reason: "test error".into(),
                code: None,
            }),
            BifrostMessage::EcdhRequest(EcdhRequest {
                target_pubkey: [6; 32],
                session_id: "e1".into(),
                participants: vec![1, 2],
            }),
            BifrostMessage::EcdhResponse(EcdhResponse {
                session_id: "e1".into(),
                participant_id: 1,
                partial_ecdh: [7; 32],
            }),
            BifrostMessage::EcdhResult(EcdhResult {
                session_id: "e1".into(),
                shared_secret: [8; 32],
            }),
            BifrostMessage::EcdhError(EcdhError {
                session_id: "e1".into(),
                reason: "ECDH computation failed".into(),
                code: Some("ERR_ECDH".into()),
            }),
        ];

        for msg in messages {
            let json = serde_json::to_string(&msg).unwrap();
            let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, deserialized);
        }
    }

    #[test]
    fn test_ecdh_result_serialize() {
        let msg = BifrostMessage::EcdhResult(EcdhResult {
            session_id: "ecdh-session-2".to_string(),
            shared_secret: [0xAB; 32],
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/ecdh/result""#));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_ecdh_error_serialize() {
        let msg = BifrostMessage::EcdhError(EcdhError {
            session_id: "ecdh-session-3".to_string(),
            reason: "Insufficient threshold participants".to_string(),
            code: Some("ERR_THRESHOLD".to_string()),
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/ecdh/error""#));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }
}
