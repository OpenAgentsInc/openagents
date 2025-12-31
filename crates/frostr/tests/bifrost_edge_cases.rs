//! Edge case tests for Bifrost threshold signing protocol
//!
//! These tests verify boundary conditions:
//! - Threshold equals total participants (n-of-n)
//! - Minimum threshold (1-of-n - single signer)
//! - Maximum message sizes
//! - Empty and minimal messages
//! - Extreme threshold values
//! - Invalid threshold configurations

use frostr::bifrost::{
    BifrostConfig, BifrostMessage, BifrostNode, CommitmentRequest, CommitmentResponse,
    PartialSignature, ParticipantCommitment, SigningPackageMessage, TimeoutConfig,
};
use frostr::keygen::generate_key_shares;

// ============================================================================
// THRESHOLD BOUNDARY TESTS
// ============================================================================

#[test]
fn test_n_of_n_threshold_signing_setup() {
    // 3-of-3 threshold - ALL participants required
    let shares = generate_key_shares(3, 3).expect("failed to generate 3-of-3 shares");

    assert_eq!(shares.len(), 3);

    for share in &shares {
        assert_eq!(share.threshold, 3);
        assert_eq!(share.total, 3);
    }

    // All shares must have the same group public key
    let group_pk = shares[0].public_key_package.verifying_key();
    for share in &shares[1..] {
        assert_eq!(
            share.public_key_package.verifying_key(),
            group_pk,
            "All shares must have same group pubkey"
        );
    }
}

#[test]
fn test_2_of_2_threshold() {
    // 2-of-2 minimum multi-sig
    let shares = generate_key_shares(2, 2).expect("failed to generate 2-of-2 shares");

    assert_eq!(shares.len(), 2);

    for share in &shares {
        assert_eq!(share.threshold, 2);
        assert_eq!(share.total, 2);
    }
}

#[test]
fn test_1_of_1_threshold() {
    // 1-of-1 is effectively a regular key (no threshold)
    // This might or might not be supported
    let result = generate_key_shares(1, 1);

    // Either succeeds (degenerates to single key) or fails (invalid config)
    // Document the behavior
    if result.is_ok() {
        let shares = result.unwrap();
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].threshold, 1);
        assert_eq!(shares[0].total, 1);
    } else {
        // InvalidThreshold or InvalidMinSigners is acceptable for 1-of-1
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("threshold")
                || err_msg.contains("Threshold")
                || err_msg.contains("InvalidMinSigners")
                || err_msg.contains("Signers"),
            "Expected threshold/signers-related error, got: {}",
            err_msg
        );
    }
}

#[test]
fn test_large_threshold_10_of_15() {
    // Larger threshold configurations
    let shares = generate_key_shares(10, 15).expect("failed to generate 10-of-15 shares");

    assert_eq!(shares.len(), 15);

    for (i, share) in shares.iter().enumerate() {
        assert_eq!(share.threshold, 10);
        assert_eq!(share.total, 15);
        assert_eq!(share.participant_id, (i + 1) as u8);
    }
}

#[test]
fn test_threshold_equals_total_minus_one() {
    // n-1 of n - only one participant can be missing
    let shares = generate_key_shares(4, 5).expect("failed to generate 4-of-5 shares");

    assert_eq!(shares.len(), 5);

    for share in &shares {
        assert_eq!(share.threshold, 4);
        assert_eq!(share.total, 5);
    }
}

#[test]
fn test_minimum_threshold_2_of_n() {
    // 2-of-5 - very low threshold
    let shares = generate_key_shares(2, 5).expect("failed to generate 2-of-5 shares");

    assert_eq!(shares.len(), 5);

    for share in &shares {
        assert_eq!(share.threshold, 2);
        assert_eq!(share.total, 5);
    }
}

// ============================================================================
// INVALID THRESHOLD TESTS
// ============================================================================

#[test]
fn test_threshold_greater_than_total_rejected() {
    // Threshold > Total is invalid
    let result = generate_key_shares(5, 3);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("threshold") || err_msg.contains("Threshold") || err_msg.contains("5"),
        "Expected threshold error, got: {}",
        err_msg
    );
}

#[test]
fn test_zero_threshold_rejected() {
    // Threshold of 0 is invalid
    let result = generate_key_shares(0, 3);

    assert!(result.is_err());
}

#[test]
fn test_zero_total_rejected() {
    // Total of 0 is invalid
    let result = generate_key_shares(0, 0);

    assert!(result.is_err());
}

// ============================================================================
// MESSAGE SIZE BOUNDARY TESTS
// ============================================================================

#[test]
fn test_minimum_event_hash() {
    // Event hash with minimal non-zero value
    let mut event_hash = [0u8; 32];
    event_hash[31] = 1; // Smallest non-zero hash

    let request = CommitmentRequest {
        event_hash,
        session_id: "test".to_string(),
        participants: vec![1, 2],
        initiator_id: 1,
    };

    assert_eq!(request.event_hash[31], 1);
    assert_eq!(request.event_hash[0], 0);
}

#[test]
fn test_maximum_event_hash() {
    // Event hash with all bits set
    let event_hash = [0xFF; 32];

    let request = CommitmentRequest {
        event_hash,
        session_id: "test".to_string(),
        participants: vec![1, 2],
        initiator_id: 1,
    };

    assert!(request.event_hash.iter().all(|&b| b == 0xFF));
}

#[test]
fn test_minimum_session_id() {
    // Single character session ID
    let request = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "x".to_string(),
        participants: vec![1, 2],
        initiator_id: 1,
    };

    assert_eq!(request.session_id.len(), 1);

    // Serialization should work
    let json = serde_json::to_string(&BifrostMessage::CommitmentRequest(request)).unwrap();
    assert!(json.contains("\"session_id\":\"x\""));
}

#[test]
fn test_unicode_session_id() {
    // Session ID with unicode characters
    let request = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "测试会话🔐".to_string(),
        participants: vec![1, 2],
        initiator_id: 1,
    };

    // Serialization round-trip should preserve unicode
    let json = serde_json::to_string(&BifrostMessage::CommitmentRequest(request.clone())).unwrap();
    let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();

    if let BifrostMessage::CommitmentRequest(req) = deserialized {
        assert_eq!(req.session_id, "测试会话🔐");
    } else {
        panic!("Wrong message type");
    }
}

// ============================================================================
// PARTICIPANT LIST BOUNDARY TESTS
// ============================================================================

#[test]
fn test_minimum_participants_for_threshold() {
    // With threshold 2, minimum is 2 participants
    let request = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        participants: vec![1, 2],
        initiator_id: 1,
    };

    assert_eq!(request.participants.len(), 2);
}

#[test]
fn test_single_participant_list() {
    // Single participant - might be valid for 1-of-n or self-signing
    let request = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        participants: vec![1],
        initiator_id: 1,
    };

    assert_eq!(request.participants.len(), 1);
}

#[test]
fn test_large_participant_list() {
    // Many participants
    let participants: Vec<u8> = (1..=100).collect();

    let request = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        participants: participants.clone(),
        initiator_id: 1,
    };

    assert_eq!(request.participants.len(), 100);

    // Serialization should handle large lists
    let json = serde_json::to_string(&BifrostMessage::CommitmentRequest(request)).unwrap();
    let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();

    if let BifrostMessage::CommitmentRequest(req) = deserialized {
        assert_eq!(req.participants.len(), 100);
    }
}

#[test]
fn test_max_participant_id() {
    // u8 max value as participant ID
    let request = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        participants: vec![1, 255],
        initiator_id: 255,
    };

    assert_eq!(request.initiator_id, 255);
    assert!(request.participants.contains(&255));
}

// ============================================================================
// COMMITMENT BOUNDARY TESTS
// ============================================================================

#[test]
fn test_commitment_exactly_66_bytes() {
    // Commitment is hiding (33) + binding (33) = 66 bytes
    let response = CommitmentResponse {
        session_id: "test".to_string(),
        participant_id: 1,
        nonce_commitment: [0x02; 66],
    };

    assert_eq!(response.nonce_commitment.len(), 66);
}

#[test]
fn test_commitment_with_valid_point_prefix() {
    // 0x02 or 0x03 are valid compressed point prefixes
    let mut commitment_02 = [0u8; 66];
    commitment_02[0] = 0x02;
    commitment_02[33] = 0x02;

    let mut commitment_03 = [0u8; 66];
    commitment_03[0] = 0x03;
    commitment_03[33] = 0x03;

    let response_02 = CommitmentResponse {
        session_id: "test".to_string(),
        participant_id: 1,
        nonce_commitment: commitment_02,
    };

    let response_03 = CommitmentResponse {
        session_id: "test".to_string(),
        participant_id: 2,
        nonce_commitment: commitment_03,
    };

    // Both should serialize
    assert!(serde_json::to_string(&BifrostMessage::CommitmentResponse(response_02)).is_ok());
    assert!(serde_json::to_string(&BifrostMessage::CommitmentResponse(response_03)).is_ok());
}

// ============================================================================
// PARTIAL SIGNATURE BOUNDARY TESTS
// ============================================================================

#[test]
fn test_partial_signature_exactly_32_bytes() {
    // Partial signature is a 32-byte scalar
    let sig = PartialSignature {
        session_id: "test".to_string(),
        participant_id: 1,
        partial_sig: [0x42; 32],
    };

    assert_eq!(sig.partial_sig.len(), 32);
}

#[test]
fn test_partial_signature_minimum_value() {
    // Minimum valid scalar (1)
    let mut sig_bytes = [0u8; 32];
    sig_bytes[31] = 0x01;

    let sig = PartialSignature {
        session_id: "test".to_string(),
        participant_id: 1,
        partial_sig: sig_bytes,
    };

    assert_eq!(sig.partial_sig[31], 1);
    assert!(sig.partial_sig[..31].iter().all(|&b| b == 0));
}

#[test]
fn test_partial_signature_near_curve_order() {
    // secp256k1 order n = FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
    // Values near the order should be handled
    let sig = PartialSignature {
        session_id: "test".to_string(),
        participant_id: 1,
        partial_sig: [0xFE; 32], // Large but under order
    };

    // Should serialize without issue
    let json = serde_json::to_string(&BifrostMessage::PartialSignature(sig)).unwrap();
    assert!(!json.is_empty());
}

// ============================================================================
// SIGNING PACKAGE BOUNDARY TESTS
// ============================================================================

#[test]
fn test_signing_package_minimum_commitments() {
    // Minimum: threshold commitments (e.g., 2 for 2-of-3)
    let package = SigningPackageMessage {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        commitments: vec![
            ParticipantCommitment {
                participant_id: 1,
                commitment: [0x01; 66],
            },
            ParticipantCommitment {
                participant_id: 2,
                commitment: [0x02; 66],
            },
        ],
        participants: vec![1, 2],
    };

    assert_eq!(package.commitments.len(), 2);
    assert_eq!(package.participants.len(), 2);
}

#[test]
fn test_signing_package_all_commitments() {
    // Maximum: all n participants provide commitments
    let n = 5;
    let commitments: Vec<ParticipantCommitment> = (1..=n)
        .map(|i| ParticipantCommitment {
            participant_id: i as u8,
            commitment: [i as u8; 66],
        })
        .collect();

    let participants: Vec<u8> = (1..=n as u8).collect();

    let package = SigningPackageMessage {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        commitments,
        participants,
    };

    assert_eq!(package.commitments.len(), 5);
    assert_eq!(package.participants.len(), 5);
}

// ============================================================================
// NODE CONFIGURATION BOUNDARY TESTS
// ============================================================================

#[test]
fn test_node_with_no_peer_pubkeys() {
    // Node with no configured peers (solo mode)
    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        peer_pubkeys: vec![],
        ..Default::default()
    };

    let node = BifrostNode::with_config(config).expect("failed to create node");
    assert!(node.config().peer_pubkeys.is_empty());
}

#[test]
fn test_node_with_many_peer_pubkeys() {
    // Node with many configured peers
    let peer_pubkeys: Vec<[u8; 32]> = (1..=20)
        .map(|i| {
            let mut key = [0u8; 32];
            key[31] = i as u8;
            key
        })
        .collect();

    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        peer_pubkeys: peer_pubkeys.clone(),
        ..Default::default()
    };

    let node = BifrostNode::with_config(config).expect("failed to create node");
    assert_eq!(node.config().peer_pubkeys.len(), 20);
}

#[test]
fn test_node_minimum_timeout() {
    // Very short timeouts
    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        timeouts: TimeoutConfig {
            sign_timeout_ms: 1, // 1ms
            ..Default::default()
        },
        ..Default::default()
    };

    let node = BifrostNode::with_config(config).expect("failed to create node");
    assert_eq!(node.config().timeouts.sign_timeout_ms, 1);
}

#[test]
fn test_node_maximum_timeout() {
    // Very long timeouts
    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        timeouts: TimeoutConfig {
            sign_timeout_ms: u64::MAX,
            ..Default::default()
        },
        ..Default::default()
    };

    let node = BifrostNode::with_config(config).expect("failed to create node");
    assert_eq!(node.config().timeouts.sign_timeout_ms, u64::MAX);
}

// ============================================================================
// FROST SHARE BOUNDARY TESTS
// ============================================================================

#[test]
fn test_frost_share_index_range() {
    // FROST shares have indices from 1 to n
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    assert_eq!(shares[0].participant_id, 1);
    assert_eq!(shares[1].participant_id, 2);
    assert_eq!(shares[2].participant_id, 3);
}

#[test]
fn test_frost_share_secret_not_exposed() {
    // Verify that secret share material is properly encapsulated
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    // The key_package contains the secret share but shouldn't expose raw bytes
    // We can only verify the public interface
    assert!(shares[0].key_package.verifying_share().serialize().is_ok());
}

// ============================================================================
// SERIALIZATION BOUNDARY TESTS
// ============================================================================

#[test]
fn test_message_serialization_deterministic() {
    // Same message should serialize to same JSON
    let msg = BifrostMessage::CommitmentRequest(CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        participants: vec![1, 2, 3],
        initiator_id: 1,
    });

    let json1 = serde_json::to_string(&msg).unwrap();
    let json2 = serde_json::to_string(&msg).unwrap();

    assert_eq!(json1, json2);
}

#[test]
fn test_message_deserialization_whitespace_tolerant() {
    // JSON with extra whitespace should still parse
    let json = r#"{
        "type": "/sign/commit/req",
        "event_hash": [66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66,66],
        "session_id": "test",
        "participants": [1, 2],
        "initiator_id": 1
    }"#;

    let result: Result<BifrostMessage, _> = serde_json::from_str(json);
    assert!(result.is_ok());
}

#[test]
fn test_very_deep_nesting_rejected() {
    // Extremely nested JSON should either be rejected or handled safely
    // This is a security/DoS test
    let mut json = "{".repeat(100);
    json.push_str(&"}".repeat(100));

    let result: Result<BifrostMessage, _> = serde_json::from_str(&json);
    // Should fail to parse as BifrostMessage (wrong structure)
    assert!(result.is_err());
}
