//! Bifrost protocol message types for threshold operations
//!
//! Bifrost is the coordination protocol for threshold signing and ECDH
//! operations among FROST participants. Messages are exchanged via Nostr
//! encrypted NIP-44 channels.
//!
//! # FROST Two-Phase Signing Protocol (RFC 9591)
//!
//! All threshold signing uses the two-round FROST protocol:
//!
//! **Round 1 - Commitment Collection:**
//! 1. Coordinator sends `CommitmentRequest` to all k participants
//! 2. Each participant generates nonces, stores them, responds with `CommitmentResponse`
//! 3. Coordinator collects all k commitments
//!
//! **Round 2 - Signature Generation:**
//! 1. Coordinator sends `SigningPackage` with ALL k commitments to participants
//! 2. Each participant computes their partial signature using the complete package
//! 3. Participants respond with `PartialSignature`
//! 4. Coordinator aggregates into final signature

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// Bifrost protocol message wrapper
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum BifrostMessage {
    // === FROST Two-Phase Signing Protocol ===
    /// Round 1: Request commitments from participants (coordinator → peers)
    #[serde(rename = "/sign/commit/req")]
    CommitmentRequest(CommitmentRequest),

    /// Round 1: Commitment response from participant (peer → coordinator)
    #[serde(rename = "/sign/commit/res")]
    CommitmentResponse(CommitmentResponse),

    /// Round 2: Full signing package with all commitments (coordinator → peers)
    #[serde(rename = "/sign/package")]
    SigningPackage(SigningPackageMessage),

    /// Round 2: Partial signature response (peer → coordinator)
    #[serde(rename = "/sign/partial")]
    PartialSignature(PartialSignature),

    /// Final aggregated signature result
    #[serde(rename = "/sign/result")]
    SignResult(SignResult),

    /// Signing error
    #[serde(rename = "/sign/error")]
    SignError(SignError),

    // === ECDH Protocol ===
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

    // === Utility ===
    /// Ping request to check peer connectivity
    #[serde(rename = "/ping")]
    Ping(Ping),

    /// Pong response to ping
    #[serde(rename = "/pong")]
    Pong(Pong),
}

// ============================================================================
// FROST Two-Phase Signing Messages (RFC 9591 Compliant)
// ============================================================================

/// Round 1: Coordinator requests commitments from all k participants
///
/// In the FROST two-round protocol, the coordinator first collects commitments
/// from all signing participants before anyone generates partial signatures.
/// Each signer needs ALL k commitments to compute their partial signature correctly.
///
/// # Protocol Flow
/// 1. Coordinator generates their own commitment
/// 2. Coordinator sends CommitmentRequest to (k-1) other participants
/// 3. Each participant responds with CommitmentResponse
/// 4. Coordinator collects all k commitments
/// 5. Coordinator sends SigningPackageMessage to all participants
///
/// # Examples
///
/// ```
/// use frostr::bifrost::CommitmentRequest;
///
/// let request = CommitmentRequest {
///     event_hash: [1u8; 32],
///     session_id: "session_123".to_string(),
///     participants: vec![1, 2, 3],
///     initiator_id: 1,
/// };
///
/// assert_eq!(request.session_id, "session_123");
/// assert_eq!(request.participants.len(), 3);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitmentRequest {
    /// Event hash to sign (32 bytes) - participants verify they're signing the right thing
    pub event_hash: [u8; 32],

    /// Session ID for correlating messages
    pub session_id: String,

    /// Participant identifiers expected to participate in signing
    pub participants: Vec<u8>,

    /// Initiator's participant ID
    pub initiator_id: u8,
}

/// Round 1: Participant sends their commitment back to coordinator
///
/// The commitment is a pair of points (hiding, binding) that will be used
/// to compute binding factors in Round 2. Participants must store their
/// secret nonces locally until Round 2.
///
/// # Examples
///
/// ```
/// use frostr::bifrost::CommitmentResponse;
///
/// let response = CommitmentResponse {
///     session_id: "session_123".to_string(),
///     participant_id: 2,
///     nonce_commitment: [0x02; 66],
/// };
///
/// assert_eq!(response.participant_id, 2);
/// ```
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitmentResponse {
    /// Session ID from the request
    pub session_id: String,

    /// Participant ID (FROST identifier as u8)
    pub participant_id: u8,

    /// This participant's nonce commitment: hiding (33) + binding (33) = 66 bytes
    #[serde_as(as = "[_; 66]")]
    pub nonce_commitment: [u8; 66],
}

/// Round 2: Coordinator sends full signing package with ALL commitments
///
/// After collecting all k commitments in Round 1, the coordinator broadcasts
/// the complete set to all participants. Each participant can now compute
/// their binding factor and partial signature.
///
/// # Examples
///
/// ```
/// use frostr::bifrost::{SigningPackageMessage, ParticipantCommitment};
///
/// let package = SigningPackageMessage {
///     event_hash: [0x42; 32],
///     session_id: "session_123".to_string(),
///     commitments: vec![
///         ParticipantCommitment { participant_id: 1, commitment: [0x02; 66] },
///         ParticipantCommitment { participant_id: 2, commitment: [0x02; 66] },
///     ],
///     participants: vec![1, 2],
/// };
///
/// assert_eq!(package.commitments.len(), 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SigningPackageMessage {
    /// Event hash to sign (32 bytes)
    pub event_hash: [u8; 32],

    /// Session ID for correlating messages
    pub session_id: String,

    /// All k commitments from all participants (participant_id, commitment)
    /// Participants use this to build the SigningPackage
    pub commitments: Vec<ParticipantCommitment>,

    /// Participant identifiers who are signing
    pub participants: Vec<u8>,
}

/// A single participant's commitment for the SigningPackage
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParticipantCommitment {
    /// Participant ID
    pub participant_id: u8,

    /// Their nonce commitment: hiding (33) + binding (33) = 66 bytes
    #[serde_as(as = "[_; 66]")]
    pub commitment: [u8; 66],
}

/// Round 2: Participant sends their partial signature
///
/// After receiving the complete SigningPackage, the participant computes
/// their partial signature using their stored nonces and all k commitments.
///
/// # Examples
///
/// ```
/// use frostr::bifrost::PartialSignature;
///
/// let partial = PartialSignature {
///     session_id: "session_123".to_string(),
///     participant_id: 2,
///     partial_sig: [0xAB; 32],
/// };
///
/// assert_eq!(partial.participant_id, 2);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartialSignature {
    /// Session ID from the request
    pub session_id: String,

    /// Participant ID
    pub participant_id: u8,

    /// Partial signature (32 bytes scalar)
    pub partial_sig: [u8; 32],
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

// ============================================================================
// ECDH Messages
// ============================================================================

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

// ============================================================================
// Utility Messages
// ============================================================================

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
    fn test_commitment_request_serialize() {
        let msg = BifrostMessage::CommitmentRequest(CommitmentRequest {
            event_hash: [0x42; 32],
            session_id: "test-session-1".to_string(),
            participants: vec![1, 2, 3],
            initiator_id: 1,
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/sign/commit/req""#));
        assert!(json.contains("test-session-1"));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_commitment_response_serialize() {
        let msg = BifrostMessage::CommitmentResponse(CommitmentResponse {
            session_id: "test-session-1".to_string(),
            participant_id: 2,
            nonce_commitment: [0xCD; 66],
        });

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_signing_package_serialize() {
        let msg = BifrostMessage::SigningPackage(SigningPackageMessage {
            event_hash: [0x42; 32],
            session_id: "test-session-1".to_string(),
            commitments: vec![
                ParticipantCommitment {
                    participant_id: 1,
                    commitment: [0x02; 66],
                },
                ParticipantCommitment {
                    participant_id: 2,
                    commitment: [0x02; 66],
                },
            ],
            participants: vec![1, 2],
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/sign/package""#));

        let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_partial_signature_serialize() {
        let msg = BifrostMessage::PartialSignature(PartialSignature {
            session_id: "test-session-1".to_string(),
            participant_id: 2,
            partial_sig: [0xAB; 32],
        });

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"/sign/partial""#));

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
            BifrostMessage::CommitmentRequest(CommitmentRequest {
                event_hash: [1; 32],
                session_id: "s1".into(),
                participants: vec![1, 2],
                initiator_id: 1,
            }),
            BifrostMessage::CommitmentResponse(CommitmentResponse {
                session_id: "s1".into(),
                participant_id: 1,
                nonce_commitment: [2; 66],
            }),
            BifrostMessage::SigningPackage(SigningPackageMessage {
                event_hash: [1; 32],
                session_id: "s1".into(),
                commitments: vec![ParticipantCommitment {
                    participant_id: 1,
                    commitment: [2; 66],
                }],
                participants: vec![1, 2],
            }),
            BifrostMessage::PartialSignature(PartialSignature {
                session_id: "s1".into(),
                participant_id: 1,
                partial_sig: [3; 32],
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

        let ping_json = serde_json::to_string(&ping).unwrap();
        let ping_deserialized: BifrostMessage = serde_json::from_str(&ping_json).unwrap();
        assert_eq!(ping, ping_deserialized);

        let pong_json = serde_json::to_string(&pong).unwrap();
        let pong_deserialized: BifrostMessage = serde_json::from_str(&pong_json).unwrap();
        assert_eq!(pong, pong_deserialized);
    }
}
