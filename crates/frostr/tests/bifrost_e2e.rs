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

use frostr::bifrost::{BifrostConfig, BifrostNode, TimeoutConfig};
use frostr::ecdh::threshold_ecdh;
use frostr::keygen::generate_key_shares;
use nostr::generate_secret_key;
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

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
    let transport = node.transport().ok_or_else(|| {
        frostr::Error::Protocol("Transport not initialized".into())
    })?;

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
    let _group_pk = shares[0].public_key_package.verifying_key();

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
    let peer_pubkey_1 = nostr::get_public_key(&secret_key_1)
        .expect("failed to derive pubkey 1");
    let peer_pubkey_2 = nostr::get_public_key(&secret_key_2)
        .expect("failed to derive pubkey 2");

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
    let result = with_responder!(
        run_responder_loop(&node_2),
        node_1.sign(&event_hash)
    );

    // Signing should succeed
    let signature = result.expect("signing should succeed");
    assert_eq!(signature.len(), 64, "signature should be 64 bytes");

    // TODO: Verify signature against group public key when signature format is finalized
    // The signature is in BIP-340 format (R.x || s)
    // frostr::signing::verify_signature requires frost_secp256k1::Signature which is different

    // 5. Clean up
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
    let peer_pubkey_bytes = nostr::get_public_key(&peer_secret_key)
        .expect("should derive public key from secret key");

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

    let peer_pubkey_1 = nostr::get_public_key(&secret_key_1)
        .expect("failed to derive pubkey 1");
    let peer_pubkey_2 = nostr::get_public_key(&secret_key_2)
        .expect("failed to derive pubkey 2");

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
    let result = with_responder!(
        run_responder_loop(&node_2),
        node_1.ecdh(&peer_pubkey_bytes)
    );

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
        let peer_pubkeys: Vec<[u8; 32]> = (0..3)
            .filter(|&j| j != i)
            .map(|j| pubkeys[j])
            .collect();

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
    }.await;

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

    use frostr::signing::{round1_commit, round2_sign, aggregate_signatures, verify_signature};
    use frost_secp256k1::SigningPackage;
    use std::collections::BTreeMap;

    // Test all possible 2-of-3 quorums: (0,1), (0,2), (1,2)
    let quorums: Vec<Vec<usize>> = vec![
        vec![0, 1],
        vec![0, 2],
        vec![1, 2],
    ];

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
        verify_signature(&event_hash, &signature, &group_pk)
            .expect(&format!("signature from quorum {:?} should be valid against group public key", quorum));
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
    let fake_peer_pubkey = nostr::get_public_key(&fake_peer_secret)
        .expect("failed to derive fake peer pubkey");

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
        0x79, 0xBE, 0x66, 0x7E, 0xF9, 0xDC, 0xBB, 0xAC,
        0x55, 0xA0, 0x62, 0x95, 0xCE, 0x87, 0x0B, 0x07,
        0x02, 0x9B, 0xFC, 0xDB, 0x2D, 0xCE, 0x28, 0xD9,
        0x59, 0xF2, 0x81, 0x5B, 0x16, 0xF8, 0x17, 0x98,
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
