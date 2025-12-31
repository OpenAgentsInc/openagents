//! End-to-end integration tests for NIP-SA (Sovereign Agents Protocol)
//!
//! These tests verify that the complete NIP-SA stack works correctly, testing:
//! - Agent profile creation and identity management (kind 38000)
//! - Agent state updates with goals and memory (kind 38001)
//! - Agent schedule configuration (kind 38002)
//! - Public goals publishing (kind 38003)
//!
//! Unlike unit tests which test individual types, these tests validate
//! the complete event creation and serialization flow for NIP-SA events.
//!
//! Part of d-006: Operationalize NIP-SA (Sovereign Agents Protocol)

use nostr::{
    AgentProfile, AgentProfileContent, AgentSchedule, AgentState, AgentStateContent, AutonomyLevel,
    Goal, GoalStatus, KIND_AGENT_PROFILE, KIND_AGENT_SCHEDULE, KIND_AGENT_STATE, KIND_TICK_REQUEST,
    KIND_TICK_RESULT, KIND_TRAJECTORY_EVENT, KIND_TRAJECTORY_SESSION, MemoryEntry, StepType,
    ThresholdConfig, TickAction, TickRequest, TickResult, TickResultContent, TickStatus,
    TickTrigger, TrajectoryEvent, TrajectoryEventContent, TrajectorySession,
    TrajectorySessionContent, TrajectoryVisibility, TriggerType,
};
use nostr::{EventTemplate, finalize_event, generate_secret_key};

#[test]
fn test_agent_profile_creation() {
    // 1. Create agent identity
    let agent_secret_key = generate_secret_key();
    let agent_pubkey = nostr::get_public_key(&agent_secret_key).expect("should get agent pubkey");

    // 2. Create agent profile content
    let profile_content = AgentProfileContent::new(
        "Autonomous Research Agent",
        "AI agent specializing in research and data analysis",
        AutonomyLevel::Autonomous,
        "1.0.0",
    )
    .with_picture("https://example.com/avatar.png")
    .with_capabilities(vec![
        "research".to_string(),
        "data-analysis".to_string(),
        "report-generation".to_string(),
    ]);

    // 3. Create threshold config (2-of-3)
    let marketplace_signer = hex::encode([0x11u8; 32]);
    let threshold =
        ThresholdConfig::new(2, 3, marketplace_signer).expect("should create threshold config");

    // 4. Create profile
    let profile = AgentProfile::new(profile_content, threshold, hex::encode(&agent_pubkey))
        .with_lud16("agent@getalby.com");

    // 5. Convert to Nostr event
    let content = serde_json::to_string(&profile.content).expect("should serialize profile");

    let template = EventTemplate {
        kind: KIND_AGENT_PROFILE,
        content,
        tags: profile.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key).expect("should sign event");

    // 6. Verify event structure
    assert_eq!(event.kind, KIND_AGENT_PROFILE);
    assert!(!event.content.is_empty());

    // Verify tags include threshold config
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "threshold" && t[1] == "2" && t[2] == "3")
    );

    // Verify tags include operator
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "operator" && t[1] == hex::encode(&agent_pubkey))
    );

    // Verify tags include lud16
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "lud16" && t[1] == "agent@getalby.com")
    );

    // 7. Parse back the profile content
    let parsed: AgentProfileContent =
        serde_json::from_str(&event.content).expect("should deserialize profile");
    assert_eq!(parsed.name, "Autonomous Research Agent");
    assert_eq!(parsed.capabilities.len(), 3);
    assert_eq!(parsed.version, "1.0.0");
}

#[test]
fn test_agent_state_update() {
    // 1. Create agent identity
    let agent_secret_key = generate_secret_key();

    // 2. Create agent state with goals and memory
    let mut state_content = AgentStateContent::new();

    // Add some goals
    state_content.add_goal(Goal::new(
        "goal-001",
        "Analyze latest AI research papers",
        1,
    ));
    state_content.add_goal(Goal::new("goal-002", "Generate weekly research summary", 2));

    // Add memory entries
    state_content.add_memory(MemoryEntry::new(
        "observation",
        "Found 15 new papers on multimodal learning",
    ));
    state_content.add_memory(MemoryEntry::new(
        "action",
        "Scheduled analysis task for tomorrow",
    ));

    // 3. Create state
    let state = AgentState::new(state_content.clone());

    // 4. Convert to Nostr event (encrypted in production, plaintext for testing)
    let content = serde_json::to_string(&state.content).expect("should serialize state");

    let template = EventTemplate {
        kind: KIND_AGENT_STATE,
        content,
        tags: vec![], // State events typically have minimal tags (encrypted)
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key).expect("should sign event");

    // 5. Verify event structure
    assert_eq!(event.kind, KIND_AGENT_STATE);
    assert!(!event.content.is_empty());

    // 6. Parse back the state content
    let parsed: AgentStateContent =
        serde_json::from_str(&event.content).expect("should deserialize state");

    assert_eq!(parsed.goals.len(), 2);
    assert_eq!(parsed.goals[0].id, "goal-001");
    assert_eq!(parsed.goals[0].status, GoalStatus::Active);
    assert_eq!(parsed.goals[1].priority, 2);

    assert_eq!(parsed.memory.len(), 2);
    assert_eq!(parsed.memory[0].memory_type, "observation");
    assert!(parsed.memory[0].content.contains("15 new papers"));
}

#[test]
fn test_agent_schedule_creation() {
    // 1. Create agent identity
    let agent_secret_key = generate_secret_key();

    // 2. Create agent schedule
    let schedule = AgentSchedule::new()
        .with_heartbeat(3600)
        .expect("valid heartbeat") // Heartbeat every hour
        .add_trigger(TriggerType::Mention) // Triggered when mentioned
        .add_trigger(TriggerType::Dm) // Triggered on DM
        .add_trigger(TriggerType::Zap); // Triggered on zap

    // 3. Convert to Nostr event
    let content = serde_json::to_string(&schedule).expect("should serialize schedule");

    let template = EventTemplate {
        kind: KIND_AGENT_SCHEDULE,
        content,
        tags: schedule.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key).expect("should sign event");

    // 4. Verify event structure
    assert_eq!(event.kind, KIND_AGENT_SCHEDULE);
    assert!(!event.content.is_empty());

    // Verify heartbeat tag
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "heartbeat" && t[1] == "3600")
    );

    // Verify trigger tags
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "trigger" && t[1] == "mention")
    );
    assert!(event.tags.iter().any(|t| t[0] == "trigger" && t[1] == "dm"));
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "trigger" && t[1] == "zap")
    );

    // 5. Parse back the schedule
    let parsed: AgentSchedule =
        serde_json::from_str(&event.content).expect("should deserialize schedule");

    assert_eq!(parsed.heartbeat_seconds, Some(3600));
    assert_eq!(parsed.triggers.len(), 3);
    assert!(matches!(parsed.triggers[0], TriggerType::Mention));
    assert!(matches!(parsed.triggers[1], TriggerType::Dm));
    assert!(matches!(parsed.triggers[2], TriggerType::Zap));

    // NOTE: Full E2E test with relay would include:
    // - Agent publishes profile announcement to relay
    // - Agent publishes initial state (encrypted) to relay
    // - Agent publishes schedule configuration to relay
    // - Scheduler subscribes to schedule events and triggers ticks
    // - Agent receives tick requests and publishes tick results
    // - State updates are published after each tick
    // - External observers can subscribe to profile and public goals
    // - Threshold signatures protect agent identity in production
}

#[test]
fn test_tick_request_creation() {
    // 1. Create identities
    let agent_secret_key = generate_secret_key();
    let runner_secret_key = generate_secret_key();

    let runner_pubkey =
        nostr::get_public_key(&runner_secret_key).expect("should get runner pubkey");

    // 2. Create tick request (runner requests agent to execute a tick)
    let tick_request = TickRequest::new(hex::encode(&runner_pubkey), TickTrigger::Heartbeat);

    // 3. Convert to Nostr event
    let template = EventTemplate {
        kind: KIND_TICK_REQUEST,
        content: String::new(), // Tick requests have empty content
        tags: tick_request.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key).expect("should sign event");

    // 4. Verify event structure
    assert_eq!(event.kind, KIND_TICK_REQUEST);
    assert!(event.content.is_empty());

    // Verify tags include runner
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "runner" && t[1] == hex::encode(&runner_pubkey))
    );

    // Verify tags include trigger type
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "trigger" && t[1] == "heartbeat")
    );

    // NOTE: Full tick request workflow would include:
    // - Scheduler monitors agent schedule events
    // - Heartbeat timer or event trigger fires
    // - Scheduler publishes tick request to relay
    // - Agent subscribes to tick requests for their pubkey
    // - Agent receives tick request and begins execution
}

#[test]
fn test_tick_result_publishing() {
    // 1. Create identities
    let agent_secret_key = generate_secret_key();
    let runner_pubkey = hex::encode([0x22u8; 32]);

    // 2. Simulate a previous tick request
    let request_id = hex::encode([0x99u8; 32]);

    // 3. Create tick result content with metrics and actions
    let mut action1_meta = serde_json::Map::new();
    action1_meta.insert(
        "goal_id".to_string(),
        serde_json::Value::String("goal-001".to_string()),
    );

    let mut action2_meta = serde_json::Map::new();
    action2_meta.insert(
        "content".to_string(),
        serde_json::Value::String("Published research summary".to_string()),
    );

    let result_content = TickResultContent::new(
        1500,   // tokens_in
        800,    // tokens_out
        0.0023, // cost_usd
        2,      // goals_updated
    )
    .add_action(TickAction {
        action_type: "update_goal".to_string(),
        id: None,
        metadata: action1_meta,
    })
    .add_action(TickAction {
        action_type: "post".to_string(),
        id: Some(hex::encode([0xABu8; 32])),
        metadata: action2_meta,
    });

    // 4. Create tick result
    let tick_result = TickResult::new(
        request_id.clone(),
        runner_pubkey.clone(),
        TickStatus::Success,
        2500, // duration_ms
        result_content,
    );

    // 5. Convert to Nostr event
    let content =
        serde_json::to_string(&tick_result.content).expect("should serialize result content");

    let template = EventTemplate {
        kind: KIND_TICK_RESULT,
        content,
        tags: tick_result.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key).expect("should sign event");

    // 6. Verify event structure
    assert_eq!(event.kind, KIND_TICK_RESULT);
    assert!(!event.content.is_empty());

    // Verify tags include request ID reference
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "request" && t[1] == request_id)
    );

    // Verify tags include runner
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "runner" && t[1] == runner_pubkey)
    );

    // Verify tags include status
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "status" && t[1] == "success")
    );

    // Verify tags include duration
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "duration_ms" && t[1] == "2500")
    );

    // Verify tags include action count
    assert!(event.tags.iter().any(|t| t[0] == "actions" && t[1] == "2"));

    // 7. Parse back the result content
    let parsed: TickResultContent =
        serde_json::from_str(&event.content).expect("should deserialize result");

    assert_eq!(parsed.tokens_in, 1500);
    assert_eq!(parsed.tokens_out, 800);
    assert_eq!(parsed.cost_usd, 0.0023);
    assert_eq!(parsed.goals_updated, 2);
    assert_eq!(parsed.actions.len(), 2);

    assert_eq!(parsed.actions[0].action_type, "update_goal");
    assert_eq!(parsed.actions[1].action_type, "post");
    assert!(parsed.actions[1].id.is_some());

    // NOTE: Full tick result workflow would include:
    // - Agent completes tick execution
    // - Agent publishes tick result with metrics
    // - Runner monitors tick results via subscription
    // - Runner tracks token usage and costs
    // - Runner updates agent state with new goals/memory
    // - Dashboard displays tick metrics and execution history
    // - Billing system charges for token usage
}

#[test]
fn test_trajectory_session_lifecycle() {
    // 1. Create agent identity
    let agent_secret_key = generate_secret_key();

    // 2. Create trajectory session (starts when tick begins)
    let session_content = TrajectorySessionContent::new(
        "session-abc123",
        1704067200, // 2024-01-01 00:00:00 UTC
        "claude-sonnet-4.5",
    );

    let tick_id = hex::encode([0xCCu8; 32]);
    let trajectory_session = TrajectorySession::new(
        session_content,
        tick_id.clone(),
        TrajectoryVisibility::Public,
    );

    // 3. Convert to Nostr event
    let content =
        serde_json::to_string(&trajectory_session.content).expect("should serialize session");

    let template = EventTemplate {
        kind: KIND_TRAJECTORY_SESSION,
        content,
        tags: trajectory_session.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key).expect("should sign event");

    // 4. Verify event structure
    assert_eq!(event.kind, KIND_TRAJECTORY_SESSION);
    assert!(!event.content.is_empty());

    // Verify tags
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "d" && t[1] == "session-abc123")
    );
    assert!(event.tags.iter().any(|t| t[0] == "tick" && t[1] == tick_id));
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "visibility" && t[1] == "public")
    );
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "started_at" && t[1] == "1704067200")
    );
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "model" && t[1] == "claude-sonnet-4.5")
    );

    // 5. Parse back the session
    let parsed: TrajectorySessionContent =
        serde_json::from_str(&event.content).expect("should deserialize session");

    assert_eq!(parsed.session_id, "session-abc123");
    assert_eq!(parsed.started_at, 1704067200);
    assert_eq!(parsed.model, "claude-sonnet-4.5");
    assert_eq!(parsed.total_events, 0);
    assert!(parsed.ended_at.is_none());
}

#[test]
fn test_trajectory_event_publishing() {
    // 1. Create agent identity
    let agent_secret_key = generate_secret_key();

    // 2. Create trajectory events for different step types
    let session_id = "session-xyz789";
    let tick_id = hex::encode([0xDDu8; 32]);

    // Tool use step
    let mut tool_data = serde_json::Map::new();
    tool_data.insert(
        "tool".to_string(),
        serde_json::Value::String("read_file".to_string()),
    );
    tool_data.insert(
        "input".to_string(),
        serde_json::Value::String("{\"path\":\"/etc/passwd\"}".to_string()),
    );

    let tool_event = TrajectoryEvent::new(
        TrajectoryEventContent {
            step_type: StepType::ToolUse,
            data: tool_data,
        },
        session_id,
        tick_id.clone(),
        1, // First event in sequence
    );

    // Convert to Nostr event
    let content = serde_json::to_string(&tool_event.content).expect("should serialize event");

    let template = EventTemplate {
        kind: KIND_TRAJECTORY_EVENT,
        content,
        tags: tool_event.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key).expect("should sign event");

    // 3. Verify event structure
    assert_eq!(event.kind, KIND_TRAJECTORY_EVENT);
    assert!(!event.content.is_empty());

    // Verify tags
    assert!(
        event
            .tags
            .iter()
            .any(|t| t[0] == "session" && t[1] == session_id)
    );
    assert!(event.tags.iter().any(|t| t[0] == "tick" && t[1] == tick_id));
    assert!(event.tags.iter().any(|t| t[0] == "seq" && t[1] == "1"));

    // 4. Parse back the event content
    let parsed: TrajectoryEventContent =
        serde_json::from_str(&event.content).expect("should deserialize event");

    assert!(matches!(parsed.step_type, StepType::ToolUse));
    assert_eq!(
        parsed.data.get("tool").unwrap().as_str().unwrap(),
        "read_file"
    );

    // NOTE: Full trajectory workflow would include:
    // - Agent starts tick, publishes trajectory session (kind 38030)
    // - For each step, agent publishes trajectory event (kind 38031)
    // - Events are ordered by sequence number
    // - Session is finalized with ended_at timestamp and trajectory_hash
    // - External observers subscribe to trajectory events for transparency
    // - Trajectory data enables training, debugging, and auditing
    // - Privacy: TrajectoryVisibility controls who can see thinking/tool details
}
