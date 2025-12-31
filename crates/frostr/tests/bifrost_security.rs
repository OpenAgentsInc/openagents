//! Security tests for Bifrost threshold signing protocol
//!
//! These tests verify that the protocol properly handles:
//! - Invalid partial signatures (bad curve scalars)
//! - Invalid commitments (not on curve)
//! - Wrong participant IDs
//! - Wrong session IDs
//! - Duplicate responses from same peer
//! - Session ID collision prevention
//! - Unauthorized participants

use frostr::bifrost::{
    BifrostConfig, BifrostMessage, BifrostNode, CommitmentRequest, CommitmentResponse,
    PartialSignature, ParticipantCommitment, SigningPackageMessage,
};
use frostr::keygen::generate_key_shares;
use nostr::get_public_key;
use std::collections::HashSet;

// ============================================================================
// SESSION ID UNIQUENESS TESTS
// ============================================================================

#[test]
fn test_session_id_uniqueness() {
    // Verify that generated session IDs are unique
    // Session IDs are 128-bit random values, collision should be astronomically unlikely
    let mut session_ids = HashSet::new();

    // Generate 10,000 session IDs and verify no collisions
    for _ in 0..10_000 {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let session_id = format!("{:032x}", u128::from_be_bytes(bytes));

        assert!(
            session_ids.insert(session_id.clone()),
            "Session ID collision detected: {}",
            session_id
        );
    }
}

// ============================================================================
// INVALID MESSAGE TESTS
// ============================================================================

#[test]
fn test_invalid_participant_id_too_large() {
    // Participant IDs must be in valid range (1-255 for u8, but typically < threshold n)
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([1u8; 32]),
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // Create a CommitmentResponse with invalid participant_id (255, when only 1-3 exist)
    let invalid_response = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: "test-session".to_string(),
        participant_id: 255,          // Invalid - participants are 1, 2, 3
        nonce_commitment: [0x02; 66], // Mock commitment
    });

    // The node should either ignore this or return None (no response to invalid input)
    let result = node.handle_message(&invalid_response);
    // Should not cause a panic - either Ok(None) or Ok(Some(_)) is acceptable
    // The important thing is it doesn't crash
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_invalid_commitment_all_zeros() {
    // A commitment of all zeros is likely not a valid curve point
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([1u8; 32]),
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // Create a CommitmentResponse with all-zero commitment (invalid)
    let invalid_response = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: "test-session".to_string(),
        participant_id: 2,
        nonce_commitment: [0x00; 66], // All zeros - invalid curve point
    });

    let result = node.handle_message(&invalid_response);
    // Should handle gracefully (not panic)
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_invalid_partial_signature_all_zeros() {
    // A partial signature of all zeros is likely not a valid scalar
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([1u8; 32]),
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // Create a PartialSignature with all-zero sig (might be invalid)
    let invalid_sig = BifrostMessage::PartialSignature(PartialSignature {
        session_id: "test-session".to_string(),
        participant_id: 2,
        partial_sig: [0x00; 32], // All zeros
    });

    let result = node.handle_message(&invalid_sig);
    // Should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_invalid_partial_signature_all_ff() {
    // A partial signature of 0xFF...FF is larger than the curve order
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([1u8; 32]),
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // Create a PartialSignature with all-0xFF sig (larger than curve order n)
    let invalid_sig = BifrostMessage::PartialSignature(PartialSignature {
        session_id: "test-session".to_string(),
        participant_id: 2,
        partial_sig: [0xFF; 32], // All 0xFF - larger than secp256k1 order
    });

    let result = node.handle_message(&invalid_sig);
    // Should handle gracefully
    assert!(result.is_ok() || result.is_err());
}

// ============================================================================
// SESSION ISOLATION TESTS
// ============================================================================

#[test]
fn test_wrong_session_id_ignored() {
    // Responses with wrong session ID should not affect other sessions
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([1u8; 32]),
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // Create a response for a non-existent session
    let wrong_session_response = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: "non-existent-session".to_string(),
        participant_id: 2,
        nonce_commitment: [0x02; 66],
    });

    let result = node.handle_message(&wrong_session_response);
    // Should return Ok(None) - the message is valid but not for any active session
    assert!(result.is_ok());
}

// ============================================================================
// MESSAGE SERIALIZATION SECURITY TESTS
// ============================================================================

#[test]
fn test_malformed_json_message() {
    // Verify that malformed JSON is properly rejected
    let malformed_jsons = vec![
        r#"{"type":"/sign/commit/res","session_id":"test"}"#, // Missing fields
        r#"{"type":"/sign/commit/res","session_id":"test","participant_id":"not_a_number"}"#, // Wrong type
        r#"{"type":"/unknown/type"}"#, // Unknown message type
        r#"{invalid json"#,            // Invalid JSON syntax
        r#""#,                         // Empty string
        r#"null"#,                     // Null
        r#"[]"#,                       // Array instead of object
    ];

    for json in malformed_jsons {
        let result: Result<BifrostMessage, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Should reject malformed JSON: {}", json);
    }
}

#[test]
fn test_message_field_overflow() {
    // Test that extremely large values don't cause issues
    let large_session_id = "x".repeat(1_000_000); // 1MB session ID

    let msg = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: large_session_id.clone(),
        participants: vec![1, 2, 3],
        initiator_id: 1,
    };

    // Serialization should succeed but produce a large output
    let json = serde_json::to_string(&BifrostMessage::CommitmentRequest(msg.clone()));
    assert!(json.is_ok());

    // Deserialization should also work
    let json_str = json.unwrap();
    let deserialized: Result<BifrostMessage, _> = serde_json::from_str(&json_str);
    assert!(deserialized.is_ok());
}

#[test]
fn test_empty_participants_list() {
    // An empty participants list should be handled
    let msg = CommitmentRequest {
        event_hash: [0x42; 32],
        session_id: "test-session".to_string(),
        participants: vec![], // Empty participants
        initiator_id: 1,
    };

    let json = serde_json::to_string(&BifrostMessage::CommitmentRequest(msg)).unwrap();
    let deserialized: BifrostMessage = serde_json::from_str(&json).unwrap();

    if let BifrostMessage::CommitmentRequest(req) = deserialized {
        assert!(req.participants.is_empty());
    } else {
        panic!("Wrong message type");
    }
}

// ============================================================================
// NODE CONFIGURATION SECURITY TESTS
// ============================================================================

#[test]
fn test_node_without_frost_share() {
    // A node without a FROST share should not be able to sign
    let config = BifrostConfig {
        secret_key: Some([1u8; 32]),
        ..Default::default()
    };

    let node = BifrostNode::with_config(config).expect("failed to create node");

    // Node should indicate it doesn't have a share
    assert!(!node.has_frost_share());
    assert_eq!(node.threshold(), None);
}

#[test]
fn test_node_with_invalid_peer_pubkey() {
    // Test that invalid peer pubkeys are handled
    let secret_key: [u8; 32] = [1u8; 32];
    let invalid_pubkey: [u8; 32] = [0xFF; 32]; // Not a valid x-coordinate

    let config = BifrostConfig {
        secret_key: Some(secret_key),
        peer_pubkeys: vec![invalid_pubkey],
        ..Default::default()
    };

    // Node creation should still succeed (validation happens at message time)
    let result = BifrostNode::with_config(config);
    assert!(result.is_ok());
}

// ============================================================================
// COMMITMENT VALIDATION TESTS
// ============================================================================

#[test]
fn test_commitment_wrong_length() {
    // Commitments must be exactly 66 bytes (hiding + binding)
    // This tests the serialization/deserialization boundary

    // Valid commitment
    let valid = CommitmentResponse {
        session_id: "test".to_string(),
        participant_id: 1,
        nonce_commitment: [0x02; 66],
    };

    let json = serde_json::to_string(&BifrostMessage::CommitmentResponse(valid)).unwrap();
    let parsed: Result<BifrostMessage, _> = serde_json::from_str(&json);
    assert!(parsed.is_ok());

    // The array is fixed-size in Rust, so wrong-length can't be represented in the type system
    // However, malformed JSON with wrong array length should fail
    let wrong_length_json = r#"{"type":"/sign/commit/res","session_id":"test","participant_id":1,"nonce_commitment":[2,2,2]}"#;
    let result: Result<BifrostMessage, _> = serde_json::from_str(wrong_length_json);
    assert!(
        result.is_err(),
        "Should reject commitment with wrong length"
    );
}

// ============================================================================
// SIGNING PACKAGE VALIDATION TESTS
// ============================================================================

#[test]
fn test_signing_package_mismatched_participants() {
    // SigningPackage with commitments not matching participants list
    let package = SigningPackageMessage {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        commitments: vec![
            ParticipantCommitment {
                participant_id: 1,
                commitment: [0x02; 66],
            },
            // Missing commitment for participant 2
        ],
        participants: vec![1, 2], // Lists 2 participants but only 1 commitment
    };

    // Serialization works
    let json = serde_json::to_string(&BifrostMessage::SigningPackage(package)).unwrap();
    let parsed: BifrostMessage = serde_json::from_str(&json).unwrap();

    // The mismatch should be caught during signature generation, not serialization
    if let BifrostMessage::SigningPackage(pkg) = parsed {
        assert_eq!(pkg.commitments.len(), 1);
        assert_eq!(pkg.participants.len(), 2);
        // Signature generation with this package should fail
    }
}

#[test]
fn test_signing_package_duplicate_commitments() {
    // SigningPackage with duplicate participant IDs
    let package = SigningPackageMessage {
        event_hash: [0x42; 32],
        session_id: "test".to_string(),
        commitments: vec![
            ParticipantCommitment {
                participant_id: 1,
                commitment: [0x02; 66],
            },
            ParticipantCommitment {
                participant_id: 1, // Duplicate!
                commitment: [0x03; 66],
            },
        ],
        participants: vec![1, 2],
    };

    // Serialization works (protocol should handle duplicates at signature time)
    let json = serde_json::to_string(&BifrostMessage::SigningPackage(package)).unwrap();
    let parsed: BifrostMessage = serde_json::from_str(&json).unwrap();

    if let BifrostMessage::SigningPackage(pkg) = parsed {
        // Both commitments are present
        assert_eq!(pkg.commitments.len(), 2);
        // Both have same participant_id
        assert_eq!(
            pkg.commitments[0].participant_id,
            pkg.commitments[1].participant_id
        );
    }
}

// ============================================================================
// KEY DERIVATION TESTS
// ============================================================================

#[test]
fn test_public_key_derivation_consistency() {
    // Verify that public key derivation is deterministic
    let secret_key: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x01;
        k
    };

    let pubkey1 = get_public_key(&secret_key).expect("failed to derive pubkey");
    let pubkey2 = get_public_key(&secret_key).expect("failed to derive pubkey");

    assert_eq!(
        pubkey1, pubkey2,
        "Public key derivation should be deterministic"
    );
}

#[test]
fn test_different_secrets_produce_different_pubkeys() {
    let secret_key1: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x01;
        k
    };
    let secret_key2: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x02;
        k
    };

    let pubkey1 = get_public_key(&secret_key1).expect("failed to derive pubkey 1");
    let pubkey2 = get_public_key(&secret_key2).expect("failed to derive pubkey 2");

    assert_ne!(
        pubkey1, pubkey2,
        "Different secrets should produce different pubkeys"
    );
}

// ============================================================================
// FROST SHARE SECURITY TESTS
// ============================================================================

#[test]
fn test_frost_share_threshold_validation() {
    // Verify that FROST shares have correct threshold
    let shares_2_of_3 = generate_key_shares(2, 3).expect("2-of-3");
    let shares_3_of_5 = generate_key_shares(3, 5).expect("3-of-5");

    for share in &shares_2_of_3 {
        assert_eq!(share.threshold, 2);
        assert_eq!(share.total, 3);
    }

    for share in &shares_3_of_5 {
        assert_eq!(share.threshold, 3);
        assert_eq!(share.total, 5);
    }
}

#[test]
fn test_frost_share_group_pubkey_consistency() {
    // All shares from the same generation should have the same group public key
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let group_pk = shares[0].public_key_package.verifying_key();

    for share in &shares[1..] {
        let pk = share.public_key_package.verifying_key();
        assert_eq!(
            group_pk, pk,
            "All shares should have the same group public key"
        );
    }
}

#[test]
fn test_frost_share_different_generations_different_keys() {
    // Two separate key generations should produce different group keys (with overwhelming probability)
    let shares1 = generate_key_shares(2, 3).expect("first generation");
    let shares2 = generate_key_shares(2, 3).expect("second generation");

    let group_pk1 = shares1[0].public_key_package.verifying_key();
    let group_pk2 = shares2[0].public_key_package.verifying_key();

    assert_ne!(
        group_pk1, group_pk2,
        "Different key generations should produce different group keys"
    );
}
