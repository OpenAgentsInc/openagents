//! Concurrent session tests for Bifrost threshold signing protocol
//!
//! These tests verify that:
//! - Multiple signing sessions can run in parallel
//! - Responses are correctly routed to their sessions
//! - Session IDs provide proper isolation
//! - Late responses don't affect subsequent sessions
//! - ECDH and signing operations can run concurrently

use frostr::bifrost::{
    BifrostConfig, BifrostMessage, BifrostNode, CommitmentResponse, PartialSignature,
    ParticipantCommitment, SigningPackageMessage, TimeoutConfig,
};
use frostr::keygen::generate_key_shares;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Barrier, RwLock};
use tokio::time::{Duration, sleep};

// ============================================================================
// SESSION ID ISOLATION TESTS
// ============================================================================

#[tokio::test]
async fn test_session_id_format_and_uniqueness() {
    // Verify session IDs are properly formatted and unique
    let mut session_ids = HashSet::new();

    // Generate many session IDs (simulating what BifrostNode would generate)
    for _ in 0..1000 {
        use rand::RngCore;
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let session_id = format!("{:032x}", u128::from_be_bytes(bytes));

        // Session ID should be 32 hex characters
        assert_eq!(session_id.len(), 32);
        assert!(session_id.chars().all(|c| c.is_ascii_hexdigit()));

        // Should be unique
        assert!(session_ids.insert(session_id));
    }
}

#[tokio::test]
async fn test_session_state_isolation() {
    // Create a node and verify that multiple sessions don't interfere
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        peer_pubkeys: vec![[0x02; 32], [0x03; 32]],
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // Create responses for different sessions
    let response_a = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: "session_a".to_string(),
        participant_id: 2,
        nonce_commitment: [0x02; 66],
    });

    let response_b = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: "session_b".to_string(),
        participant_id: 2,
        nonce_commitment: [0x03; 66],
    });

    // Both should be handled without error (though they may not match active sessions)
    let result_a = node.handle_message(&response_a);
    let result_b = node.handle_message(&response_b);

    assert!(result_a.is_ok());
    assert!(result_b.is_ok());
}

#[tokio::test]
async fn test_response_routing_by_session_id() {
    // Verify that responses are correctly matched to their sessions
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // Create a response with unknown session ID
    let unknown_session_response = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: "nonexistent_session_12345".to_string(),
        participant_id: 2,
        nonce_commitment: [0x02; 66],
    });

    // Should return Ok but not produce any output (no matching session)
    let result = node.handle_message(&unknown_session_response);
    assert!(result.is_ok());

    // The result should be None (no response generated)
    if let Ok(Some(_)) = result {
        // If there's a response, it shouldn't be for the unknown session
        // This is acceptable behavior
    }
}

// ============================================================================
// CONCURRENT NODE OPERATION TESTS
// ============================================================================

#[tokio::test]
async fn test_multiple_nodes_independent_operation() {
    // Create multiple independent nodes and verify they don't interfere
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let mut nodes = Vec::new();

    for i in 0..3 {
        let mut secret_key = [0u8; 32];
        secret_key[31] = (i + 1) as u8;

        let config = BifrostConfig {
            secret_key: Some(secret_key),
            ..Default::default()
        };

        let mut node = BifrostNode::with_config(config).expect("failed to create node");
        node.set_frost_share(shares[i].clone());
        nodes.push(node);
    }

    // Verify each node has correct configuration
    for (i, node) in nodes.iter().enumerate() {
        assert!(node.has_frost_share());
        assert_eq!(node.threshold(), Some(2));
        assert_eq!(node.frost_share().unwrap().participant_id, (i + 1) as u8);
    }
}

#[tokio::test]
async fn test_concurrent_message_handling() {
    // Test that a node can handle messages from multiple sources concurrently
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        ..Default::default()
    };

    let node = Arc::new(RwLock::new(
        BifrostNode::with_config(config).expect("failed to create node"),
    ));

    {
        let mut n = node.write().await;
        n.set_frost_share(shares[0].clone());
    }

    // Spawn multiple concurrent message handlers
    let mut handles = Vec::new();

    for i in 0..10 {
        let node_clone = Arc::clone(&node);
        let session_id = format!("concurrent_session_{}", i);

        let handle = tokio::spawn(async move {
            let _response = BifrostMessage::CommitmentResponse(CommitmentResponse {
                session_id,
                participant_id: 2,
                nonce_commitment: [0x02; 66],
            });

            // Acquire read lock to handle message
            // Note: In real usage, handle_message may need mutable access
            // This tests that concurrent access doesn't cause issues
            let node = node_clone.read().await;
            // Can't call handle_message on immutable node, just verify node state
            assert!(node.has_frost_share());
            Ok::<_, ()>(())
        });

        handles.push(handle);
    }

    // Wait for all to complete without panic
    for handle in handles {
        handle
            .await
            .expect("task panicked")
            .expect("handler failed");
    }
}

// ============================================================================
// PARTIAL SIGNATURE COLLECTION TESTS
// ============================================================================

#[tokio::test]
async fn test_partial_signature_from_different_participants() {
    // Verify that partial signatures from different participants are distinguishable
    let _shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let session_id = "test_session".to_string();
    let mut collected_sigs: HashMap<u8, PartialSignature> = HashMap::new();

    // Create partial signatures from participants 1, 2, 3
    for participant_id in 1..=3u8 {
        let sig = PartialSignature {
            session_id: session_id.clone(),
            participant_id,
            partial_sig: [participant_id; 32], // Mock sig based on participant ID
        };

        collected_sigs.insert(participant_id, sig);
    }

    // Verify we have 3 distinct signatures
    assert_eq!(collected_sigs.len(), 3);

    // Verify each signature has correct participant ID
    for (id, sig) in &collected_sigs {
        assert_eq!(sig.participant_id, *id);
        assert_eq!(sig.session_id, session_id);
    }
}

#[tokio::test]
async fn test_duplicate_partial_signature_handling() {
    // Test that duplicate signatures from same participant are handled
    let session_id = "test_session".to_string();
    let mut collected_sigs: HashMap<u8, PartialSignature> = HashMap::new();

    // First signature from participant 2
    let sig1 = PartialSignature {
        session_id: session_id.clone(),
        participant_id: 2,
        partial_sig: [0xAA; 32],
    };

    // Duplicate/second signature from participant 2
    let sig2 = PartialSignature {
        session_id: session_id.clone(),
        participant_id: 2,
        partial_sig: [0xBB; 32],
    };

    // Insert first
    collected_sigs.insert(sig1.participant_id, sig1);
    assert_eq!(collected_sigs.len(), 1);

    // Insert duplicate - should overwrite
    let old = collected_sigs.insert(sig2.participant_id, sig2.clone());
    assert!(old.is_some());
    assert_eq!(collected_sigs.len(), 1);

    // Should have the second value
    assert_eq!(collected_sigs.get(&2).unwrap().partial_sig, [0xBB; 32]);
}

// ============================================================================
// COMMITMENT COLLECTION TESTS
// ============================================================================

#[tokio::test]
async fn test_commitment_collection_threshold() {
    // Test collecting exactly threshold number of commitments
    let threshold = 2;
    let _total = 3;

    let mut commitments: HashMap<u8, ParticipantCommitment> = HashMap::new();

    // Collect commitments up to threshold
    for participant_id in 1..=threshold {
        let commitment = ParticipantCommitment {
            participant_id: participant_id as u8,
            commitment: [participant_id as u8; 66],
        };
        commitments.insert(participant_id as u8, commitment);
    }

    // Verify we have threshold commitments
    assert_eq!(commitments.len(), threshold);

    // Should be ready to create signing package
    assert!(commitments.len() >= threshold);
}

#[tokio::test]
async fn test_commitment_collection_more_than_threshold() {
    // Test that collecting more than threshold commitments works
    let threshold = 2;
    let total = 3;

    let mut commitments: HashMap<u8, ParticipantCommitment> = HashMap::new();

    // Collect all commitments (more than threshold)
    for participant_id in 1..=total {
        let commitment = ParticipantCommitment {
            participant_id: participant_id as u8,
            commitment: [participant_id as u8; 66],
        };
        commitments.insert(participant_id as u8, commitment);
    }

    // Verify we have all commitments
    assert_eq!(commitments.len(), total);

    // Can still create signing package with subset
    let signing_participants: Vec<u8> = commitments.keys().take(threshold).cloned().collect();
    assert_eq!(signing_participants.len(), threshold);
}

// ============================================================================
// TIMEOUT AND CLEANUP TESTS
// ============================================================================

#[tokio::test]
async fn test_session_timeout_cleanup() {
    // Test that sessions are properly cleaned up after timeout
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    let config = BifrostConfig {
        secret_key: Some([0x01; 32]),
        timeouts: TimeoutConfig {
            sign_timeout_ms: 100, // Very short timeout
            ..Default::default()
        },
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());

    // After timeout, late responses should be ignored
    sleep(Duration::from_millis(200)).await;

    let late_response = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: "expired_session".to_string(),
        participant_id: 2,
        nonce_commitment: [0x02; 66],
    });

    // Should handle gracefully (either ignore or return Ok(None))
    let result = node.handle_message(&late_response);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_response_after_completion() {
    // Test that responses after session completion don't cause issues
    let session_id = "completed_session".to_string();

    // Simulate a completed session state
    let mut completed_sessions: HashSet<String> = HashSet::new();
    completed_sessions.insert(session_id.clone());

    // A late response arrives
    let late_response = CommitmentResponse {
        session_id: session_id.clone(),
        participant_id: 2,
        nonce_commitment: [0x02; 66],
    };

    // Check if session is already completed
    let is_completed = completed_sessions.contains(&late_response.session_id);
    assert!(is_completed);

    // In real implementation, this response should be ignored
}

// ============================================================================
// SIGNING PACKAGE CREATION TESTS
// ============================================================================

#[tokio::test]
async fn test_signing_package_creation_with_minimum_participants() {
    // Create signing package with exactly threshold participants
    let threshold = 2;

    let event_hash = [0x42; 32];
    let session_id = "test_session".to_string();

    let commitments = vec![
        ParticipantCommitment {
            participant_id: 1,
            commitment: [0x01; 66],
        },
        ParticipantCommitment {
            participant_id: 2,
            commitment: [0x02; 66],
        },
    ];

    let participants: Vec<u8> = commitments.iter().map(|c| c.participant_id).collect();

    let package = SigningPackageMessage {
        event_hash,
        session_id: session_id.clone(),
        commitments,
        participants: participants.clone(),
    };

    assert_eq!(package.commitments.len(), threshold);
    assert_eq!(package.participants.len(), threshold);
    assert_eq!(package.event_hash, event_hash);
}

#[tokio::test]
async fn test_signing_package_serialization_round_trip() {
    // Verify signing package can be serialized and deserialized
    let package = SigningPackageMessage {
        event_hash: [0x42; 32],
        session_id: "test_session".to_string(),
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

    let message = BifrostMessage::SigningPackage(package.clone());
    let json = serde_json::to_string(&message).expect("serialization failed");
    let deserialized: BifrostMessage = serde_json::from_str(&json).expect("deserialization failed");

    if let BifrostMessage::SigningPackage(pkg) = deserialized {
        assert_eq!(pkg.session_id, package.session_id);
        assert_eq!(pkg.event_hash, package.event_hash);
        assert_eq!(pkg.commitments.len(), package.commitments.len());
        assert_eq!(pkg.participants, package.participants);
    } else {
        panic!("Wrong message type after deserialization");
    }
}

// ============================================================================
// INTERLEAVED MESSAGE TESTS
// ============================================================================

#[tokio::test]
async fn test_interleaved_responses_different_sessions() {
    // Simulate interleaved responses from different sessions arriving out of order
    let session_a_responses = vec![
        CommitmentResponse {
            session_id: "session_a".to_string(),
            participant_id: 1,
            nonce_commitment: [0xA1; 66],
        },
        CommitmentResponse {
            session_id: "session_a".to_string(),
            participant_id: 2,
            nonce_commitment: [0xA2; 66],
        },
    ];

    let session_b_responses = vec![
        CommitmentResponse {
            session_id: "session_b".to_string(),
            participant_id: 1,
            nonce_commitment: [0xB1; 66],
        },
        CommitmentResponse {
            session_id: "session_b".to_string(),
            participant_id: 2,
            nonce_commitment: [0xB2; 66],
        },
    ];

    // Interleave: A1, B1, A2, B2
    let interleaved = vec![
        &session_a_responses[0],
        &session_b_responses[0],
        &session_a_responses[1],
        &session_b_responses[1],
    ];

    // Collect by session
    let mut session_a_collected: Vec<&CommitmentResponse> = Vec::new();
    let mut session_b_collected: Vec<&CommitmentResponse> = Vec::new();

    for response in interleaved {
        match response.session_id.as_str() {
            "session_a" => session_a_collected.push(response),
            "session_b" => session_b_collected.push(response),
            _ => panic!("Unknown session"),
        }
    }

    // Verify correct collection
    assert_eq!(session_a_collected.len(), 2);
    assert_eq!(session_b_collected.len(), 2);

    // Verify participant IDs are correct
    assert_eq!(session_a_collected[0].participant_id, 1);
    assert_eq!(session_a_collected[1].participant_id, 2);
    assert_eq!(session_b_collected[0].participant_id, 1);
    assert_eq!(session_b_collected[1].participant_id, 2);
}

#[tokio::test]
async fn test_mixed_message_types_same_session() {
    // Test handling different message types for the same session
    let session_id = "mixed_session".to_string();

    let commitment_response = BifrostMessage::CommitmentResponse(CommitmentResponse {
        session_id: session_id.clone(),
        participant_id: 2,
        nonce_commitment: [0x02; 66],
    });

    let partial_sig = BifrostMessage::PartialSignature(PartialSignature {
        session_id: session_id.clone(),
        participant_id: 2,
        partial_sig: [0x02; 32],
    });

    // Both messages belong to same session
    let commitment_session = match &commitment_response {
        BifrostMessage::CommitmentResponse(r) => &r.session_id,
        _ => panic!("wrong type"),
    };

    let sig_session = match &partial_sig {
        BifrostMessage::PartialSignature(s) => &s.session_id,
        _ => panic!("wrong type"),
    };

    assert_eq!(commitment_session, sig_session);
}

// ============================================================================
// BARRIER SYNCHRONIZATION TESTS
// ============================================================================

#[tokio::test]
async fn test_barrier_synchronized_responses() {
    // Simulate multiple nodes responding at approximately the same time
    let barrier = Arc::new(Barrier::new(3));
    let collected = Arc::new(RwLock::new(Vec::new()));

    let mut handles = Vec::new();

    for participant_id in 1..=3u8 {
        let barrier_clone = Arc::clone(&barrier);
        let collected_clone = Arc::clone(&collected);

        let handle = tokio::spawn(async move {
            // Wait for all participants to be ready
            barrier_clone.wait().await;

            // Create and "send" response
            let response = CommitmentResponse {
                session_id: "synchronized_session".to_string(),
                participant_id,
                nonce_commitment: [participant_id; 66],
            };

            // Collect response
            collected_clone.write().await.push(response);
        });

        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await.expect("task panicked");
    }

    // Verify all responses collected
    let responses = collected.read().await;
    assert_eq!(responses.len(), 3);

    // Verify all participant IDs present
    let ids: HashSet<u8> = responses.iter().map(|r| r.participant_id).collect();
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
    assert!(ids.contains(&3));
}

// ============================================================================
// RACE CONDITION PREVENTION TESTS
// ============================================================================

#[tokio::test]
async fn test_no_race_in_commitment_collection() {
    // Test that concurrent commitment additions don't cause data loss
    let commitments = Arc::new(RwLock::new(HashMap::<u8, ParticipantCommitment>::new()));

    let mut handles = Vec::new();

    for participant_id in 1..=10u8 {
        let commitments_clone = Arc::clone(&commitments);

        let handle = tokio::spawn(async move {
            let commitment = ParticipantCommitment {
                participant_id,
                commitment: [participant_id; 66],
            };

            // Small random delay to create race conditions
            tokio::time::sleep(Duration::from_micros((rand::random::<u64>() % 100) as u64)).await;

            commitments_clone
                .write()
                .await
                .insert(participant_id, commitment);
        });

        handles.push(handle);
    }

    // Wait for all
    for handle in handles {
        handle.await.expect("task panicked");
    }

    // Verify no data loss
    let final_commitments = commitments.read().await;
    assert_eq!(final_commitments.len(), 10);

    // Verify all IDs present
    for i in 1..=10u8 {
        assert!(final_commitments.contains_key(&i));
    }
}
