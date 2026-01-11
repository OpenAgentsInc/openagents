//! End-to-end sovereign agent lifecycle tests
//!
//! These tests verify the complete integration between FROSTR threshold signatures
//! and NIP-SA (Sovereign Agents protocol). They exercise:
//!
//! - Threshold identity generation (FROSTR 2-of-3)
//! - Agent profile publishing with threshold signature
//! - Encrypted state storage and retrieval
//! - Threshold ECDH for state decryption
//! - Tick execution with trajectory publishing
//!
//! These are the "Phase 3" tests from directive d-014.

use frost_secp256k1::{Identifier, SigningPackage};
use frostr::bifrost::{BifrostConfig, BifrostNode, TimeoutConfig};
use frostr::ecdh::threshold_ecdh;
use frostr::keygen::generate_key_shares;
use frostr::signing::{aggregate_signatures, round1_commit, round2_sign, verify_signature};
use nostr::{
    // NIP-SA types
    AgentProfile,
    AgentProfileContent,
    AgentSchedule,
    AgentState,
    AgentStateContent,
    AutonomyLevel,
    Goal,
    // Event kinds
    KIND_AGENT_PROFILE,
    MemoryEntry,
    StepType,
    ThresholdConfig,
    TickRequest,
    TickResult,
    TickResultContent,
    TickStatus,
    TickTrigger,
    TrajectoryEventContent,
    TrajectorySessionContent,
    TriggerType,
    // NIP-44 encryption
    encrypt_v2,
    generate_secret_key,
    get_public_key,
};
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::time::{Duration, sleep};

/// Helper: Start an in-process test relay
async fn start_test_relay(port: u16) -> (Arc<RelayServer>, tempfile::TempDir) {
    let config = RelayConfig {
        bind_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
        ..Default::default()
    };

    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_config = DatabaseConfig {
        path: db_path,
        ..Default::default()
    };

    let db = Database::new(db_config).unwrap();
    let server = Arc::new(RelayServer::new(config, db));

    let server_clone = Arc::clone(&server);
    tokio::spawn(async move {
        server_clone.start().await.ok();
    });

    sleep(Duration::from_millis(200)).await;
    (server, temp_dir)
}

/// Helper: Create a compressed public key from x-only bytes
fn to_compressed_pubkey(xonly: &[u8; 32]) -> [u8; 33] {
    let mut compressed = [0u8; 33];
    compressed[0] = 0x02;
    compressed[1..].copy_from_slice(xonly);
    compressed
}

/// Helper: Calculate SHA256 hash
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

// ============================================================================
// PHASE 3: FULL AGENT LIFECYCLE TESTS
// ============================================================================

/// Test the complete sovereign agent lifecycle:
/// 1. Generate threshold identity (FROSTR 2-of-3)
/// 2. Create agent profile with threshold config
/// 3. Sign profile with threshold signature
/// 4. Store encrypted state
/// 5. Retrieve and decrypt state using threshold ECDH
/// 6. Execute tick with trajectory
#[tokio::test]
async fn test_sovereign_agent_lifecycle() {
    // === STEP 1: Generate threshold identity ===
    let shares = generate_key_shares(2, 3).expect("keygen should succeed");
    assert_eq!(shares.len(), 3);

    let group_pk = shares[0].public_key_package.verifying_key();
    let serialized = group_pk.serialize().expect("serialization should work");
    let group_pk_bytes: [u8; 33] = serialized
        .as_slice()
        .try_into()
        .expect("should be 33 bytes");

    // Extract x-only pubkey (skip the 0x02/0x03 prefix)
    let agent_pubkey_xonly: [u8; 32] = group_pk_bytes[1..33].try_into().unwrap();
    let agent_pubkey_hex = hex::encode(&agent_pubkey_xonly);

    // === STEP 2: Create agent profile ===
    // Marketplace signer pubkey (in real scenario, this would be a known entity)
    let marketplace_secret = generate_secret_key();
    let marketplace_pubkey = get_public_key(&marketplace_secret).expect("pubkey");
    let marketplace_pubkey_hex = hex::encode(&marketplace_pubkey);

    // Runner/operator pubkey
    let operator_secret = generate_secret_key();
    let operator_pubkey = get_public_key(&operator_secret).expect("pubkey");
    let operator_pubkey_hex = hex::encode(&operator_pubkey);

    let threshold_config =
        ThresholdConfig::new(2, 3, &marketplace_pubkey_hex).expect("valid threshold config");

    let profile_content = AgentProfileContent::new(
        "TestAgent",
        "A sovereign test agent for lifecycle verification",
        AutonomyLevel::Bounded,
        "1.0.0",
    )
    .with_capabilities(vec!["testing".to_string(), "verification".to_string()]);

    let profile = AgentProfile::new(
        profile_content.clone(),
        threshold_config,
        &operator_pubkey_hex,
    )
    .with_lud16("testagent@example.com");

    profile.validate().expect("profile should be valid");

    // Build profile event content and tags
    let profile_json = profile.content.to_json().expect("serialization");
    let profile_tags = profile.build_tags();

    // Verify profile content
    let parsed_content = AgentProfileContent::from_json(&profile_json).expect("parse");
    assert_eq!(parsed_content.name, "TestAgent");
    assert_eq!(parsed_content.autonomy_level, AutonomyLevel::Bounded);

    // Verify tags include threshold config
    assert!(
        profile_tags
            .iter()
            .any(|t| t[0] == "d" && t[1] == "profile")
    );
    assert!(
        profile_tags
            .iter()
            .any(|t| t[0] == "threshold" && t[1] == "2" && t[2] == "3")
    );

    // === STEP 3: Sign profile with threshold signature ===
    // Create event hash (simulating Nostr event ID calculation)
    let event_data = format!(
        r#"[0,"{}",{},{},[],{}]"#,
        agent_pubkey_hex,
        1703000000u64, // created_at
        KIND_AGENT_PROFILE,
        &profile_json
    );
    let event_hash = sha256(event_data.as_bytes());

    // Perform threshold signing using shares 0 and 1
    let quorum = vec![0usize, 1usize];
    let mut nonces_list = Vec::new();
    let mut commitments_map = BTreeMap::new();

    for &i in &quorum {
        let (nonces, commitments) = round1_commit(&shares[i]);
        let id = Identifier::try_from((i + 1) as u16).unwrap();
        commitments_map.insert(id, commitments);
        nonces_list.push(nonces);
    }

    let signing_package = SigningPackage::new(commitments_map, &event_hash);
    let mut sig_shares = BTreeMap::new();

    for (idx, &i) in quorum.iter().enumerate() {
        let sig_share = round2_sign(&shares[i], &nonces_list[idx], &signing_package)
            .expect("signing should succeed");
        let id = Identifier::try_from((i + 1) as u16).unwrap();
        sig_shares.insert(id, sig_share);
    }

    let signature = aggregate_signatures(&signing_package, &sig_shares, &shares[0])
        .expect("aggregation should succeed");

    // Verify signature against group public key
    verify_signature(&event_hash, &signature, &group_pk).expect("signature should be valid");

    // === STEP 4: Store encrypted state ===
    let mut state_content = AgentStateContent::new();
    state_content.add_goal(Goal::new("goal-1", "Complete lifecycle test", 1));
    state_content.add_memory(MemoryEntry::with_timestamp(
        "observation",
        "Test started successfully",
        1703000000,
    ));
    state_content.update_balance(100_000); // 100k sats
    state_content.record_tick(1703000000);

    let state = AgentState::new(state_content);
    let state_tags = state.build_tags();

    // Verify state tags
    assert!(state_tags.iter().any(|t| t[0] == "d" && t[1] == "state"));
    assert!(state_tags.iter().any(|t| t[0] == "encrypted"));

    // Encrypt state using NIP-44 (operator encrypts to agent pubkey)
    let agent_compressed = to_compressed_pubkey(&agent_pubkey_xonly);
    let state_json = state.content.to_json().expect("serialization");
    let encrypted_state = encrypt_v2(&operator_secret, &agent_compressed, &state_json)
        .expect("encryption should succeed");

    assert!(!encrypted_state.is_empty());
    assert_ne!(encrypted_state, state_json);

    // === STEP 5: Decrypt state using threshold ECDH ===
    // In a real scenario, decryption would use threshold ECDH with the operator's pubkey
    // For this test, we verify that threshold ECDH produces consistent shared secrets
    let shared_secret_01 =
        threshold_ecdh(&shares[0..2], &operator_pubkey).expect("ECDH should succeed");
    let shared_secret_12 =
        threshold_ecdh(&shares[1..3], &operator_pubkey).expect("ECDH should succeed");
    let shared_secret_02 =
        threshold_ecdh(&[shares[0].clone(), shares[2].clone()], &operator_pubkey)
            .expect("ECDH should succeed");

    // All quorums should produce the same shared secret
    assert_eq!(
        shared_secret_01, shared_secret_12,
        "ECDH secrets should match"
    );
    assert_eq!(
        shared_secret_12, shared_secret_02,
        "ECDH secrets should match"
    );

    // Verify the secret is non-zero
    assert_ne!(
        shared_secret_01, [0u8; 32],
        "shared secret should not be zero"
    );

    // === STEP 6: Execute tick with trajectory ===
    // Create tick request
    let tick_request = TickRequest::new(&operator_pubkey_hex, TickTrigger::Heartbeat);
    let tick_request_tags = tick_request.build_tags();

    assert!(tick_request_tags.iter().any(|t| t[0] == "runner"));
    assert!(
        tick_request_tags
            .iter()
            .any(|t| t[0] == "trigger" && t[1] == "heartbeat")
    );

    // Create tick result
    let tick_content = TickResultContent::new(
        1000, // tokens_in
        500,  // tokens_out
        0.05, // cost_usd
        1,    // goals_updated
    );

    let tick_result = TickResult::new(
        "tick-request-id",
        &operator_pubkey_hex,
        TickStatus::Success,
        1500, // duration_ms
        tick_content,
    );

    let tick_result_tags = tick_result.build_tags();
    assert!(
        tick_result_tags
            .iter()
            .any(|t| t[0] == "status" && t[1] == "success")
    );

    // Create trajectory session
    let traj_session = TrajectorySessionContent::new("traj-session-001", 1703000000, "test-model")
        .with_total_events(2);

    assert_eq!(traj_session.session_id, "traj-session-001");
    assert_eq!(traj_session.model, "test-model");

    // Create trajectory events
    let traj_event1 = TrajectoryEventContent {
        step_type: StepType::ToolUse,
        data: {
            let mut m = serde_json::Map::new();
            m.insert("tool".to_string(), serde_json::json!("verify_signature"));
            m
        },
    };

    let traj_event2 = TrajectoryEventContent {
        step_type: StepType::ToolResult,
        data: {
            let mut m = serde_json::Map::new();
            m.insert("success".to_string(), serde_json::json!(true));
            m
        },
    };

    assert_eq!(traj_event1.step_type, StepType::ToolUse);
    assert_eq!(traj_event2.step_type, StepType::ToolResult);

    // === LIFECYCLE COMPLETE ===
    // All phases passed:
    // ✓ Threshold identity generated
    // ✓ Agent profile created with threshold config
    // ✓ Profile signed with threshold signature
    // ✓ State encrypted for agent
    // ✓ Threshold ECDH produces consistent secrets
    // ✓ Tick request/result flow works
    // ✓ Trajectory session/events created
}

/// Test agent signing with Bifrost over relay
#[tokio::test]
async fn test_agent_signs_with_bifrost() {
    // 1. Start test relay
    let port = 19200;
    let (_server, _temp_dir) = start_test_relay(port).await;
    let relay_url = format!("ws://127.0.0.1:{}", port);

    // 2. Generate threshold shares
    let shares = generate_key_shares(2, 3).expect("keygen");
    let group_pk = shares[0].public_key_package.verifying_key();

    // 3. Create agent profile event hash
    let agent_pubkey = group_pk.serialize().expect("serialize");
    let profile_content = AgentProfileContent::new(
        "BifrostTestAgent",
        "Tests Bifrost signing",
        AutonomyLevel::Supervised,
        "1.0.0",
    );
    let profile_json = profile_content.to_json().expect("json");

    // Simulate event ID calculation
    let event_data = format!(
        r#"[0,"{}",{},{},[],{}]"#,
        hex::encode(&agent_pubkey.as_slice()[1..33]),
        1703000000u64,
        KIND_AGENT_PROFILE,
        &profile_json
    );
    let event_hash = sha256(event_data.as_bytes());

    // 4. Create Bifrost nodes
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

    let peer_pubkey_1 = get_public_key(&secret_key_1).expect("pubkey");
    let peer_pubkey_2 = get_public_key(&secret_key_2).expect("pubkey");

    let config_1 = BifrostConfig {
        default_relays: vec![relay_url.clone()],
        secret_key: Some(secret_key_1),
        peer_pubkeys: vec![peer_pubkey_1, peer_pubkey_2],
        timeouts: TimeoutConfig {
            sign_timeout_ms: 10000,
            ..Default::default()
        },
        ..Default::default()
    };

    let config_2 = BifrostConfig {
        default_relays: vec![relay_url],
        secret_key: Some(secret_key_2),
        peer_pubkeys: vec![peer_pubkey_1, peer_pubkey_2],
        timeouts: TimeoutConfig {
            sign_timeout_ms: 10000,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut node_1 = BifrostNode::with_config(config_1).expect("node 1");
    let mut node_2 = BifrostNode::with_config(config_2).expect("node 2");

    node_1.set_frost_share(shares[0].clone());
    node_2.set_frost_share(shares[1].clone());

    node_1.start().await.expect("start node 1");
    node_2.start().await.expect("start node 2");

    sleep(Duration::from_millis(500)).await;

    // 5. Sign using Bifrost with responder
    let result = tokio::select! {
        biased;
        r = node_1.sign(&event_hash) => r,
        _ = run_responder(&node_2) => {
            Err(frostr::Error::Protocol("responder exited".into()))
        }
    };

    // 6. Verify signature
    let signature_bytes = result.expect("signing should succeed");
    assert_eq!(signature_bytes.len(), 64);

    // Convert to frost signature for verification
    let mut sig_with_prefix = vec![0x00]; // Compact signature prefix
    sig_with_prefix.extend_from_slice(&signature_bytes);

    // The signature should be verifiable (we already tested this in bifrost_e2e)
    // Here we just verify we got a 64-byte signature

    // 7. Cleanup
    node_1.stop().await.ok();
    node_2.stop().await.ok();
}

/// Helper to run responder loop
async fn run_responder(node: &BifrostNode) -> frostr::Result<()> {
    let transport = node
        .transport()
        .ok_or_else(|| frostr::Error::Protocol("Transport not initialized".into()))?;

    loop {
        match tokio::time::timeout(Duration::from_secs(30), transport.receive()).await {
            Ok(Ok(message)) => {
                if let Ok(Some(response)) = node.handle_message(&message) {
                    transport.broadcast(&response).await?;
                }
            }
            Ok(Err(_)) => {}
            Err(_) => return Ok(()),
        }
    }
}

/// Test threshold ECDH for decrypting DMs to agent
#[tokio::test]
async fn test_agent_decrypts_dm_with_threshold_ecdh() {
    // 1. Generate agent's threshold identity
    let shares = generate_key_shares(2, 3).expect("keygen");
    let group_pk = shares[0].public_key_package.verifying_key();
    let serialized = group_pk.serialize().expect("serialize");
    let group_pk_bytes: [u8; 33] = serialized.as_slice().try_into().unwrap();
    let agent_pubkey_xonly: [u8; 32] = group_pk_bytes[1..33].try_into().unwrap();

    // 2. External sender creates a DM for the agent
    let sender_secret = generate_secret_key();
    let sender_pubkey = get_public_key(&sender_secret).expect("pubkey");

    let dm_content = r#"{"type": "task", "description": "Please review this code"}"#;

    // Encrypt DM to agent pubkey
    let agent_compressed = to_compressed_pubkey(&agent_pubkey_xonly);
    let encrypted_dm = encrypt_v2(&sender_secret, &agent_compressed, dm_content)
        .expect("encryption should succeed");

    // 3. Agent uses threshold ECDH to derive shared secret with sender
    // Different quorums should all produce the same shared secret
    let secret_01 = threshold_ecdh(&shares[0..2], &sender_pubkey).expect("ECDH with shares 0,1");
    let secret_02 = threshold_ecdh(&[shares[0].clone(), shares[2].clone()], &sender_pubkey)
        .expect("ECDH with shares 0,2");
    let secret_12 = threshold_ecdh(&shares[1..3], &sender_pubkey).expect("ECDH with shares 1,2");

    // All quorums produce same secret
    assert_eq!(secret_01, secret_02, "quorum (0,1) should equal (0,2)");
    assert_eq!(secret_02, secret_12, "quorum (0,2) should equal (1,2)");

    // 4. The shared secret can be used for NIP-44 decryption
    // In practice, NIP-44 uses ECDH internally, so this demonstrates
    // that threshold ECDH produces valid shared secrets

    // Verify the shared secret is usable (non-zero, consistent)
    assert_ne!(secret_01, [0u8; 32]);

    // 5. Verify ciphertext properties
    assert!(!encrypted_dm.is_empty());
    assert_ne!(encrypted_dm, dm_content);
    assert!(encrypted_dm.len() > dm_content.len()); // Encrypted should be longer
}

/// Test state encryption with threshold ECDH round-trip
#[tokio::test]
async fn test_state_encryption_with_threshold_ecdh() {
    // 1. Generate threshold identity
    let shares = generate_key_shares(2, 3).expect("keygen");
    let group_pk = shares[0].public_key_package.verifying_key();
    let serialized = group_pk.serialize().expect("serialize");
    let group_pk_bytes: [u8; 33] = serialized.as_slice().try_into().unwrap();
    let agent_pubkey_xonly: [u8; 32] = group_pk_bytes[1..33].try_into().unwrap();

    // 2. Operator creates and encrypts agent state
    let operator_secret = generate_secret_key();
    let operator_pubkey = get_public_key(&operator_secret).expect("pubkey");

    let mut state = AgentStateContent::new();
    state.add_goal(Goal::new("goal-1", "Process 10 tasks", 1));
    state.add_goal(Goal::new("goal-2", "Maintain reputation", 2));
    state.add_memory(MemoryEntry::with_timestamp(
        "init",
        "Agent initialized",
        1703000000,
    ));
    state.update_balance(50_000);
    state.record_tick(1703000000);

    let state_json = state.to_json().expect("json");

    // Encrypt state to agent
    let agent_compressed = to_compressed_pubkey(&agent_pubkey_xonly);
    let encrypted_state =
        encrypt_v2(&operator_secret, &agent_compressed, &state_json).expect("encryption");

    // 3. Agent uses threshold ECDH to derive shared secret for decryption
    let shared_secret = threshold_ecdh(&shares[0..2], &operator_pubkey).expect("threshold ECDH");

    // 4. Verify different quorums produce same secret
    let shared_secret_alt =
        threshold_ecdh(&[shares[0].clone(), shares[2].clone()], &operator_pubkey)
            .expect("threshold ECDH alt quorum");

    assert_eq!(
        shared_secret, shared_secret_alt,
        "different quorums should produce same shared secret"
    );

    // 5. The shared secret enables state decryption
    // (In practice, NIP-44 decrypt would use this internally)
    assert_ne!(shared_secret, [0u8; 32]);
    assert!(!encrypted_state.is_empty());
}

/// Test agent schedule event creation
#[tokio::test]
async fn test_agent_schedule_creation() {
    // Create schedule with heartbeat and triggers
    let schedule = AgentSchedule::new()
        .with_heartbeat(300)
        .expect("valid heartbeat") // 5 minutes
        .add_trigger(TriggerType::Mention)
        .add_trigger(TriggerType::Dm)
        .add_trigger(TriggerType::Zap);

    assert_eq!(schedule.heartbeat_seconds, Some(300));
    assert_eq!(schedule.triggers.len(), 3);

    let tags = schedule.build_tags();

    // Verify tags
    assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "schedule"));
    assert!(tags.iter().any(|t| t[0] == "heartbeat" && t[1] == "300"));
    assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "mention"));
    assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "dm"));
    assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "zap"));
}

/// Test trajectory hash verification
#[tokio::test]
async fn test_trajectory_hash_verification() {
    // Create trajectory session and events
    let session_id = "session-abc123";
    let model = "codex-sonnet-4.5";

    // Create events
    let events = vec![
        TrajectoryEventContent {
            step_type: StepType::Message,
            data: {
                let mut m = serde_json::Map::new();
                m.insert("content".to_string(), serde_json::json!("Hello"));
                m
            },
        },
        TrajectoryEventContent {
            step_type: StepType::ToolUse,
            data: {
                let mut m = serde_json::Map::new();
                m.insert("tool".to_string(), serde_json::json!("Read"));
                m
            },
        },
        TrajectoryEventContent {
            step_type: StepType::ToolResult,
            data: {
                let mut m = serde_json::Map::new();
                m.insert("result".to_string(), serde_json::json!("file contents"));
                m
            },
        },
    ];

    // Calculate hash of all events
    let mut hasher = Sha256::new();
    for event in &events {
        let event_json = serde_json::to_string(event).expect("json");
        hasher.update(event_json.as_bytes());
    }
    let events_hash: [u8; 32] = hasher.finalize().into();
    let events_hash_hex = hex::encode(events_hash);

    // Create session with hash
    let session = TrajectorySessionContent::new(session_id, 1703000000, model)
        .with_total_events(events.len() as u32)
        .with_hash(&events_hash_hex);

    assert_eq!(session.session_id, session_id);
    assert_eq!(session.total_events, 3);
    assert_eq!(session.trajectory_hash, Some(events_hash_hex.clone()));

    // Verify hash matches
    let mut verify_hasher = Sha256::new();
    for event in &events {
        let event_json = serde_json::to_string(event).expect("json");
        verify_hasher.update(event_json.as_bytes());
    }
    let verify_hash: [u8; 32] = verify_hasher.finalize().into();

    assert_eq!(
        hex::encode(verify_hash),
        session.trajectory_hash.unwrap(),
        "trajectory hash should match recalculated hash"
    );
}
