//! End-to-end integration tests for Trajectory contribution system
//!
//! Tests trajectory collection, redaction, quality validation, and contribution to relay.
//! Part of d-015: Comprehensive Marketplace and Agent Commerce E2E Tests

use chrono::Utc;
use marketplace::trajectories::{TrajectoryConfig, TrajectorySession};
use nostr::{EventTemplate, finalize_event, generate_secret_key, get_public_key};
use nostr::{
    KIND_TRAJECTORY_EVENT, KIND_TRAJECTORY_SESSION, StepType, TrajectoryEventContent,
    TrajectorySessionContent, TrajectoryVisibility,
};
use nostr_client::RelayConnection;
use nostr_relay::{Database, DatabaseConfig, RelayConfig, RelayServer};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

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

fn test_relay_url(port: u16) -> String {
    format!("ws://127.0.0.1:{}", port)
}

#[allow(dead_code)]
fn sha256_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

fn create_test_session() -> TrajectorySession {
    TrajectorySession {
        session_id: "sess-test-001".to_string(),
        source: "claude".to_string(),
        path: PathBuf::from("/tmp/test_session.rlog"),
        initial_commit: Some("abc123".to_string()),
        final_commit: Some("def456".to_string()),
        ci_passed: Some(true),
        started_at: Utc::now() - chrono::Duration::hours(1),
        ended_at: Some(Utc::now()),
        token_count: 15000,
        tool_calls: 42,
        quality_score: 0.85,
    }
}

// =============================================================================
// Phase 4 Tests: Trajectory E2E with Real Relay
// =============================================================================

#[tokio::test]
async fn test_trajectory_session_publish_to_relay() {
    // Test: Publish trajectory session to relay with NIP-SA events
    //
    // 1. Create trajectory session content
    // 2. Publish to relay (kind:38030)
    // 3. Consumer can discover and verify session

    let (_server, _tmp) = start_test_relay(19230).await;
    let relay_url = test_relay_url(19230);

    let contributor_secret_key = generate_secret_key();
    let _contributor_pubkey = get_public_key(&contributor_secret_key).expect("pubkey");

    // Create trajectory session content (NIP-SA kind:38030)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let session_content =
        TrajectorySessionContent::new("session-test-123", now - 3600, "claude-sonnet-4.5")
            .with_end_time(now)
            .with_total_events(42);

    // Create NIP-SA trajectory session
    let session = nostr::TrajectorySession::new(
        session_content.clone(),
        "tick-001",
        TrajectoryVisibility::Public,
    );

    let session_json = session_content.to_json().expect("serialize");
    let session_tags = session.build_tags();

    let session_template = EventTemplate {
        kind: KIND_TRAJECTORY_SESSION,
        content: session_json,
        tags: session_tags,
        created_at: now,
    };

    let session_event = finalize_event(&session_template, &contributor_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [KIND_TRAJECTORY_SESSION],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("trajectory-sessions", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    let confirmation = relay
        .publish_event(&session_event, Duration::from_secs(5))
        .await
        .expect("publish");

    assert!(confirmation.accepted);

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive")
        .expect("should have session");

    assert_eq!(received.id, session_event.id);
    assert_eq!(received.kind, KIND_TRAJECTORY_SESSION);

    let parsed: TrajectorySessionContent =
        TrajectorySessionContent::from_json(&received.content).expect("parse");
    assert_eq!(parsed.session_id, "session-test-123");
    assert_eq!(parsed.model, "claude-sonnet-4.5");

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_trajectory_events_publish_to_relay() {
    // Test: Publish trajectory events to relay (kind:38031)
    //
    // 1. Publish multiple trajectory events (tool use, tool result, thinking)
    // 2. Consumer can discover events by session

    let (_server, _tmp) = start_test_relay(19231).await;
    let relay_url = test_relay_url(19231);

    let contributor_secret_key = generate_secret_key();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create multiple trajectory events
    let events_data = vec![
        (
            StepType::ToolUse,
            json!({"tool": "Read", "path": "/src/main.rs"}),
        ),
        (
            StepType::ToolResult,
            json!({"success": true, "output": "file contents..."}),
        ),
        (
            StepType::Thinking,
            json!({"thought": "I need to analyze the code"}),
        ),
        (
            StepType::Message,
            json!({"message": "Here's the analysis..."}),
        ),
    ];

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [KIND_TRAJECTORY_EVENT],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("trajectory-events", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    let mut published_ids = Vec::new();

    for (seq, (step_type, data)) in events_data.iter().enumerate() {
        let mut content = TrajectoryEventContent::new(step_type.clone());
        for (key, value) in data.as_object().unwrap() {
            content = content.with_data(key, value.clone());
        }

        let event =
            nostr::TrajectoryEvent::new(content.clone(), "session-123", "tick-001", seq as u32);

        let event_json = content.to_json().expect("serialize");
        let event_tags = event.build_tags();

        let event_template = EventTemplate {
            kind: KIND_TRAJECTORY_EVENT,
            content: event_json,
            tags: event_tags,
            created_at: now + seq as u64,
        };

        let nostr_event = finalize_event(&event_template, &contributor_secret_key).expect("sign");
        published_ids.push(nostr_event.id.clone());

        relay
            .publish_event(&nostr_event, Duration::from_secs(5))
            .await
            .expect("publish");
    }

    // Receive all events
    let mut received_events = Vec::new();
    for _ in 0..4 {
        match tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
            Ok(Some(event)) => received_events.push(event),
            _ => break,
        }
    }

    assert_eq!(received_events.len(), 4, "Should receive all 4 events");

    // Verify all have correct kind and session reference
    for event in &received_events {
        assert_eq!(event.kind, KIND_TRAJECTORY_EVENT);
        let has_session_ref = event
            .tags
            .iter()
            .any(|t| t[0] == "session" && t[1] == "session-123");
        assert!(has_session_ref, "Event should reference session");
    }

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_trajectory_with_hash_verification() {
    // Test: Trajectory session with hash verification
    //
    // 1. Create events and calculate hash
    // 2. Include hash in session
    // 3. Verify hash matches published events

    let (_server, _tmp) = start_test_relay(19232).await;
    let relay_url = test_relay_url(19232);

    let contributor_secret_key = generate_secret_key();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Create events and calculate hash
    let events_json = vec![
        json!({"step_type": "tool_use", "tool": "Read"}),
        json!({"step_type": "tool_result", "success": true}),
    ];

    let events_str: Vec<String> = events_json.iter().map(|e| e.to_string()).collect();
    let trajectory_hash =
        TrajectorySessionContent::calculate_hash(&events_str).expect("calculate hash");

    // Create session with hash
    let session_content =
        TrajectorySessionContent::new("session-hash-test", now - 1800, "claude-sonnet-4.5")
            .with_end_time(now)
            .with_total_events(2)
            .with_hash(trajectory_hash.clone());

    let session = nostr::TrajectorySession::new(
        session_content.clone(),
        "tick-hash-001",
        TrajectoryVisibility::Public,
    );

    let session_json = session_content.to_json().expect("serialize");
    let session_tags = session.build_tags();

    let session_template = EventTemplate {
        kind: KIND_TRAJECTORY_SESSION,
        content: session_json,
        tags: session_tags,
        created_at: now,
    };

    let session_event = finalize_event(&session_template, &contributor_secret_key).expect("sign");

    let relay = RelayConnection::new(&relay_url).expect("connection");
    relay.connect().await.expect("connect");

    let filter = json!({
        "kinds": [KIND_TRAJECTORY_SESSION],
        "limit": 10
    });
    let mut rx = relay
        .subscribe_with_channel("trajectory-sessions", &[filter])
        .await
        .expect("subscribe");

    sleep(Duration::from_millis(100)).await;

    relay
        .publish_event(&session_event, Duration::from_secs(5))
        .await
        .expect("publish");

    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("should receive")
        .expect("should have session");

    let parsed: TrajectorySessionContent =
        TrajectorySessionContent::from_json(&received.content).expect("parse");

    assert!(parsed.trajectory_hash.is_some(), "Session should have hash");
    assert_eq!(
        parsed.trajectory_hash.unwrap(),
        trajectory_hash,
        "Hash should match"
    );

    // Verify we can reproduce the hash
    let verified_hash = TrajectorySessionContent::calculate_hash(&events_str).unwrap();
    assert_eq!(
        verified_hash, trajectory_hash,
        "Hash should be reproducible"
    );

    relay.disconnect().await.ok();
}

// =============================================================================
// Configuration Tests
// =============================================================================

#[test]
fn test_trajectory_config_defaults() {
    let config = TrajectoryConfig::default();

    assert!(!config.sources.is_empty(), "Should have default sources");
    assert!(
        !config.auto_contribute,
        "Auto-contribute should be off by default"
    );
    assert!(
        config.min_quality_score > 0.0,
        "Should have minimum threshold"
    );
}

#[test]
fn test_trajectory_session_creation() {
    let session = create_test_session();

    assert!(!session.session_id.is_empty(), "Should have session ID");
    assert!(
        session.initial_commit.is_some(),
        "Should have initial commit"
    );
    assert!(session.final_commit.is_some(), "Should have final commit");
    assert!(session.ci_passed.is_some(), "Should have CI result");
    assert!(session.token_count > 0, "Should have tokens");
    assert!(session.tool_calls > 0, "Should have tool calls");
}

#[test]
fn test_step_types() {
    // Verify all step types can be created
    let _tool_use = StepType::ToolUse;
    let _tool_result = StepType::ToolResult;
    let _thinking = StepType::Thinking;
    let _message = StepType::Message;
}

#[test]
fn test_trajectory_visibility() {
    // Verify visibility options
    let _public = TrajectoryVisibility::Public;
    let _private = TrajectoryVisibility::Private;
}
