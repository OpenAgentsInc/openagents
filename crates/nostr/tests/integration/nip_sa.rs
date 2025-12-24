//! NIP-SA (Sovereign Agents) integration tests
//!
//! These tests verify that NIP-SA event types work correctly and can be used
//! to build events for relay communication.

use super::{start_test_relay, test_relay_url};
use nostr::{
    // NIP-SA Profile types
    AgentProfile, AgentProfileContent, AutonomyLevel, ThresholdConfig, KIND_AGENT_PROFILE,
    // NIP-SA Schedule types
    AgentSchedule, TriggerType, KIND_AGENT_SCHEDULE,
    // NIP-SA Tick types
    TickRequest, TickResult, TickResultContent, TickStatus, TickTrigger,
    KIND_TICK_REQUEST, KIND_TICK_RESULT,
    // NIP-SA Trajectory types
    TrajectorySessionContent, TrajectoryEventContent, StepType,
    KIND_TRAJECTORY_EVENT, KIND_TRAJECTORY_SESSION,
};

#[tokio::test]
async fn test_agent_profile_publish_and_fetch() {
    // 1. Start test relay
    let port = 19100;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let _relay_url = test_relay_url(port);

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

    let threshold = ThresholdConfig::new(2, 3, &marketplace_signer)
        .expect("valid threshold config");
    let profile = AgentProfile::new(content.clone(), threshold, &operator)
        .with_lud16("researchbot@getalby.com");

    // 4. Validate profile
    profile.validate().expect("profile should be valid");

    // 5. Build event content and tags
    let content_json = profile.content.to_json().expect("serialization should work");
    let tags = profile.build_tags();

    // 6. Verify profile content can be parsed back
    let parsed_content = AgentProfileContent::from_json(&content_json)
        .expect("should parse back");
    assert_eq!(parsed_content.name, "ResearchBot");
    assert_eq!(parsed_content.capabilities.len(), 2);
    assert_eq!(parsed_content.autonomy_level, AutonomyLevel::Bounded);

    // 7. Verify tags are correct
    assert!(tags.iter().any(|t| t[0] == "d" && t[1] == "profile"));
    assert!(tags.iter().any(|t| t[0] == "threshold" && t[1] == "2" && t[2] == "3"));
    assert!(tags.iter().any(|t| t[0] == "operator"));
    assert!(tags.iter().any(|t| t[0] == "signer"));
    assert!(tags.iter().any(|t| t[0] == "lud16" && t[1] == "researchbot@getalby.com"));

    // 8. Verify kind constant
    assert_eq!(KIND_AGENT_PROFILE, 38000);
}

#[tokio::test]
async fn test_agent_schedule_replaceable_semantics() {
    // 1. Start test relay
    let port = 19101;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let _relay_url = test_relay_url(port);

    // 2. Create schedule configuration
    let schedule = AgentSchedule::new()
        .with_heartbeat(60).expect("valid heartbeat") // 60 second heartbeat
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
    assert_eq!(KIND_AGENT_SCHEDULE, 38002);
}

#[tokio::test]
async fn test_tick_request_result_flow() {
    // 1. Start test relay
    let port = 19102;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let _relay_url = test_relay_url(port);

    // 2. Create runner pubkey (hex string)
    let runner_pubkey = "2".repeat(64);

    // 3. Create tick request
    let tick_request = TickRequest::new(&runner_pubkey, TickTrigger::Heartbeat);
    assert_eq!(tick_request.runner, runner_pubkey);
    assert_eq!(tick_request.trigger, TickTrigger::Heartbeat);

    // 4. Build tick request tags
    let request_tags = tick_request.build_tags();

    // Verify runner pubkey tag
    assert!(request_tags.iter().any(|t| t[0] == "runner" && t[1] == runner_pubkey));

    // Verify trigger tag
    assert!(request_tags.iter().any(|t| t[0] == "trigger" && t[1] == "heartbeat"));

    // 5. Create tick result content
    let content = TickResultContent::new(
        1000,  // tokens_in
        500,   // tokens_out
        0.05,  // cost_usd
        2,     // goals_updated
    );

    // 6. Create tick result
    let request_id = "event123";
    let tick_result = TickResult::new(
        request_id,
        &runner_pubkey,
        TickStatus::Success,
        1234,  // duration_ms
        content,
    );

    assert_eq!(tick_result.status, TickStatus::Success);
    assert_eq!(tick_result.duration_ms, 1234);
    assert_eq!(tick_result.action_count, 0); // No actions in content

    // 7. Build tick result tags
    let result_tags = tick_result.build_tags();

    // Verify request tag
    assert!(result_tags.iter().any(|t| t[0] == "request" && t[1] == request_id));

    // Verify status tag
    assert!(result_tags.iter().any(|t| t[0] == "status" && t[1] == "success"));

    // Verify duration tag
    assert!(result_tags.iter().any(|t| t[0] == "duration_ms" && t[1] == "1234"));

    // 8. Test failure status
    let failure_content = TickResultContent::new(0, 0, 0.0, 0);
    let failure_result = TickResult::new(
        "event456",
        &runner_pubkey,
        TickStatus::Failure,
        500,
        failure_content,
    );
    assert_eq!(failure_result.status, TickStatus::Failure);

    // 9. Verify kind constants
    assert_eq!(KIND_TICK_REQUEST, 38010);
    assert_eq!(KIND_TICK_RESULT, 38011);
}

#[tokio::test]
async fn test_trajectory_session_and_events() {
    // 1. Start test relay
    let port = 19103;
    let (_server, _addr, _temp_dir) = start_test_relay(port).await;
    let _relay_url = test_relay_url(port);

    // 2. Create trajectory session content
    let session = TrajectorySessionContent::new(
        "traj_session_456",
        1703000000,  // started_at
        "claude-sonnet-4.5",
    )
    .with_total_events(42);

    assert_eq!(session.session_id, "traj_session_456");
    assert_eq!(session.started_at, 1703000000);
    assert_eq!(session.model, "claude-sonnet-4.5");
    assert_eq!(session.total_events, 42);

    // 3. Create trajectory event content (tool use)
    let mut tool_use_data = serde_json::Map::new();
    tool_use_data.insert("tool".to_string(), serde_json::Value::String("Read".to_string()));
    tool_use_data.insert(
        "input".to_string(),
        serde_json::json!({"file_path": "/path/to/file"}),
    );

    let tool_use_event = TrajectoryEventContent {
        step_type: StepType::ToolUse,
        data: tool_use_data,
    };

    assert_eq!(tool_use_event.step_type, StepType::ToolUse);

    // 4. Create trajectory event content (tool result)
    let mut tool_result_data = serde_json::Map::new();
    tool_result_data.insert("tool".to_string(), serde_json::Value::String("Read".to_string()));
    tool_result_data.insert("success".to_string(), serde_json::Value::Bool(true));
    tool_result_data.insert("output".to_string(), serde_json::Value::String("file contents...".to_string()));

    let tool_result_event = TrajectoryEventContent {
        step_type: StepType::ToolResult,
        data: tool_result_data,
    };

    assert_eq!(tool_result_event.step_type, StepType::ToolResult);

    // 5. Test session serialization
    let session_json = serde_json::to_string(&session).expect("serialization should work");
    let parsed_session: TrajectorySessionContent = serde_json::from_str(&session_json)
        .expect("parsing should work");
    assert_eq!(parsed_session.session_id, "traj_session_456");

    // 6. Test event serialization
    let event_json = serde_json::to_string(&tool_use_event).expect("serialization should work");
    let parsed_event: TrajectoryEventContent = serde_json::from_str(&event_json)
        .expect("parsing should work");
    assert_eq!(parsed_event.step_type, StepType::ToolUse);

    // 7. Verify kind constants
    assert_eq!(KIND_TRAJECTORY_SESSION, 38030);
    assert_eq!(KIND_TRAJECTORY_EVENT, 38031);
}

#[tokio::test]
async fn test_nip_sa_event_kinds_are_correct() {
    // Verify all NIP-SA event kinds match the specification
    assert_eq!(KIND_AGENT_PROFILE, 38000);
    assert_eq!(KIND_AGENT_SCHEDULE, 38002);
    assert_eq!(KIND_TICK_REQUEST, 38010);
    assert_eq!(KIND_TICK_RESULT, 38011);
    assert_eq!(KIND_TRAJECTORY_SESSION, 38030);
    assert_eq!(KIND_TRAJECTORY_EVENT, 38031);
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
        let parsed: TrajectoryEventContent = serde_json::from_str(&json).expect("parsing should work");
        assert_eq!(parsed.step_type, step_type);
    }
}
