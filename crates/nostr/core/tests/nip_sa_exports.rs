//! Integration test verifying that all NIP-SA types are publicly accessible

use nostr::{
    // Profile (kind:38000)
    KIND_AGENT_PROFILE,
    // State (kind:38001)
    AgentStateContent, Goal, GoalStatus, MemoryEntry, KIND_AGENT_STATE,
    // Schedule (kind:38002)
    KIND_AGENT_SCHEDULE,
    // Goals (kind:38003)
    KIND_PUBLIC_GOALS,
    // Tick (kinds:38010, 38011)
    KIND_TICK_REQUEST, KIND_TICK_RESULT,
    // Trajectory (kinds:38030, 38031)
    StepType, TrajectoryEventContent, TrajectorySessionContent, KIND_TRAJECTORY_EVENT,
    KIND_TRAJECTORY_SESSION,
    // Skill (kinds:38020, 38021)
    SkillLicenseContent, KIND_SKILL_DELIVERY, KIND_SKILL_LICENSE,
};

#[test]
fn test_nip_sa_event_kinds() {
    // Verify all event kind constants are accessible and have correct values
    assert_eq!(KIND_AGENT_PROFILE, 38000);
    assert_eq!(KIND_AGENT_STATE, 38001);
    assert_eq!(KIND_AGENT_SCHEDULE, 38002);
    assert_eq!(KIND_PUBLIC_GOALS, 38003);
    assert_eq!(KIND_TICK_REQUEST, 38010);
    assert_eq!(KIND_TICK_RESULT, 38011);
    assert_eq!(KIND_SKILL_LICENSE, 38020);
    assert_eq!(KIND_SKILL_DELIVERY, 38021);
    assert_eq!(KIND_TRAJECTORY_SESSION, 38030);
    assert_eq!(KIND_TRAJECTORY_EVENT, 38031);
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
        model: "claude".to_string(),
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
