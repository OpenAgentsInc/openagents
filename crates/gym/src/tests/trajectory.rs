//! E2E tests for TrajectoryView component
//!
//! Tests trajectory list display, selection, and store integration.

use gpui::TestAppContext;
use gym_test::fixtures::{TrajectoryViewFixture, TrajectoryAssertExt};
use std::sync::{Arc, Mutex};
use atif_store::TrajectoryStore;
use atif::{Agent, Step};

// ============================================================================
// Initialization Tests
// ============================================================================

#[gpui::test]
fn test_trajectory_view_starts_empty(cx: &mut TestAppContext) {
    let view = TrajectoryViewFixture::create(cx);

    view.assert_that(cx)
        .has_no_store()
        .has_no_trajectories()
        .has_no_selection();
}

#[gpui::test]
fn test_trajectory_view_with_store_starts_empty(cx: &mut TestAppContext) {
    let (view, _store) = TrajectoryViewFixture::create_with_store(cx);

    view.assert_that(cx)
        .has_store()
        .has_no_trajectories()
        .has_no_selection();
}

// ============================================================================
// Store Integration Tests
// ============================================================================

#[gpui::test]
fn test_set_store_enables_store(cx: &mut TestAppContext) {
    let view = TrajectoryViewFixture::create(cx);
    view.assert_that(cx).has_no_store();

    let store = Arc::new(Mutex::new(
        TrajectoryStore::in_memory().expect("Failed to create store")
    ));
    TrajectoryViewFixture::set_store(&view, store, cx);

    view.assert_that(cx).has_store();
}

#[gpui::test]
fn test_refresh_loads_trajectories_from_store(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add a trajectory to the store
    let session_id = create_test_trajectory(&store, "TestAgent", 3);
    assert!(!session_id.is_empty());

    // Refresh the view
    TrajectoryViewFixture::refresh(&view, cx);

    view.assert_that(cx)
        .has_trajectories()
        .has_trajectory_count(1);
}

#[gpui::test]
fn test_loads_multiple_trajectories(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add multiple trajectories
    for i in 0..5 {
        create_test_trajectory(&store, &format!("Agent{}", i), 2);
    }

    TrajectoryViewFixture::refresh(&view, cx);

    view.assert_that(cx).has_trajectory_count(5);
}

// ============================================================================
// Selection Tests
// ============================================================================

#[gpui::test]
fn test_select_trajectory_updates_selection(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add a trajectory
    let session_id = create_test_trajectory(&store, "TestAgent", 3);

    TrajectoryViewFixture::refresh(&view, cx);
    view.assert_that(cx).has_no_selection();

    // Select the trajectory
    TrajectoryViewFixture::select_trajectory(&view, &session_id, cx);

    view.assert_that(cx)
        .has_selection()
        .has_selected_id(&session_id);
}

#[gpui::test]
fn test_select_trajectory_loads_steps(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add a trajectory with steps
    let session_id = create_test_trajectory(&store, "StepAgent", 5);

    TrajectoryViewFixture::refresh(&view, cx);
    view.assert_that(cx).has_no_selected_steps();

    // Select the trajectory
    TrajectoryViewFixture::select_trajectory(&view, &session_id, cx);

    view.assert_that(cx)
        .has_selected_steps()
        .has_selected_step_count(5);
}

#[gpui::test]
fn test_select_different_trajectory_changes_selection(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add two trajectories with different step counts
    let session_a = create_test_trajectory(&store, "AgentA", 2);
    let session_b = create_test_trajectory(&store, "AgentB", 7);

    TrajectoryViewFixture::refresh(&view, cx);

    // Select first trajectory
    TrajectoryViewFixture::select_trajectory(&view, &session_a, cx);
    view.assert_that(cx)
        .has_selected_id(&session_a)
        .has_selected_step_count(2);

    // Select second trajectory
    TrajectoryViewFixture::select_trajectory(&view, &session_b, cx);
    view.assert_that(cx)
        .has_selected_id(&session_b)
        .has_selected_step_count(7);
}

#[gpui::test]
fn test_select_nonexistent_trajectory_clears_steps(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add a trajectory
    let session_id = create_test_trajectory(&store, "Agent", 3);

    TrajectoryViewFixture::refresh(&view, cx);

    // Select the real trajectory
    TrajectoryViewFixture::select_trajectory(&view, &session_id, cx);
    view.assert_that(cx).has_selected_step_count(3);

    // Try to select a nonexistent trajectory
    TrajectoryViewFixture::select_trajectory(&view, "nonexistent", cx);

    // Steps should be cleared, but ID is still set (no steps loaded)
    view.assert_that(cx)
        .has_selected_id("nonexistent")
        .has_no_selected_steps();
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[gpui::test]
fn test_empty_store_returns_no_trajectories(cx: &mut TestAppContext) {
    let (view, _store) = TrajectoryViewFixture::create_with_store(cx);

    TrajectoryViewFixture::refresh(&view, cx);

    view.assert_that(cx).has_no_trajectories();
}

#[gpui::test]
fn test_trajectory_with_zero_steps(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add a trajectory with no steps
    let session_id = create_test_trajectory(&store, "Agent", 0);

    TrajectoryViewFixture::refresh(&view, cx);
    TrajectoryViewFixture::select_trajectory(&view, &session_id, cx);

    view.assert_that(cx)
        .has_selection()
        .has_no_selected_steps();
}

#[gpui::test]
fn test_multiple_refreshes_are_idempotent(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add trajectories
    for i in 0..3 {
        create_test_trajectory(&store, &format!("Agent{}", i), 1);
    }

    // Refresh multiple times
    for _ in 0..5 {
        TrajectoryViewFixture::refresh(&view, cx);
    }

    view.assert_that(cx).has_trajectory_count(3);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test trajectory and return its session ID
fn create_test_trajectory(
    store: &Arc<Mutex<TrajectoryStore>>,
    agent_name: &str,
    step_count: usize,
) -> String {
    let mut guard = store.lock().unwrap();

    // Create agent
    let agent = Agent::new(agent_name, "1.0.0")
        .with_model("test-model");

    // Create trajectory
    let session_id = guard.create_trajectory(&agent).expect("Failed to create trajectory");

    // Add steps
    for i in 0..step_count {
        let step = if i % 2 == 0 {
            Step::user(i as i64 + 1, format!("User message {}", i))
        } else {
            Step::agent(i as i64 + 1, format!("Agent response {}", i))
        };
        guard.add_step(&session_id, &step).expect("Failed to add step");
    }

    // Complete the trajectory if it has steps
    if step_count > 0 {
        guard.complete_trajectory(&session_id, None).expect("Failed to complete trajectory");
    }

    session_id
}
