//! End-to-end integration tests for Bifrost threshold operations over real Nostr relays
//!
//! These tests verify that the complete NIP-SA and Bifrost stack works correctly
//! over real relay connections, testing:
//! - Threshold signing (2-of-3, 3-of-5 configurations)
//! - Threshold ECDH for encryption
//! - Peer discovery and coordination
//! - Timeout and error handling
//!
//! Unlike integration_signing.rs which uses mock transport, these tests use
//! actual in-process Nostr relays to ensure realistic interoperability.

use frost_secp256k1::Signature as FrostSignature;
use frostr::bifrost::{BifrostConfig, BifrostNode, TimeoutConfig};
use frostr::ecdh::threshold_ecdh;
use frostr::keygen::generate_key_shares;
use frostr::signing::verify_signature;
use nostr::{decrypt_v2, encrypt_v2, generate_secret_key, get_public_key};
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use std::sync::Arc;
use tokio::time::{Duration, sleep};

/// Helper macro to run responder inline alongside coordinator operation.
/// This avoids ownership issues with spawning tasks.
macro_rules! with_responder {
    ($responder:expr, $coordinator:expr) => {{
        tokio::select! {
            biased;
            result = $coordinator => result,
            _ = $responder => {
                Err(frostr::Error::Protocol("responder exited unexpectedly".into()))
            }
        }
    }};
}

/// Run a responder loop for a node (keeps running until an error or no more messages)
async fn run_responder_loop(node: &BifrostNode) -> frostr::Result<()> {
    let transport = node
        .transport()
        .ok_or_else(|| frostr::Error::Protocol("Transport not initialized".into()))?;

    // Keep handling messages until timeout (no more messages)
    loop {
        match tokio::time::timeout(Duration::from_secs(30), transport.receive()).await {
            Ok(Ok(message)) => {
                // Handle the message and get optional response
                if let Ok(Some(response)) = node.handle_message(&message) {
                    // Broadcast response back
                    transport.broadcast(&response).await?;
                }
                // Continue loop to handle more messages
            }
            Ok(Err(_e)) => {
                // Transport error - continue running, might recover
            }
            Err(_) => {
                // Timeout - no messages, exit normally
                return Ok(());
            }
        }
    }
}

/// Test helper: Start an in-process test relay and return its WebSocket URL
async fn start_test_relay(port: u16) -> (Arc<RelayServer>, tempfile::TempDir) {
    let config = RelayConfig {
        bind_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        ..Default::default()
    };

    // Create temp dir for database
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_config = DatabaseConfig {
        path: db_path,
        ..Default::default()
    };

    let db = Database::new(db_config).unwrap();
    let server = Arc::new(RelayServer::new(config, db));

    // Start server in background
    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        server_clone.start().await.ok();
    });

    // Give server time to start
    sleep(Duration::from_millis(200)).await;

    (server, temp_dir)
}

/// Get test relay WebSocket URL for given port
fn test_relay_url(port: u16) -> String {
    format!("ws://127.0.0.1:{}", port)
}

#[tokio::test]
async fn test_bifrost_signing_2_of_3_over_relay() {
    // 1. Start test relay
    let port = 19000;
    let (_server, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    // 2. Generate 2-of-3 FROST shares
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");
    assert_eq!(shares.len(), 3);
    let group_pk = shares[0].public_key_package.verifying_key();

    // 3. Create BifrostNode instances for 2 peers pointing at test relay
    // Each peer needs unique secret key for Nostr identity
    // Use valid secp256k1 secret keys (32-byte scalars)
    let secret_key_1: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x01; // Scalar value 1
        k
    };
    let secret_key_2: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x02; // Scalar value 2
        k
    };

    // Derive actual public keys from secret keys using nostr NIP-01
    let peer_pubkey_1 = nostr::get_public_key(&secret_key_1).expect("failed to derive pubkey 1");
    let peer_pubkey_2 = nostr::get_public_key(&secret_key_2).expect("failed to derive pubkey 2");

    let config_1 = BifrostConfig {
        default_relays: vec![relay_url.clone()],
        secret_key: Some(secret_key_1),
        peer_pubkeys: vec![peer_pubkey_2], // Peer 1 knows about Peer 2
        timeouts: TimeoutConfig {
            sign_timeout_ms: 10000,
            ..Default::default()
        },
        ..Default::default()
    };

    let config_2 = BifrostConfig {
        default_relays: vec![relay_url],
        secret_key: Some(secret_key_2),
        peer_pubkeys: vec![peer_pubkey_1], // Peer 2 knows about Peer 1
        timeouts: TimeoutConfig {
            sign_timeout_ms: 10000,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut node_1 = BifrostNode::with_config(config_1).expect("failed to create node 1");
    let mut node_2 = BifrostNode::with_config(config_2).expect("failed to create node 2");

    // Set FROST shares on both nodes
    node_1.set_frost_share(shares[0].clone());
    node_2.set_frost_share(shares[1].clone());

    // Start both nodes
    node_1.start().await.expect("failed to start node 1");
    node_2.start().await.expect("failed to start node 2");

    // Give nodes time to connect to relay
    sleep(Duration::from_millis(500)).await;

    // 4. Execute signing round with event hash
    // Run responder for node_2 alongside coordinator operation on node_1
    let event_hash = [0x42; 32];

    // Use select to run responder and coordinator concurrently
    let result = with_responder!(run_responder_loop(&node_2), node_1.sign(&event_hash));

    // Signing should succeed
    let signature = result.expect("signing should succeed");
    assert_eq!(signature.len(), 64, "signature should be 64 bytes");

    // Verify signature against group public key using FROST schnorr verification
    let mut signature_bytes = [0u8; 65];
    signature_bytes[1..33].copy_from_slice(&signature[..32]);
    signature_bytes[33..65].copy_from_slice(&signature[32..]);

    let mut verified = false;
    for prefix in [0x02u8, 0x03u8] {
        signature_bytes[0] = prefix;
        if let Ok(frost_signature) = FrostSignature::deserialize(&signature_bytes) {
            if verify_signature(&event_hash, &frost_signature, &group_pk).is_ok() {
                verified = true;
                break;
            }
        }
    }

    assert!(verified, "signature should verify against group public key");

    // 5. Clean up
    node_1.stop().await.ok();
    node_2.stop().await.ok();
}

#[tokio::test]
async fn test_bifrost_peer_ping_over_relay() {
    // 1. Start test relay
    let port = 19004;
    let (_server, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    // 2. Generate 2-of-3 FROST shares
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");
    assert_eq!(shares.len(), 3);

    // 3. Create Bifrost nodes with peer pubkeys configured
    let secret_key_1: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x01;
        k
    };
    let secret_key_2: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x02;
        k
    };

    let peer_pubkey_1 = get_public_key(&secret_key_1).expect("failed to derive pubkey 1");
    let peer_pubkey_2 = get_public_key(&secret_key_2).expect("failed to derive pubkey 2");

    let config_1 = BifrostConfig {
        default_relays: vec![relay_url.clone()],
        secret_key: Some(secret_key_1),
        peer_pubkeys: vec![peer_pubkey_2],
        timeouts: TimeoutConfig {
            default_timeout_ms: 5000,
            ..Default::default()
        },
        ..Default::default()
    };

    let config_2 = BifrostConfig {
        default_relays: vec![relay_url],
        secret_key: Some(secret_key_2),
        peer_pubkeys: vec![peer_pubkey_1],
        timeouts: TimeoutConfig {
            default_timeout_ms: 5000,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut node_1 = BifrostNode::with_config(config_1).expect("failed to create node 1");
    let mut node_2 = BifrostNode::with_config(config_2).expect("failed to create node 2");

    node_1.set_frost_share(shares[0].clone());
    node_2.set_frost_share(shares[1].clone());

    node_1.add_peer(peer_pubkey_2);
    node_2.add_peer(peer_pubkey_1);

    node_1.start().await.expect("failed to start node 1");
    node_2.start().await.expect("failed to start node 2");

    sleep(Duration::from_millis(500)).await;

    let pong = node_1
        .ping(&peer_pubkey_2)
        .await
        .expect("ping should succeed");
    assert!(pong, "peer should respond to ping");

    assert!(
        node_1.get_peer_latency(&peer_pubkey_2).is_some(),
        "peer latency should be recorded after ping"
    );

    node_1.stop().await.ok();
    node_2.stop().await.ok();
}

#[tokio::test]
async fn test_bifrost_ecdh_2_of_3_over_relay() {
    // 1. Start test relay
    let port = 19001;
    let (_server, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    // 2. Generate 2-of-3 FROST shares
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");
    assert_eq!(shares.len(), 3);

    // 3. Generate an external peer public key for ECDH
    // In a real scenario, this would be another agent's public key
    let peer_secret_key = generate_secret_key();
    let peer_pubkey_bytes =
        nostr::get_public_key(&peer_secret_key).expect("should derive public key from secret key");

    // 4. Create BifrostNode instances with properly derived pubkeys
    let secret_key_1: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x01;
        k
    };
    let secret_key_2: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x02;
        k
    };

    let peer_pubkey_1 = nostr::get_public_key(&secret_key_1).expect("failed to derive pubkey 1");
    let peer_pubkey_2 = nostr::get_public_key(&secret_key_2).expect("failed to derive pubkey 2");

    let config_1 = BifrostConfig {
        default_relays: vec![relay_url.clone()],
        secret_key: Some(secret_key_1),
        peer_pubkeys: vec![peer_pubkey_2],
        timeouts: TimeoutConfig {
            ecdh_timeout_ms: 10000,
            ..Default::default()
        },
        ..Default::default()
    };

    let config_2 = BifrostConfig {
        default_relays: vec![relay_url],
        secret_key: Some(secret_key_2),
        peer_pubkeys: vec![peer_pubkey_1],
        timeouts: TimeoutConfig {
            ecdh_timeout_ms: 10000,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut node_1 = BifrostNode::with_config(config_1).expect("failed to create node 1");
    let mut node_2 = BifrostNode::with_config(config_2).expect("failed to create node 2");

    node_1.set_frost_share(shares[0].clone());
    node_2.set_frost_share(shares[1].clone());

    // Start nodes
    node_1.start().await.expect("failed to start node 1");
    node_2.start().await.expect("failed to start node 2");

    sleep(Duration::from_millis(500)).await;

    // 5. Test BifrostNode.ecdh() method with responder loop
    let result = with_responder!(run_responder_loop(&node_2), node_1.ecdh(&peer_pubkey_bytes));

    // ECDH should succeed over relay
    let shared_secret = result.expect("ECDH should succeed over relay");
    assert_eq!(shared_secret.len(), 32, "shared secret should be 32 bytes");

    // 6. Demonstrate local threshold ECDH (without relay coordination)
    // This uses the local threshold_ecdh function directly
    let local_shared_secret = threshold_ecdh(&shares[0..2], &peer_pubkey_bytes)
        .expect("local threshold ECDH should succeed");

    // 7. Verify both methods produce the same shared secret
    assert_eq!(
        shared_secret, local_shared_secret,
        "relay and local ECDH should produce same shared secret"
    );

    // 8. Clean up
    node_1.stop().await.ok();
    node_2.stop().await.ok();
}

#[tokio::test]
async fn test_bifrost_3_of_5_signing() {
    // 1. Start test relay
    let port = 19002;
    let (_server, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    // 2. Generate 3-of-5 FROST shares
    let shares = generate_key_shares(3, 5).expect("failed to generate shares");
    assert_eq!(shares.len(), 5);
    let _group_pk = shares[0].public_key_package.verifying_key();

    // 3. Create BifrostNode instances for 3 peers (quorum size)
    // First generate all secret keys and derive their pubkeys
    let secret_keys: Vec<[u8; 32]> = (0..3)
        .map(|i| {
            let mut k = [0u8; 32];
            k[31] = (i + 1) as u8;
            k
        })
        .collect();

    let pubkeys: Vec<[u8; 32]> = secret_keys
        .iter()
        .map(|sk| nostr::get_public_key(sk).expect("failed to derive pubkey"))
        .collect();

    let mut nodes = Vec::new();
    for i in 0..3 {
        let peer_pubkeys: Vec<[u8; 32]> = (0..3).filter(|&j| j != i).map(|j| pubkeys[j]).collect();

        let config = BifrostConfig {
            default_relays: vec![relay_url.clone()],
            secret_key: Some(secret_keys[i]),
            peer_pubkeys,
            timeouts: TimeoutConfig {
                sign_timeout_ms: 10000,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut node = BifrostNode::with_config(config).expect("failed to create node");
        node.set_frost_share(shares[i].clone());
        nodes.push(node);
    }

    // 4. Start all nodes
    for node in &mut nodes {
        node.start().await.expect("failed to start node");
    }
    sleep(Duration::from_millis(500)).await;

    // 5. Execute signing round with event hash
    // Run responders for nodes[1] and nodes[2] alongside coordinator on nodes[0]
    let event_hash = [0x42; 32];

    // Use a custom select that prioritizes coordinator result but doesn't fail on responder exit
    let result = async {
        tokio::select! {
            biased;
            // Coordinator has priority - if it completes, we use its result
            r = nodes[0].sign(&event_hash) => r,
            // Responders run concurrently - we don't care if they exit after handling requests
            r = async {
                let _ = tokio::join!(
                    run_responder_loop(&nodes[1]),
                    run_responder_loop(&nodes[2])
                );
                // If both responders exit without coordinator finishing, it's a timeout
                Err::<[u8; 64], _>(frostr::Error::Timeout)
            } => r,
        }
    }
    .await;

    // Signing should succeed over relay
    let signature = result.expect("3-of-5 signing should succeed over relay");
    assert_eq!(signature.len(), 64, "signature should be 64 bytes");

    // 6. Clean up
    for node in &mut nodes {
        node.stop().await.ok();
    }
}

#[tokio::test]
async fn test_bifrost_any_quorum_produces_same_signature() {
    // Generate 2-of-3 FROST shares
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");
    let group_pk = shares[0].public_key_package.verifying_key();
    let event_hash = [0xAB; 32];

    use frost_secp256k1::SigningPackage;
    use frostr::signing::{aggregate_signatures, round1_commit, round2_sign, verify_signature};
    use std::collections::BTreeMap;

    // Test all possible 2-of-3 quorums: (0,1), (0,2), (1,2)
    let quorums: Vec<Vec<usize>> = vec![vec![0, 1], vec![0, 2], vec![1, 2]];

    for quorum in &quorums {
        // Round 1: Generate nonces for this quorum
        let mut nonces_list = Vec::new();
        let mut commitments_map = BTreeMap::new();
        for &i in quorum {
            let (nonces, commitments) = round1_commit(&shares[i]);
            let id = frost_secp256k1::Identifier::try_from((i + 1) as u16).unwrap();
            commitments_map.insert(id, commitments);
            nonces_list.push(nonces);
        }

        // Round 2: Generate signature shares
        let signing_package = SigningPackage::new(commitments_map, &event_hash);
        let mut sig_shares = BTreeMap::new();
        for (idx, &i) in quorum.iter().enumerate() {
            let sig_share = round2_sign(&shares[i], &nonces_list[idx], &signing_package)
                .expect("round2 should succeed");
            let id = frost_secp256k1::Identifier::try_from((i + 1) as u16).unwrap();
            sig_shares.insert(id, sig_share);
        }

        // Aggregate signatures
        let signature = aggregate_signatures(&signing_package, &sig_shares, &shares[quorum[0]])
            .expect("aggregation should succeed");

        // Verify signature - all quorums should produce valid signatures against same group key
        verify_signature(&event_hash, &signature, &group_pk).expect(&format!(
            "signature from quorum {:?} should be valid against group public key",
            quorum
        ));
    }
}

#[tokio::test]
async fn test_bifrost_timeout_handling() {
    // 1. Start test relay
    let port = 19003;
    let (_server, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    // 2. Generate 2-of-3 FROST shares
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    // 3. Create ONLY ONE node (so no peer responds)
    let secret_key: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x01;
        k
    };
    // Use a fake peer pubkey that won't respond
    let fake_peer_secret: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x02;
        k
    };
    let fake_peer_pubkey =
        nostr::get_public_key(&fake_peer_secret).expect("failed to derive fake peer pubkey");

    let config = BifrostConfig {
        default_relays: vec![relay_url],
        secret_key: Some(secret_key),
        peer_pubkeys: vec![fake_peer_pubkey], // Peer that doesn't exist
        timeouts: TimeoutConfig {
            sign_timeout_ms: 1000, // Short timeout
            ecdh_timeout_ms: 1000,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut node = BifrostNode::with_config(config).expect("failed to create node");
    node.set_frost_share(shares[0].clone());
    node.start().await.expect("failed to start node");
    sleep(Duration::from_millis(300)).await;

    // 4. Try signing - should timeout since no peer responds
    let event_hash = [0x42; 32];
    let result = node.sign(&event_hash).await;

    // Should fail with timeout error
    assert!(result.is_err(), "should fail when no peers respond");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains("timeout")
            || err_msg.contains("relay")
            || err_msg.contains("connect")
            || err_msg.contains("Transport"),
        "expected timeout or transport error, got: {}",
        err_msg
    );

    // 5. Clean up
    node.stop().await.ok();
}

#[tokio::test]
async fn test_bifrost_local_ecdh_quorum_determinism() {
    // Generate 2-of-3 FROST shares
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    // Generate a peer public key (use generator point x-coordinate)
    let peer_pubkey: [u8; 32] = [
        0x79, 0xBE, 0x66, 0x7E, 0xF9, 0xDC, 0xBB, 0xAC, 0x55, 0xA0, 0x62, 0x95, 0xCE, 0x87, 0x0B,
        0x07, 0x02, 0x9B, 0xFC, 0xDB, 0x2D, 0xCE, 0x28, 0xD9, 0x59, 0xF2, 0x81, 0x5B, 0x16, 0xF8,
        0x17, 0x98,
    ];

    // Test all possible 2-of-3 quorums
    let secret_01 = threshold_ecdh(&[shares[0].clone(), shares[1].clone()], &peer_pubkey)
        .expect("ECDH with shares 0,1 should succeed");
    let secret_02 = threshold_ecdh(&[shares[0].clone(), shares[2].clone()], &peer_pubkey)
        .expect("ECDH with shares 0,2 should succeed");
    let secret_12 = threshold_ecdh(&[shares[1].clone(), shares[2].clone()], &peer_pubkey)
        .expect("ECDH with shares 1,2 should succeed");

    // All quorums should produce the same shared secret
    assert_eq!(
        secret_01, secret_02,
        "ECDH with quorum (0,1) should equal quorum (0,2)"
    );
    assert_eq!(
        secret_02, secret_12,
        "ECDH with quorum (0,2) should equal quorum (1,2)"
    );

    // Verify it's a non-zero secret
    assert_ne!(secret_01, [0u8; 32], "shared secret should not be zero");
}

// ============================================================================
// NIP-44 ENCRYPTION TESTS
// ============================================================================

/// Helper: Create a compressed public key from x-only pubkey bytes
fn to_compressed_pubkey(xonly: &[u8; 32]) -> [u8; 33] {
    let mut compressed = [0u8; 33];
    compressed[0] = 0x02; // Even y-coordinate prefix
    compressed[1..].copy_from_slice(xonly);
    compressed
}

#[tokio::test]
async fn test_nip44_encryption_round_trip() {
    // Create two key pairs (simulating two Bifrost nodes)
    let secret_key_a = generate_secret_key();
    let secret_key_b = generate_secret_key();

    let pubkey_a = get_public_key(&secret_key_a).expect("failed to derive pubkey A");
    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");

    // Message to encrypt
    let plaintext =
        r#"{"session_id":"test-session","msg_type":"commit_req","message":"hello bifrost"}"#;

    // Node A encrypts for Node B
    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let ciphertext =
        encrypt_v2(&secret_key_a, &compressed_b, plaintext).expect("encryption should succeed");

    // Verify ciphertext is different from plaintext (actually encrypted)
    assert_ne!(
        ciphertext, plaintext,
        "ciphertext should differ from plaintext"
    );
    assert!(
        ciphertext.len() > plaintext.len(),
        "ciphertext should be longer than plaintext"
    );

    // Node B decrypts message from Node A
    let compressed_a = to_compressed_pubkey(&pubkey_a);
    let decrypted =
        decrypt_v2(&secret_key_b, &compressed_a, &ciphertext).expect("decryption should succeed");

    // Verify round-trip integrity
    assert_eq!(
        decrypted, plaintext,
        "decrypted message should match original"
    );
}

#[tokio::test]
async fn test_nip44_wrong_key_decryption_fails() {
    // Create three key pairs
    let secret_key_a = generate_secret_key();
    let secret_key_b = generate_secret_key();
    let secret_key_c = generate_secret_key();

    let pubkey_a = get_public_key(&secret_key_a).expect("failed to derive pubkey A");
    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");

    // Ensure C is different from A and B
    assert_ne!(secret_key_c, secret_key_a);
    assert_ne!(secret_key_c, secret_key_b);

    // Node A encrypts for Node B
    let plaintext = "secret bifrost message for node B only";
    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let ciphertext =
        encrypt_v2(&secret_key_a, &compressed_b, plaintext).expect("encryption should succeed");

    // Node C (unauthorized) attempts to decrypt - should fail
    // Node C uses its own secret key to try decrypting message from A
    let compressed_a = to_compressed_pubkey(&pubkey_a);
    let result = decrypt_v2(&secret_key_c, &compressed_a, &ciphertext);

    // Decryption should fail because C doesn't share a secret with A for this message
    assert!(result.is_err(), "decryption with wrong key should fail");
}

#[tokio::test]
async fn test_nip44_corrupted_ciphertext_rejected() {
    // Create two key pairs
    let secret_key_a = generate_secret_key();
    let secret_key_b = generate_secret_key();

    let pubkey_a = get_public_key(&secret_key_a).expect("failed to derive pubkey A");
    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");

    // Node A encrypts for Node B
    let plaintext = "important signing data";
    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let ciphertext =
        encrypt_v2(&secret_key_a, &compressed_b, plaintext).expect("encryption should succeed");

    // Corrupt the ciphertext by flipping bits
    let mut corrupted_bytes = ciphertext.clone().into_bytes();
    if corrupted_bytes.len() > 20 {
        // Flip some bytes in the middle of the ciphertext
        corrupted_bytes[10] ^= 0xFF;
        corrupted_bytes[15] ^= 0xAA;
    }
    let corrupted = String::from_utf8_lossy(&corrupted_bytes).to_string();

    // Attempt to decrypt corrupted ciphertext
    let compressed_a = to_compressed_pubkey(&pubkey_a);
    let result = decrypt_v2(&secret_key_b, &compressed_a, &corrupted);

    // Decryption should fail (corrupted MAC or padding)
    assert!(
        result.is_err(),
        "corrupted ciphertext should fail decryption"
    );
}

#[tokio::test]
async fn test_nip44_empty_message_rejected() {
    // Create two key pairs
    let secret_key_a = generate_secret_key();
    let secret_key_b = generate_secret_key();

    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");

    // Empty string should fail (NIP-44 requires min 1 byte plaintext)
    let plaintext = "";
    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let result = encrypt_v2(&secret_key_a, &compressed_b, plaintext);

    assert!(
        result.is_err(),
        "empty message should be rejected per NIP-44 spec"
    );
}

#[tokio::test]
async fn test_nip44_large_message() {
    // Create two key pairs
    let secret_key_a = generate_secret_key();
    let secret_key_b = generate_secret_key();

    let pubkey_a = get_public_key(&secret_key_a).expect("failed to derive pubkey A");
    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");

    // Create max allowed message (NIP-44 max is 65535 bytes)
    let plaintext = "X".repeat(65535);

    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let ciphertext = encrypt_v2(&secret_key_a, &compressed_b, &plaintext)
        .expect("encryption of max length message should succeed");

    // Decrypt
    let compressed_a = to_compressed_pubkey(&pubkey_a);
    let decrypted = decrypt_v2(&secret_key_b, &compressed_a, &ciphertext)
        .expect("decryption of max length message should succeed");

    assert_eq!(
        decrypted, plaintext,
        "max length message should round-trip correctly"
    );
}

#[tokio::test]
async fn test_nip44_oversized_message_rejected() {
    // Create two key pairs
    let secret_key_a = generate_secret_key();
    let secret_key_b = generate_secret_key();

    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");

    // Message exceeding NIP-44 max (65535 bytes) should be rejected
    let plaintext = "X".repeat(65536);

    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let result = encrypt_v2(&secret_key_a, &compressed_b, &plaintext);

    assert!(
        result.is_err(),
        "message > 65535 bytes should be rejected per NIP-44 spec"
    );
}

#[tokio::test]
async fn test_nip44_peer_isolation_over_relay() {
    // This test verifies that NIP-44 encryption properly isolates messages
    // between peers - only the intended recipient can decrypt.

    // 1. Start test relay
    let port = 19010;
    let (_server, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    // 2. Generate 2-of-3 FROST shares
    let shares = generate_key_shares(2, 3).expect("failed to generate shares");

    // 3. Create THREE nodes (A, B, C) where A sends to B, and C shouldn't be able to decrypt
    let secret_key_a: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x0A;
        k
    };
    let secret_key_b: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x0B;
        k
    };
    let secret_key_c: [u8; 32] = {
        let mut k = [0u8; 32];
        k[31] = 0x0C;
        k
    };

    let pubkey_a = get_public_key(&secret_key_a).expect("failed to derive pubkey A");
    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");
    let _pubkey_c = get_public_key(&secret_key_c).expect("failed to derive pubkey C");

    // Node A knows about B only (will send encrypted messages to B)
    let config_a = BifrostConfig {
        default_relays: vec![relay_url.clone()],
        secret_key: Some(secret_key_a),
        peer_pubkeys: vec![pubkey_b], // Only B is a peer
        timeouts: TimeoutConfig {
            sign_timeout_ms: 5000,
            ..Default::default()
        },
        ..Default::default()
    };

    // Node B knows about A (can decrypt messages from A)
    let config_b = BifrostConfig {
        default_relays: vec![relay_url.clone()],
        secret_key: Some(secret_key_b),
        peer_pubkeys: vec![pubkey_a],
        timeouts: TimeoutConfig {
            sign_timeout_ms: 5000,
            ..Default::default()
        },
        ..Default::default()
    };

    // Node C knows about A (might receive relayed events, but can't decrypt)
    let config_c = BifrostConfig {
        default_relays: vec![relay_url],
        secret_key: Some(secret_key_c),
        peer_pubkeys: vec![pubkey_a],
        timeouts: TimeoutConfig {
            sign_timeout_ms: 5000,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut node_a = BifrostNode::with_config(config_a).expect("failed to create node A");
    let mut node_b = BifrostNode::with_config(config_b).expect("failed to create node B");
    let mut node_c = BifrostNode::with_config(config_c).expect("failed to create node C");

    node_a.set_frost_share(shares[0].clone());
    node_b.set_frost_share(shares[1].clone());
    node_c.set_frost_share(shares[2].clone());

    node_a.start().await.expect("failed to start node A");
    node_b.start().await.expect("failed to start node B");
    node_c.start().await.expect("failed to start node C");

    sleep(Duration::from_millis(500)).await;

    // 4. Node A initiates signing with Node B as responder
    // Node C should NOT be able to participate because:
    // - A only sends to B (peer_pubkeys)
    // - Even if C receives the relay event, it can't decrypt (wrong shared secret)

    let event_hash = [0xAB; 32];

    // Run signing between A and B
    let result = with_responder!(run_responder_loop(&node_b), node_a.sign(&event_hash));

    // Signing should succeed between A and B
    let signature = result.expect("signing between A and B should succeed");
    assert_eq!(signature.len(), 64, "signature should be 64 bytes");

    // Node C was running but should NOT have contributed (wasn't contacted)
    // The test passes if A and B can complete signing without involving C,
    // demonstrating that the NIP-44 encryption properly isolates the message to B only.

    // 5. Verify that if we incorrectly try to decrypt with C's keys, it fails
    // This simulates an eavesdropper scenario
    let test_message = "secret signing data";
    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let ciphertext =
        encrypt_v2(&secret_key_a, &compressed_b, test_message).expect("encryption should succeed");

    // C tries to decrypt (pretending the message was for them)
    let compressed_a = to_compressed_pubkey(&pubkey_a);
    let eavesdrop_result = decrypt_v2(&secret_key_c, &compressed_a, &ciphertext);
    assert!(
        eavesdrop_result.is_err(),
        "node C should not be able to decrypt message meant for B"
    );

    // B can decrypt
    let b_decrypt = decrypt_v2(&secret_key_b, &compressed_a, &ciphertext)
        .expect("node B should be able to decrypt");
    assert_eq!(b_decrypt, test_message);

    // 6. Cleanup
    node_a.stop().await.ok();
    node_b.stop().await.ok();
    node_c.stop().await.ok();
}

#[tokio::test]
async fn test_nip44_symmetric_encryption() {
    // NIP-44 uses ECDH to derive a shared secret, so A encrypting for B
    // should produce ciphertext that B can decrypt using A's pubkey.
    // Additionally, B encrypting for A should work symmetrically.

    let secret_key_a = generate_secret_key();
    let secret_key_b = generate_secret_key();

    let pubkey_a = get_public_key(&secret_key_a).expect("failed to derive pubkey A");
    let pubkey_b = get_public_key(&secret_key_b).expect("failed to derive pubkey B");

    let message_from_a = "Message from A to B";
    let message_from_b = "Response from B to A";

    // A encrypts for B
    let compressed_b = to_compressed_pubkey(&pubkey_b);
    let ciphertext_a_to_b =
        encrypt_v2(&secret_key_a, &compressed_b, message_from_a).expect("A should encrypt for B");

    // B decrypts from A
    let compressed_a = to_compressed_pubkey(&pubkey_a);
    let decrypted_by_b = decrypt_v2(&secret_key_b, &compressed_a, &ciphertext_a_to_b)
        .expect("B should decrypt from A");
    assert_eq!(decrypted_by_b, message_from_a);

    // B encrypts for A (reverse direction)
    let ciphertext_b_to_a =
        encrypt_v2(&secret_key_b, &compressed_a, message_from_b).expect("B should encrypt for A");

    // A decrypts from B
    let decrypted_by_a = decrypt_v2(&secret_key_a, &compressed_b, &ciphertext_b_to_a)
        .expect("A should decrypt from B");
    assert_eq!(decrypted_by_a, message_from_b);
}
