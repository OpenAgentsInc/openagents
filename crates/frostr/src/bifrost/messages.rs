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

    /// Ping request to check peer connectivity
    #[serde(rename = "/ping")]
    Ping(Ping),

    /// Pong response to ping
    #[serde(rename = "/pong")]
    Pong(Pong),
}

/// Request to sign an event hash
///
/// # Examples
///
/// ```
/// use frostr::bifrost::SignRequest;
///
/// let request = SignRequest {
///     event_hash: [1u8; 32],
///     nonce_commitment: [2u8; 66],  // 66 bytes: hiding (33) + binding (33)
///     session_id: "session_123".to_string(),
///     participants: vec![1, 2],
///     initiator_id: 1,
/// };
///
/// assert_eq!(request.session_id, "session_123");
/// assert_eq!(request.participants.len(), 2);
/// ```
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignRequest {
    /// Event hash to sign (32 bytes)
    pub event_hash: [u8; 32],

    /// Nonce commitment: hiding (33 bytes) + binding (33 bytes) = 66 bytes
    /// This is the initiator's FROST commitment serialized as per serialization.rs
    #[serde_as(as = "[_; 66]")]
    pub nonce_commitment: [u8; 66],

    /// Session ID for correlating messages
    pub session_id: String,

    /// Participant identifiers expected to sign
    pub participants: Vec<u8>,

    /// Initiator's participant ID (so responders know who started the round)
    pub initiator_id: u8,
}

/// Partial signature response from a participant
///
/// In the Bifrost protocol, responders generate their own nonces/commitments
/// AND their partial signature in a single round. This combines FROST rounds 1 and 2
/// for the responder side.
///
/// # Examples
///
/// ```
/// use frostr::bifrost::SignResponse;
///
/// let response = SignResponse {
///     session_id: "session_123".to_string(),
///     participant_id: 1,
///     partial_sig: [3u8; 32],
///     nonce_commitment: [4u8; 66],  // 66 bytes: hiding (33) + binding (33)
/// };
///
/// assert_eq!(response.participant_id, 1);
/// assert_eq!(response.session_id, "session_123");
/// ```
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignResponse {
    /// Session ID from the request
    pub session_id: String,

    /// Participant ID (FROST identifier as u8)
    pub participant_id: u8,

    /// Partial signature (32 bytes scalar)
    pub partial_sig: [u8; 32],

    /// This participant's nonce commitment: hiding (33) + binding (33) = 66 bytes
    /// Needed by the coordinator to build the SigningPackage
    #[serde_as(as = "[_; 66]")]
    pub nonce_commitment: [u8; 66],
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
///
/// # Examples
///
/// ```
/// use frostr::bifrost::EcdhRequest;
///
/// let request = EcdhRequest {
///     target_pubkey: [5u8; 32],
///     session_id: "ecdh_session_456".to_string(),
///     participants: vec![1, 2, 3],
/// };
///
/// assert_eq!(request.participants.len(), 3);
/// assert_eq!(request.session_id, "ecdh_session_456");
/// ```
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
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EcdhResponse {
    /// Session ID from the request
    pub session_id: String,

    /// Participant ID
    pub participant_id: u8,

    /// Partial ECDH point (33 bytes compressed point)
    #[serde_as(as = "[_; 33]")]
    pub partial_ecdh: [u8; 33],
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

/// Ping request to check peer connectivity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ping {
    /// Session ID for correlating ping/pong
    pub session_id: String,

    /// Timestamp of ping (milliseconds since epoch)
    pub timestamp: u64,
}

/// Pong response to ping
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pong {
    /// Session ID from the ping request
    pub session_id: String,

    /// Timestamp of original ping
    pub ping_timestamp: u64,

    /// Timestamp of pong (milliseconds since epoch)
    pub pong_timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_request_serialize() {
        let msg = BifrostMessage::SignRequest(SignRequest {
            event_hash: [0x42; 32],
            nonce_commitment: [0x99; 66],  // 66 bytes: hiding + binding
            session_id: "test-session-1".to_string(),
            participants: vec![1, 2, 3],
            initiator_id: 1,
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
            nonce_commitment: [0xCD; 66],  // 66 bytes: hiding + binding
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
            partial_ecdh: [0x34; 33],
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
                nonce_commitment: [2; 66],  // 66 bytes
                session_id: "s1".into(),
                participants: vec![1, 2],
                initiator_id: 1,
            }),
            BifrostMessage::SignResponse(SignResponse {
                session_id: "s1".into(),
                participant_id: 1,
                partial_sig: [3; 32],
                nonce_commitment: [4; 66],  // 66 bytes
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
                partial_ecdh: [7; 33],
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
            BifrostMessage::Ping(Ping {
                session_id: "p1".into(),
                timestamp: 9,
            }),
            BifrostMessage::Pong(Pong {
                session_id: "p1".into(),
                ping_timestamp: 9,
                pong_timestamp: 10,
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

    #[test]
    fn test_ping_serialize() {
        let msg = BifrostMessage::Ping(Ping {
            session_id: "ping-session-1".to_string(),
            timestamp: 1234567890,
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/ping""#));
        assert!(json.contains("ping-session-1"));
        assert!(json.contains("1234567890"));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_pong_serialize() {
        let msg = BifrostMessage::Pong(Pong {
            session_id: "ping-session-1".to_string(),
            ping_timestamp: 1234567890,
            pong_timestamp: 1234567900,
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/pong""#));
        assert!(json.contains("ping-session-1"));
        assert!(json.contains("1234567890"));
        assert!(json.contains("1234567900"));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_ping_pong_round_trip() {
        let ping = BifrostMessage::Ping(Ping {
            session_id: "test-ping".to_string(),
            timestamp: 9876543210,
        });

        let pong = BifrostMessage::Pong(Pong {
            session_id: "test-ping".to_string(),
            ping_timestamp: 9876543210,
            pong_timestamp: 9876543220,
        });

        // Test ping serialization
        let ping_json = serde_json::to_string(&ping).unwrap();
        let ping_deserialized: BifrostMessage = serde_json::from_str(&ping_json).unwrap();
        assert_eq!(ping, ping_deserialized);

        // Test pong serialization
        let pong_json = serde_json::to_string(&pong).unwrap();
        let pong_deserialized: BifrostMessage = serde_json::from_str(&pong_json).unwrap();
        assert_eq!(pong, pong_deserialized);
    }
}
