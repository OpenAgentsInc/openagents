//! Integration test verifying that all NIP-SA types are publicly accessible

use nostr::{
    // State (kind:39201)
    AgentStateContent,
    Goal,
    GoalStatus,
    // Profile (kind:39200)
    KIND_AGENT_PROFILE,
    // Schedule (kind:39202)
    KIND_AGENT_SCHEDULE,
    KIND_AGENT_STATE,
    // Goals (kind:39203)
    KIND_PUBLIC_GOALS,
    KIND_SKILL_DELIVERY,
    KIND_SKILL_LICENSE,
    // Tick (kinds:39210, 39211)
    KIND_TICK_REQUEST,
    KIND_TICK_RESULT,
    KIND_TRAJECTORY_EVENT,
    KIND_TRAJECTORY_SESSION,
    MemoryEntry,
    // Skill (kinds:39220, 39221)
    SkillLicenseContent,
    // Trajectory (kinds:39230, 39231)
    StepType,
    TrajectoryEventContent,
    TrajectorySessionContent,
};

#[test]
fn test_nip_sa_event_kinds() {
    // Verify all event kind constants are accessible and have correct values
    assert_eq!(KIND_AGENT_PROFILE, 39200);
    assert_eq!(KIND_AGENT_STATE, 39201);
    assert_eq!(KIND_AGENT_SCHEDULE, 39202);
    assert_eq!(KIND_PUBLIC_GOALS, 39203);
    assert_eq!(KIND_TICK_REQUEST, 39210);
    assert_eq!(KIND_TICK_RESULT, 39211);
    assert_eq!(KIND_SKILL_LICENSE, 39220);
    assert_eq!(KIND_SKILL_DELIVERY, 39221);
    assert_eq!(KIND_TRAJECTORY_SESSION, 39230);
    assert_eq!(KIND_TRAJECTORY_EVENT, 39231);
}

#[test]
fn test_state_types_accessible() {
    let mut state = AgentStateContent::new();
    assert_eq!(state.goals.len(), 0);
    assert_eq!(state.memory.len(), 0);

    state.add_goal(Goal::new("goal-1", "Test goal", 1));
    assert_eq!(state.goals.len(), 1);
    assert_eq!(state.goals[0].status, GoalStatus::Active);

    state.add_memory(MemoryEntry::new("observation", "Test memory"));
    assert_eq!(state.memory.len(), 1);
}

#[test]
fn test_trajectory_types_accessible() {
    let session = TrajectorySessionContent {
        session_id: "test".to_string(),
        started_at: 0,
        ended_at: None,
        model: "codex".to_string(),
        total_events: 0,
        trajectory_hash: None,
    };
    assert_eq!(session.session_id, "test");

    let event = TrajectoryEventContent {
        step_type: StepType::ToolUse,
        data: Default::default(),
    };
    assert!(matches!(event.step_type, StepType::ToolUse));
}

#[test]
fn test_skill_types_accessible() {
    let license = SkillLicenseContent {
        skill_id: "test-123".to_string(),
        skill_name: "test".to_string(),
        version: "1.0.0".to_string(),
        granted_at: 0,
        expires_at: None,
        capabilities: vec![],
        restrictions: None,
    };
    assert_eq!(license.skill_id, "test-123");
}
