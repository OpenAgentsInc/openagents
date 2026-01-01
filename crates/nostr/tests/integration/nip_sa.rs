//! NIP-SA (Sovereign Agents) integration tests
//!
//! These tests verify that NIP-SA event types work correctly and can be used
//! to build events for relay communication.
//!
//! Includes exchange integration tests demonstrating agent-to-agent trading.

use super::{next_test_port, start_test_relay, test_relay_url};
use neobank::{ExchangeClient, OrderParams, OrderSide, TradeOutcome};
use nostr::{
    // NIP-SA Profile types
    AgentProfile,
    AgentProfileContent,
    // NIP-SA Schedule types
    AgentSchedule,
    // NIP-SA State types
    AgentState,
    AgentStateContent,
    AutonomyLevel,
    EventTemplate,
    Goal,
    KIND_AGENT_PROFILE,
    KIND_AGENT_SCHEDULE,
    KIND_AGENT_STATE,
    KIND_TICK_REQUEST,
    KIND_TICK_RESULT,
    KIND_TRAJECTORY_EVENT,
    KIND_TRAJECTORY_SESSION,
    MemoryEntry,
    StepType,
    ThresholdConfig,
    // NIP-SA Tick types
    TickRequest,
    TickResult,
    TickResultContent,
    TickStatus,
    TickTrigger,
    TrajectoryEvent,
    TrajectoryEventContent,
    // NIP-SA Trajectory types
    TrajectorySession,
    TrajectorySessionContent,
    TrajectoryVisibility,
    TriggerType,
    finalize_event,
    generate_secret_key,
    get_public_key,
    get_public_key_hex,
};
use nostr_client::RelayConnection;
use tokio::time::{Duration, sleep, timeout};

fn to_compressed_pubkey(xonly: &[u8; 32]) -> [u8; 33] {
    let mut compressed = [0u8; 33];
    compressed[0] = 0x02;
    compressed[1..].copy_from_slice(xonly);
    compressed
}

#[tokio::test]
async fn test_agent_profile_publish_and_fetch() {
    // 1. Start test relay
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    let relay = RelayConnection::new(&relay_url).unwrap();
    relay.connect().await.unwrap();

    let secret_key = generate_secret_key();
    let pubkey_hex = get_public_key_hex(&secret_key).expect("pubkey hex");

    // 2. Create agent profile content
    let content = AgentProfileContent::new(
        "ResearchBot",
        "I research topics and provide summaries",
        AutonomyLevel::Bounded,
        "1.0.0",
    )
    .with_capabilities(vec!["research".to_string(), "summarization".to_string()]);

    // 3. Create threshold config
    let marketplace_signer = "0".repeat(64); // 32 bytes hex-encoded
    let operator = "1".repeat(64);

    let threshold =
        ThresholdConfig::new(2, 3, &marketplace_signer).expect("valid threshold config");
    let profile = AgentProfile::new(content.clone(), threshold.clone(), &operator)
        .with_lud16("researchbot@getalby.com");

    // 4. Validate profile
    profile.validate().expect("profile should be valid");

    // 5. Build event content and tags
    let content_json = profile
        .content
        .to_json()
        .expect("serialization should work");
    let tags = profile.build_tags();

    let profile_event = finalize_event(
        &EventTemplate {
            kind: KIND_AGENT_PROFILE,
            content: content_json.clone(),
            tags: tags.clone(),
            created_at: 1703000000,
        },
        &secret_key,
    )
    .expect("should sign profile");

    relay
        .publish_event(&profile_event, Duration::from_secs(5))
        .await
        .unwrap();

    let mut profile_rx = relay
        .subscribe_with_channel(
            "profile-fetch",
            &[serde_json::json!({
                "kinds": [KIND_AGENT_PROFILE],
                "authors": [pubkey_hex.clone()],
                "#d": ["profile"],
                "limit": 1
            })],
        )
        .await
        .unwrap();

    let fetched = timeout(Duration::from_secs(2), async { profile_rx.recv().await })
        .await
        .expect("should receive profile event")
        .expect("profile event should be delivered");

    // 6. Verify profile content can be parsed back
    let parsed_content = AgentProfileContent::from_json(&content_json).expect("should parse back");
    assert_eq!(parsed_content.name, "ResearchBot");
    assert_eq!(parsed_content.capabilities.len(), 2);
    assert_eq!(parsed_content.autonomy_level, AutonomyLevel::Bounded);

    // 7. Verify tags are correct
    assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "profile"));
    assert!(
        tags.iter()
            .any(|t| t[0] == "threshold" && t[1] == "2" && t[2] == "3")
    );
    assert!(tags.iter().any(|t| t[0] == "operator"));
    assert!(tags.iter().any(|t| t[0] == "signer"));
    assert!(
        tags.iter()
            .any(|t| t[0] == "lud16" && t[1] == "researchbot@getalby.com")
    );

    assert_eq!(fetched.id, profile_event.id);
    assert_eq!(fetched.pubkey, pubkey_hex);
    assert!(fetched.tags.iter().any(|t| t[0] == "threshold"));
    assert!(
        fetched
            .tags
            .iter()
            .any(|t| t[0] == "operator" && t[1] == operator)
    );

    let fetched_content =
        AgentProfileContent::from_json(&fetched.content).expect("should parse fetched content");
    assert_eq!(fetched_content.name, "ResearchBot");

    // 8. Publish a profile update and verify we can fetch it
    let updated_content = AgentProfileContent::new(
        "ResearchBot v2",
        "Updated profile description",
        AutonomyLevel::Bounded,
        "1.1.0",
    )
    .with_capabilities(vec!["research".to_string(), "summarization".to_string()]);

    let updated_profile = AgentProfile::new(updated_content, threshold, &operator)
        .with_lud16("researchbot@getalby.com");
    let updated_content_json = updated_profile
        .content
        .to_json()
        .expect("updated serialization should work");

    let updated_event = finalize_event(
        &EventTemplate {
            kind: KIND_AGENT_PROFILE,
            content: updated_content_json.clone(),
            tags: updated_profile.build_tags(),
            created_at: 1703000010,
        },
        &secret_key,
    )
    .expect("should sign updated profile");

    relay
        .publish_event(&updated_event, Duration::from_secs(5))
        .await
        .unwrap();

    let mut update_rx = relay
        .subscribe_with_channel(
            "profile-update",
            &[serde_json::json!({
                "kinds": [KIND_AGENT_PROFILE],
                "authors": [pubkey_hex],
                "#d": ["profile"],
                "since": 1703000005
            })],
        )
        .await
        .unwrap();

    let updated = timeout(Duration::from_secs(2), async { update_rx.recv().await })
        .await
        .expect("should receive updated profile")
        .expect("updated profile should be delivered");

    let updated_parsed =
        AgentProfileContent::from_json(&updated.content).expect("should parse updated content");
    assert_eq!(updated_parsed.name, "ResearchBot v2");
    assert_eq!(updated.id, updated_event.id);

    relay.disconnect().await.ok();

    // 9. Verify kind constant
    assert_eq!(KIND_AGENT_PROFILE, 39200);
}

#[tokio::test]
async fn test_agent_schedule_replaceable_semantics() {
    // 1. Start test relay
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let _relay_url = test_relay_url(port);

    // 2. Create schedule configuration
    let schedule = AgentSchedule::new()
        .with_heartbeat(60)
        .expect("valid heartbeat") // 60 second heartbeat
        .add_trigger(TriggerType::Mention)
        .add_trigger(TriggerType::Dm);

    // 3. Verify schedule fields
    assert_eq!(schedule.heartbeat_seconds, Some(60));
    assert_eq!(schedule.triggers.len(), 2);

    // 4. Build tags for the schedule event
    let tags = schedule.build_tags();

    // Verify d tag for replaceable event
    assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "schedule"));

    // Verify heartbeat tag
    assert!(tags.iter().any(|t| t[0] == "heartbeat" && t[1] == "60"));

    // Verify trigger tags
    assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "mention"));
    assert!(tags.iter().any(|t| t[0] == "trigger" && t[1] == "dm"));

    // 5. Verify kind constant
    assert_eq!(KIND_AGENT_SCHEDULE, 39202);
}

#[tokio::test]
async fn test_agent_state_encrypt_publish_fetch_decrypt() {
    // 1. Start test relay
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    let relay = RelayConnection::new(&relay_url).unwrap();
    relay.connect().await.unwrap();

    let agent_secret_key = generate_secret_key();
    let agent_pubkey = get_public_key(&agent_secret_key).expect("agent pubkey");
    let agent_pubkey_hex = get_public_key_hex(&agent_secret_key).expect("agent pubkey hex");
    let agent_pubkey_compressed = to_compressed_pubkey(&agent_pubkey);

    // 2. Build and encrypt state content
    let mut state_content = AgentStateContent::new();
    state_content.add_goal(Goal::new("goal-1", "Persist state", 1));
    state_content.add_memory(MemoryEntry::new("observation", "State initialized"));
    state_content.update_balance(12_345);

    let state = AgentState::new(state_content);
    let encrypted = state
        .encrypt(&agent_secret_key, &agent_pubkey_compressed)
        .expect("state encryption should work");

    let state_event = finalize_event(
        &EventTemplate {
            kind: KIND_AGENT_STATE,
            content: encrypted,
            tags: state.build_tags(),
            created_at: 1703000100,
        },
        &agent_secret_key,
    )
    .expect("should sign state");

    relay
        .publish_event(&state_event, Duration::from_secs(5))
        .await
        .unwrap();

    let mut state_rx = relay
        .subscribe_with_channel(
            "state-fetch",
            &[serde_json::json!({
                "kinds": [KIND_AGENT_STATE],
                "authors": [agent_pubkey_hex],
                "#d": ["state"],
                "limit": 1
            })],
        )
        .await
        .unwrap();

    let fetched = timeout(Duration::from_secs(2), async { state_rx.recv().await })
        .await
        .expect("should receive state event")
        .expect("state event should be delivered");

    let version = fetched
        .tags
        .iter()
        .find(|tag| tag.get(0).map(|t| t.as_str()) == Some("state_version"))
        .and_then(|tag| tag.get(1))
        .and_then(|v| v.parse::<u32>().ok())
        .expect("state_version tag should be present");

    let decrypted = AgentState::decrypt(
        &fetched.content,
        &agent_secret_key,
        &agent_pubkey_compressed,
        version,
    )
    .expect("state decryption should work");

    assert_eq!(decrypted.content.goals.len(), 1);
    assert_eq!(decrypted.content.memory.len(), 1);
    assert_eq!(decrypted.content.wallet_balance_sats, 12_345);

    relay.disconnect().await.ok();
}

#[tokio::test]
async fn test_tick_request_result_flow() {
    // 1. Start test relay
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    let relay = RelayConnection::new(&relay_url).unwrap();
    relay.connect().await.unwrap();

    // 2. Create runner pubkey (hex string)
    let runner_secret_key = generate_secret_key();
    let runner_pubkey = get_public_key_hex(&runner_secret_key).expect("runner pubkey hex");

    let mut tick_rx = relay
        .subscribe_with_channel(
            "tick-sub",
            &[serde_json::json!({
                "kinds": [KIND_TICK_REQUEST, KIND_TICK_RESULT],
                "authors": [runner_pubkey.clone()]
            })],
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(50)).await;

    // 3. Create tick request
    let tick_request = TickRequest::new(&runner_pubkey, TickTrigger::Heartbeat);
    assert_eq!(tick_request.runner, runner_pubkey);
    assert_eq!(tick_request.trigger, TickTrigger::Heartbeat);

    // 4. Build tick request tags
    let request_tags = tick_request.build_tags();

    // Verify runner pubkey tag
    assert!(
        request_tags
            .iter()
            .any(|t| t[0] == "runner" && t[1] == runner_pubkey)
    );

    // Verify trigger tag
    assert!(
        request_tags
            .iter()
            .any(|t| t[0] == "trigger" && t[1] == "heartbeat")
    );

    let request_event = finalize_event(
        &EventTemplate {
            kind: KIND_TICK_REQUEST,
            content: String::new(),
            tags: request_tags.clone(),
            created_at: 1703000200,
        },
        &runner_secret_key,
    )
    .expect("should sign tick request");

    relay
        .publish_event(&request_event, Duration::from_secs(5))
        .await
        .unwrap();

    // 5. Create tick result content
    let content = TickResultContent::new(
        1000, // tokens_in
        500,  // tokens_out
        0.05, // cost_usd
        2,    // goals_updated
    );

    // 6. Create tick result
    let request_id = request_event.id.clone();
    let tick_result = TickResult::new(
        &request_id,
        &runner_pubkey,
        TickStatus::Success,
        1234, // duration_ms
        content,
    );

    assert_eq!(tick_result.status, TickStatus::Success);
    assert_eq!(tick_result.duration_ms, 1234);
    assert_eq!(tick_result.action_count, 0); // No actions in content

    // 7. Build tick result tags
    let result_tags = tick_result.build_tags();

    // Verify request tag
    assert!(
        result_tags
            .iter()
            .any(|t| t[0] == "request" && t[1] == request_id)
    );

    // Verify status tag
    assert!(
        result_tags
            .iter()
            .any(|t| t[0] == "status" && t[1] == "success")
    );

    // Verify duration tag
    assert!(
        result_tags
            .iter()
            .any(|t| t[0] == "duration_ms" && t[1] == "1234")
    );

    let result_event = finalize_event(
        &EventTemplate {
            kind: KIND_TICK_RESULT,
            content: serde_json::to_string(&tick_result.content).expect("result serialization"),
            tags: result_tags.clone(),
            created_at: 1703000201,
        },
        &runner_secret_key,
    )
    .expect("should sign tick result");

    relay
        .publish_event(&result_event, Duration::from_secs(5))
        .await
        .unwrap();

    let mut got_request = None;
    let mut got_result = None;

    timeout(Duration::from_secs(2), async {
        while got_request.is_none() || got_result.is_none() {
            if let Some(evt) = tick_rx.recv().await {
                match evt.kind {
                    KIND_TICK_REQUEST => got_request = Some(evt),
                    KIND_TICK_RESULT => got_result = Some(evt),
                    _ => {}
                }
            }
        }
    })
    .await
    .expect("should receive tick events");

    let received_request = got_request.expect("tick request should arrive");
    let received_result = got_result.expect("tick result should arrive");
    assert_eq!(received_request.id, request_event.id);
    assert_eq!(received_result.id, result_event.id);

    assert!(
        received_result
            .tags
            .iter()
            .any(|t| t[0] == "request" && t[1] == request_id)
    );

    relay.disconnect().await.ok();

    // 9. Verify kind constants
    assert_eq!(KIND_TICK_REQUEST, 39210);
    assert_eq!(KIND_TICK_RESULT, 39211);
}

#[tokio::test]
async fn test_trajectory_session_and_events() {
    // 1. Start test relay
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    let relay = RelayConnection::new(&relay_url).unwrap();
    relay.connect().await.unwrap();

    let agent_secret_key = generate_secret_key();
    let agent_pubkey_hex = get_public_key_hex(&agent_secret_key).expect("agent pubkey hex");

    // 2. Create trajectory session content
    let session_content = TrajectorySessionContent::new(
        "traj_session_456",
        1703000000, // started_at
        "claude-sonnet-4.5",
    )
    .with_total_events(2);

    assert_eq!(session_content.session_id, "traj_session_456");
    assert_eq!(session_content.started_at, 1703000000);
    assert_eq!(session_content.model, "claude-sonnet-4.5");
    assert_eq!(session_content.total_events, 2);

    let session = TrajectorySession::new(
        session_content.clone(),
        "tick-123",
        TrajectoryVisibility::Public,
    );

    let session_event = finalize_event(
        &EventTemplate {
            kind: KIND_TRAJECTORY_SESSION,
            content: session_content
                .to_json()
                .expect("session serialization should work"),
            tags: session.build_tags(),
            created_at: 1703000000,
        },
        &agent_secret_key,
    )
    .expect("should sign trajectory session");

    // 3. Create trajectory event content (tool use)
    let mut tool_use_data = serde_json::Map::new();
    tool_use_data.insert(
        "tool".to_string(),
        serde_json::Value::String("Read".to_string()),
    );
    tool_use_data.insert(
        "input".to_string(),
        serde_json::json!({"file_path": "/path/to/file"}),
    );

    let tool_use_event = TrajectoryEventContent {
        step_type: StepType::ToolUse,
        data: tool_use_data,
    };

    assert_eq!(tool_use_event.step_type, StepType::ToolUse);

    let tool_use = TrajectoryEvent::new(tool_use_event.clone(), "traj_session_456", "tick-123", 1);
    let tool_use_event = finalize_event(
        &EventTemplate {
            kind: KIND_TRAJECTORY_EVENT,
            content: tool_use
                .content
                .to_json()
                .expect("tool use serialization should work"),
            tags: tool_use.build_tags(),
            created_at: 1703000001,
        },
        &agent_secret_key,
    )
    .expect("should sign tool use event");

    // 4. Create trajectory event content (tool result)
    let mut tool_result_data = serde_json::Map::new();
    tool_result_data.insert(
        "tool".to_string(),
        serde_json::Value::String("Read".to_string()),
    );
    tool_result_data.insert("success".to_string(), serde_json::Value::Bool(true));
    tool_result_data.insert(
        "output".to_string(),
        serde_json::Value::String("file contents...".to_string()),
    );

    let tool_result_event = TrajectoryEventContent {
        step_type: StepType::ToolResult,
        data: tool_result_data,
    };

    assert_eq!(tool_result_event.step_type, StepType::ToolResult);

    let tool_result =
        TrajectoryEvent::new(tool_result_event.clone(), "traj_session_456", "tick-123", 2);
    let tool_result_event = finalize_event(
        &EventTemplate {
            kind: KIND_TRAJECTORY_EVENT,
            content: tool_result
                .content
                .to_json()
                .expect("tool result serialization should work"),
            tags: tool_result.build_tags(),
            created_at: 1703000002,
        },
        &agent_secret_key,
    )
    .expect("should sign tool result event");

    relay
        .publish_event(&session_event, Duration::from_secs(5))
        .await
        .unwrap();
    relay
        .publish_event(&tool_use_event, Duration::from_secs(5))
        .await
        .unwrap();
    relay
        .publish_event(&tool_result_event, Duration::from_secs(5))
        .await
        .unwrap();

    let mut trajectory_rx = relay
        .subscribe_with_channel(
            "trajectory-fetch",
            &[serde_json::json!({
                "kinds": [KIND_TRAJECTORY_SESSION, KIND_TRAJECTORY_EVENT],
                "authors": [agent_pubkey_hex]
            })],
        )
        .await
        .unwrap();

    let mut got_session = None;
    let mut got_events = Vec::new();

    timeout(Duration::from_secs(2), async {
        while got_session.is_none() || got_events.len() < 2 {
            if let Some(evt) = trajectory_rx.recv().await {
                if evt.kind == KIND_TRAJECTORY_SESSION {
                    got_session = Some(evt);
                } else if evt.kind == KIND_TRAJECTORY_EVENT {
                    got_events.push(evt);
                }
            }
        }
    })
    .await
    .expect("should receive trajectory events");

    let received_session = got_session.expect("session event should arrive");
    assert_eq!(received_session.id, session_event.id);
    assert!(
        received_session
            .tags
            .iter()
            .any(|t| t[0] == "tick" && t[1] == "tick-123")
    );

    assert_eq!(got_events.len(), 2);
    for evt in &got_events {
        assert!(
            evt.tags
                .iter()
                .any(|t| t[0] == "session" && t[1] == "traj_session_456")
        );
        assert!(
            evt.tags
                .iter()
                .any(|t| t[0] == "tick" && t[1] == "tick-123")
        );
        assert!(evt.tags.iter().any(|t| t[0] == "seq"));
    }

    relay.disconnect().await.ok();

    // 7. Verify kind constants
    assert_eq!(KIND_TRAJECTORY_SESSION, 39230);
    assert_eq!(KIND_TRAJECTORY_EVENT, 39231);
}

#[tokio::test]
async fn test_nip_sa_event_kinds_are_correct() {
    // Verify all NIP-SA event kinds match the specification
    assert_eq!(KIND_AGENT_PROFILE, 39200);
    assert_eq!(KIND_AGENT_SCHEDULE, 39202);
    assert_eq!(KIND_TICK_REQUEST, 39210);
    assert_eq!(KIND_TICK_RESULT, 39211);
    assert_eq!(KIND_TRAJECTORY_SESSION, 39230);
    assert_eq!(KIND_TRAJECTORY_EVENT, 39231);
}

#[tokio::test]
async fn test_autonomy_levels() {
    // Test all autonomy levels serialize correctly
    let levels = vec![
        (AutonomyLevel::Supervised, "supervised"),
        (AutonomyLevel::Bounded, "bounded"),
        (AutonomyLevel::Autonomous, "autonomous"),
    ];

    for (level, expected) in levels {
        let json = serde_json::to_string(&level).expect("serialization should work");
        assert_eq!(json, format!("\"{}\"", expected));

        let parsed: AutonomyLevel = serde_json::from_str(&json).expect("parsing should work");
        assert_eq!(parsed, level);
    }
}

#[tokio::test]
async fn test_tick_triggers() {
    // Test all tick triggers
    let triggers = vec![
        (TickTrigger::Heartbeat, "heartbeat"),
        (TickTrigger::Mention, "mention"),
        (TickTrigger::Dm, "dm"),
        (TickTrigger::Zap, "zap"),
        (TickTrigger::Manual, "manual"),
    ];

    for (trigger, _expected) in triggers {
        let request = TickRequest::new("runner_pubkey", trigger.clone());
        assert_eq!(request.trigger, trigger);
    }
}

#[tokio::test]
async fn test_tick_statuses() {
    // Test all tick statuses
    let statuses = vec![
        (TickStatus::Success, "success"),
        (TickStatus::Failure, "failure"),
        (TickStatus::Timeout, "timeout"),
    ];

    for (status, expected) in statuses {
        let json = serde_json::to_string(&status).expect("serialization should work");
        assert_eq!(json, format!("\"{}\"", expected));

        let parsed: TickStatus = serde_json::from_str(&json).expect("parsing should work");
        assert_eq!(parsed, status);
    }
}

#[tokio::test]
async fn test_step_types() {
    // Test all trajectory step types
    let types = vec![
        StepType::ToolUse,
        StepType::ToolResult,
        StepType::Message,
        StepType::Thinking,
    ];

    for step_type in types {
        let event = TrajectoryEventContent {
            step_type: step_type.clone(),
            data: serde_json::Map::new(),
        };

        let json = serde_json::to_string(&event).expect("serialization should work");
        let parsed: TrajectoryEventContent =
            serde_json::from_str(&json).expect("parsing should work");
        assert_eq!(parsed.step_type, step_type);
    }
}

// ============================================================================
// EXCHANGE INTEGRATION TESTS
// ============================================================================
// These tests demonstrate agent-to-agent trading using NIP-69 orders,
// combined with NIP-SA sovereign agent lifecycle events.

/// NIP-69 P2P Order event kind
const KIND_P2P_ORDER: u16 = 38383;

/// NIP-32 Label event kind (for trade attestations)
const KIND_LABEL: u16 = 1985;

/// Full exchange flow: Two sovereign agents trading BTC for USD
///
/// This test demonstrates:
/// 1. Two sovereign agents with FROSTR-style threshold keys
/// 2. Treasury Agent publishes AgentProfile and sell order (NIP-69)
/// 3. Regular Agent publishes AgentProfile and accepts order
/// 4. Settlement executes (mock mode)
/// 5. Both agents publish attestations (NIP-32 labels)
/// 6. Both agents update AgentState with new balances
/// 7. Both agents publish TrajectoryEvents showing exchange activity
#[tokio::test]
async fn test_sovereign_agent_exchange_flow() {
    // 1. Start test relay
    let port = next_test_port();
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let relay_url = test_relay_url(port);

    let relay = RelayConnection::new(&relay_url).unwrap();
    relay.connect().await.unwrap();

    // 2. Create two sovereign agents
    let treasury_secret = generate_secret_key();
    let treasury_pubkey = get_public_key(&treasury_secret).expect("treasury pubkey");
    let treasury_pubkey_hex = get_public_key_hex(&treasury_secret).expect("treasury pubkey hex");
    let treasury_pubkey_compressed = to_compressed_pubkey(&treasury_pubkey);

    let regular_secret = generate_secret_key();
    let regular_pubkey = get_public_key(&regular_secret).expect("regular pubkey");
    let regular_pubkey_hex = get_public_key_hex(&regular_secret).expect("regular pubkey hex");
    let regular_pubkey_compressed = to_compressed_pubkey(&regular_pubkey);

    println!("\n=== Sovereign Agent Exchange Flow ===");
    println!("Treasury Agent: {}...", &treasury_pubkey_hex[..16]);
    println!("Regular Agent:  {}...", &regular_pubkey_hex[..16]);

    // 3. Publish AgentProfiles for both agents
    let marketplace_signer = "0".repeat(64);

    // Treasury Agent profile
    let treasury_content = AgentProfileContent::new(
        "TreasuryAgent",
        "Manages protocol treasury and provides liquidity",
        AutonomyLevel::Bounded,
        "1.0.0",
    )
    .with_capabilities(vec!["treasury".to_string(), "market-making".to_string()]);

    let treasury_threshold =
        ThresholdConfig::new(2, 3, &marketplace_signer).expect("valid threshold config");
    let treasury_profile = AgentProfile::new(
        treasury_content.clone(),
        treasury_threshold.clone(),
        &treasury_pubkey_hex,
    );

    let treasury_profile_event = finalize_event(
        &EventTemplate {
            kind: KIND_AGENT_PROFILE,
            content: treasury_content.to_json().expect("serialization"),
            tags: treasury_profile.build_tags(),
            created_at: 1703100000,
        },
        &treasury_secret,
    )
    .expect("sign treasury profile");

    relay
        .publish_event(&treasury_profile_event, Duration::from_secs(5))
        .await
        .unwrap();
    println!("\n[1] Treasury Agent profile published");

    // Regular Agent profile
    let regular_content = AgentProfileContent::new(
        "WorkerAgent",
        "Executes tasks and earns sats",
        AutonomyLevel::Bounded,
        "1.0.0",
    )
    .with_capabilities(vec!["compute".to_string(), "research".to_string()]);

    let regular_threshold =
        ThresholdConfig::new(2, 3, &marketplace_signer).expect("valid threshold config");
    let regular_profile = AgentProfile::new(
        regular_content.clone(),
        regular_threshold.clone(),
        &regular_pubkey_hex,
    );

    let regular_profile_event = finalize_event(
        &EventTemplate {
            kind: KIND_AGENT_PROFILE,
            content: regular_content.to_json().expect("serialization"),
            tags: regular_profile.build_tags(),
            created_at: 1703100001,
        },
        &regular_secret,
    )
    .expect("sign regular profile");

    relay
        .publish_event(&regular_profile_event, Duration::from_secs(5))
        .await
        .unwrap();
    println!("[2] Regular Agent profile published");

    // 4. Treasury Agent posts a sell order (NIP-69)
    let treasury_exchange = ExchangeClient::new_mock(&treasury_pubkey_hex);
    let order_params = OrderParams {
        side: OrderSide::Sell,
        amount_sats: 10_000,
        fiat_amount: 100, // $1.00
        currency: "USD".to_string(),
        premium_pct: 0.0,
        payment_methods: vec!["cashu".to_string()],
        expires_in: Duration::from_secs(3600),
    };

    let order_id = treasury_exchange
        .post_order(order_params.clone())
        .await
        .expect("post order");

    // Build NIP-69 tags for the order event
    let order_tags = treasury_exchange.build_order_tags(&order_params);

    let order_event = finalize_event(
        &EventTemplate {
            kind: KIND_P2P_ORDER,
            content: String::new(), // NIP-69 orders use tags, not content
            tags: order_tags.clone(),
            created_at: 1703100010,
        },
        &treasury_secret,
    )
    .expect("sign order");

    relay
        .publish_event(&order_event, Duration::from_secs(5))
        .await
        .unwrap();
    println!(
        "\n[3] Treasury Agent sell order published: {} sats @ $1.00",
        order_params.amount_sats
    );

    // 5. Regular Agent fetches and accepts order
    let regular_exchange = ExchangeClient::new_mock(&regular_pubkey_hex);

    // Simulate relay fetch by injecting order
    let order = treasury_exchange.get_order(&order_id).unwrap().unwrap();
    regular_exchange.inject_order(order).unwrap();

    let orders = regular_exchange
        .fetch_orders(Some(OrderSide::Sell))
        .await
        .expect("fetch orders");

    assert_eq!(orders.len(), 1);
    println!("[4] Regular Agent found 1 sell order");

    let trade = regular_exchange
        .accept_order(&order_id)
        .await
        .expect("accept order");
    println!("[5] Regular Agent accepted order -> trade created");

    // 6. Settlement executes
    let receipt = regular_exchange.settle(&trade).await.expect("settlement");
    println!(
        "[6] Settlement complete: {:?} in {:?}",
        receipt.method, receipt.duration
    );

    // 7. Both agents publish attestations (NIP-32)
    let settlement_ms = receipt.duration.as_millis() as u64;

    // Treasury attestation
    let trade_copy = regular_exchange.get_trade(&order_id).unwrap().unwrap();
    treasury_exchange.inject_trade(trade_copy.clone()).unwrap();

    let treasury_attest_tags =
        treasury_exchange.build_attestation_tags(&trade_copy, TradeOutcome::Success, settlement_ms);

    let treasury_attest_event = finalize_event(
        &EventTemplate {
            kind: KIND_LABEL,
            content: String::new(),
            tags: treasury_attest_tags,
            created_at: 1703100020,
        },
        &treasury_secret,
    )
    .expect("sign treasury attestation");

    relay
        .publish_event(&treasury_attest_event, Duration::from_secs(5))
        .await
        .unwrap();

    // Regular attestation
    let regular_attest_tags =
        regular_exchange.build_attestation_tags(&trade, TradeOutcome::Success, settlement_ms);

    let regular_attest_event = finalize_event(
        &EventTemplate {
            kind: KIND_LABEL,
            content: String::new(),
            tags: regular_attest_tags,
            created_at: 1703100021,
        },
        &regular_secret,
    )
    .expect("sign regular attestation");

    relay
        .publish_event(&regular_attest_event, Duration::from_secs(5))
        .await
        .unwrap();
    println!("[7] Both agents published trade attestations");

    // 8. Both agents update AgentState with new balances
    // Treasury: sold 10k sats, gained $1 USD
    let mut treasury_state_content = AgentStateContent::new();
    treasury_state_content.update_balance(990_000); // Started with 1M, sold 10k
    treasury_state_content.add_memory(MemoryEntry::new(
        "trade",
        &format!(
            "Sold 10000 sats to {} for $1.00 USD",
            &regular_pubkey_hex[..16]
        ),
    ));

    let treasury_state = AgentState::new(treasury_state_content);
    let treasury_state_encrypted = treasury_state
        .encrypt(&treasury_secret, &treasury_pubkey_compressed)
        .expect("encrypt treasury state");

    let treasury_state_event = finalize_event(
        &EventTemplate {
            kind: KIND_AGENT_STATE,
            content: treasury_state_encrypted,
            tags: treasury_state.build_tags(),
            created_at: 1703100030,
        },
        &treasury_secret,
    )
    .expect("sign treasury state");

    relay
        .publish_event(&treasury_state_event, Duration::from_secs(5))
        .await
        .unwrap();

    // Regular: bought 10k sats, spent $1 USD
    let mut regular_state_content = AgentStateContent::new();
    regular_state_content.update_balance(10_000); // Gained 10k sats
    regular_state_content.add_memory(MemoryEntry::new(
        "trade",
        &format!(
            "Bought 10000 sats from {} for $1.00 USD",
            &treasury_pubkey_hex[..16]
        ),
    ));

    let regular_state = AgentState::new(regular_state_content);
    let regular_state_encrypted = regular_state
        .encrypt(&regular_secret, &regular_pubkey_compressed)
        .expect("encrypt regular state");

    let regular_state_event = finalize_event(
        &EventTemplate {
            kind: KIND_AGENT_STATE,
            content: regular_state_encrypted,
            tags: regular_state.build_tags(),
            created_at: 1703100031,
        },
        &regular_secret,
    )
    .expect("sign regular state");

    relay
        .publish_event(&regular_state_event, Duration::from_secs(5))
        .await
        .unwrap();
    println!("[8] Both agents updated state with new balances");

    // 9. Both agents publish trajectory events showing exchange activity
    let session_id = "exchange-session-1";

    // Treasury trajectory: posted order
    let mut treasury_traj_data = serde_json::Map::new();
    treasury_traj_data.insert("action".to_string(), serde_json::json!("post_order"));
    treasury_traj_data.insert("order_id".to_string(), serde_json::json!(order_id));
    treasury_traj_data.insert("side".to_string(), serde_json::json!("sell"));
    treasury_traj_data.insert("amount_sats".to_string(), serde_json::json!(10_000));
    treasury_traj_data.insert("fiat_amount_cents".to_string(), serde_json::json!(100));

    let treasury_traj_content = TrajectoryEventContent {
        step_type: StepType::ToolUse,
        data: treasury_traj_data,
    };

    let treasury_traj =
        TrajectoryEvent::new(treasury_traj_content, session_id, "tick-exchange-1", 1);

    let treasury_traj_event = finalize_event(
        &EventTemplate {
            kind: KIND_TRAJECTORY_EVENT,
            content: treasury_traj.content.to_json().expect("traj serialization"),
            tags: treasury_traj.build_tags(),
            created_at: 1703100040,
        },
        &treasury_secret,
    )
    .expect("sign treasury trajectory");

    relay
        .publish_event(&treasury_traj_event, Duration::from_secs(5))
        .await
        .unwrap();

    // Regular trajectory: accepted order and settled
    let mut regular_traj_data = serde_json::Map::new();
    regular_traj_data.insert("action".to_string(), serde_json::json!("accept_order"));
    regular_traj_data.insert("order_id".to_string(), serde_json::json!(order_id));
    regular_traj_data.insert("settlement_method".to_string(), serde_json::json!("mock"));
    regular_traj_data.insert(
        "settlement_ms".to_string(),
        serde_json::json!(settlement_ms),
    );

    let regular_traj_content = TrajectoryEventContent {
        step_type: StepType::ToolUse,
        data: regular_traj_data,
    };

    let regular_traj = TrajectoryEvent::new(regular_traj_content, session_id, "tick-exchange-1", 2);

    let regular_traj_event = finalize_event(
        &EventTemplate {
            kind: KIND_TRAJECTORY_EVENT,
            content: regular_traj.content.to_json().expect("traj serialization"),
            tags: regular_traj.build_tags(),
            created_at: 1703100041,
        },
        &regular_secret,
    )
    .expect("sign regular trajectory");

    relay
        .publish_event(&regular_traj_event, Duration::from_secs(5))
        .await
        .unwrap();
    println!("[9] Both agents published trajectory events");

    // 10. Verify all events are on the relay
    let mut all_rx = relay
        .subscribe_with_channel(
            "all-events",
            &[serde_json::json!({
                "kinds": [KIND_AGENT_PROFILE, KIND_P2P_ORDER, KIND_LABEL, KIND_AGENT_STATE, KIND_TRAJECTORY_EVENT],
                "authors": [treasury_pubkey_hex.clone(), regular_pubkey_hex.clone()]
            })],
        )
        .await
        .unwrap();

    let mut profile_count = 0;
    let mut order_count = 0;
    let mut label_count = 0;
    let mut state_count = 0;
    let mut traj_count = 0;

    timeout(Duration::from_secs(2), async {
        while profile_count < 2
            || order_count < 1
            || label_count < 2
            || state_count < 2
            || traj_count < 2
        {
            if let Some(evt) = all_rx.recv().await {
                match evt.kind {
                    KIND_AGENT_PROFILE => profile_count += 1,
                    KIND_P2P_ORDER => order_count += 1,
                    KIND_LABEL => label_count += 1,
                    KIND_AGENT_STATE => state_count += 1,
                    KIND_TRAJECTORY_EVENT => traj_count += 1,
                    _ => {}
                }
            }
        }
    })
    .await
    .expect("should receive all events");

    println!("\n[10] Verified events on relay:");
    println!("     - {} AgentProfile events", profile_count);
    println!("     - {} P2P Order events", order_count);
    println!("     - {} Label (attestation) events", label_count);
    println!("     - {} AgentState events", state_count);
    println!("     - {} TrajectoryEvent events", traj_count);

    assert_eq!(profile_count, 2);
    assert_eq!(order_count, 1);
    assert_eq!(label_count, 2);
    assert_eq!(state_count, 2);
    assert_eq!(traj_count, 2);

    relay.disconnect().await.ok();

    println!("\n=== Exchange Flow Complete ===");
    println!("Summary:");
    println!("  - Treasury Agent sold 10,000 sats for $1.00 USD");
    println!("  - Regular Agent bought sats at market rate");
    println!("  - Both agents published attestations");
    println!("  - Both agents updated encrypted state");
    println!("  - Full audit trail on relay via trajectories");
}
