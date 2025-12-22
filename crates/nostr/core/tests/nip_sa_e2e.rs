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

use nostr::{finalize_event, generate_secret_key, EventTemplate};
use nostr::{
    AgentProfile, AgentProfileContent, AgentState, AgentStateContent, AgentSchedule,
    AutonomyLevel, Goal, GoalStatus, MemoryEntry, ThresholdConfig, TriggerType,
    KIND_AGENT_PROFILE, KIND_AGENT_STATE, KIND_AGENT_SCHEDULE,
};

#[test]
fn test_agent_profile_creation() {
    // 1. Create agent identity
    let agent_secret_key = generate_secret_key();
    let agent_pubkey = nostr::get_public_key(&agent_secret_key)
        .expect("should get agent pubkey");

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
    let threshold = ThresholdConfig::new(2, 3, marketplace_signer)
        .expect("should create threshold config");

    // 4. Create profile
    let profile = AgentProfile::new(
        profile_content.clone(),
        threshold,
        hex::encode(&agent_pubkey),
    )
    .with_lud16("agent@getalby.com");

    // 5. Convert to Nostr event
    let content = serde_json::to_string(&profile.content)
        .expect("should serialize profile");

    let template = EventTemplate {
        kind: KIND_AGENT_PROFILE,
        content,
        tags: profile.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key)
        .expect("should sign event");

    // 6. Verify event structure
    assert_eq!(event.kind, KIND_AGENT_PROFILE);
    assert!(!event.content.is_empty());

    // Verify tags include threshold config
    assert!(event.tags.iter().any(|t|
        t[0] == "threshold" && t[1] == "2" && t[2] == "3"
    ));

    // Verify tags include operator
    assert!(event.tags.iter().any(|t|
        t[0] == "operator" && t[1] == hex::encode(&agent_pubkey)
    ));

    // Verify tags include lud16
    assert!(event.tags.iter().any(|t|
        t[0] == "lud16" && t[1] == "agent@getalby.com"
    ));

    // 7. Parse back the profile content
    let parsed: AgentProfileContent = serde_json::from_str(&event.content)
        .expect("should deserialize profile");
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
    state_content.add_goal(Goal::new(
        "goal-002",
        "Generate weekly research summary",
        2,
    ));

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
    let content = serde_json::to_string(&state.content)
        .expect("should serialize state");

    let template = EventTemplate {
        kind: KIND_AGENT_STATE,
        content,
        tags: vec![], // State events typically have minimal tags (encrypted)
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key)
        .expect("should sign event");

    // 5. Verify event structure
    assert_eq!(event.kind, KIND_AGENT_STATE);
    assert!(!event.content.is_empty());

    // 6. Parse back the state content
    let parsed: AgentStateContent = serde_json::from_str(&event.content)
        .expect("should deserialize state");

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
        .with_heartbeat(3600).expect("valid heartbeat")  // Heartbeat every hour
        .add_trigger(TriggerType::Mention)  // Triggered when mentioned
        .add_trigger(TriggerType::Dm)  // Triggered on DM
        .add_trigger(TriggerType::Zap);  // Triggered on zap

    // 3. Convert to Nostr event
    let content = serde_json::to_string(&schedule)
        .expect("should serialize schedule");

    let template = EventTemplate {
        kind: KIND_AGENT_SCHEDULE,
        content,
        tags: schedule.build_tags(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let event = finalize_event(&template, &agent_secret_key)
        .expect("should sign event");

    // 4. Verify event structure
    assert_eq!(event.kind, KIND_AGENT_SCHEDULE);
    assert!(!event.content.is_empty());

    // Verify heartbeat tag
    assert!(event.tags.iter().any(|t|
        t[0] == "heartbeat" && t[1] == "3600"
    ));

    // Verify trigger tags
    assert!(event.tags.iter().any(|t|
        t[0] == "trigger" && t[1] == "mention"
    ));
    assert!(event.tags.iter().any(|t|
        t[0] == "trigger" && t[1] == "dm"
    ));
    assert!(event.tags.iter().any(|t|
        t[0] == "trigger" && t[1] == "zap"
    ));

    // 5. Parse back the schedule
    let parsed: AgentSchedule = serde_json::from_str(&event.content)
        .expect("should deserialize schedule");

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
