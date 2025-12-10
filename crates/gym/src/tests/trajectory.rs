//! E2E tests for TrajectoryView component
//!
//! Tests trajectory list display, selection, and store integration.

use gpui::TestAppContext;
use gym_test::fixtures::{TrajectoryViewFixture, TrajectoryAssertExt};
use std::sync::{Arc, Mutex};
use atif_store::TrajectoryStore;
use atif::{Trajectory, Step, StepSource};
use chrono::Utc;

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
    {
        let mut guard = store.lock().unwrap();
        let trajectory = create_test_trajectory("test-session-1", "TestAgent", 3);
        guard.save_trajectory(&trajectory).expect("Failed to save trajectory");
    }

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
    {
        let mut guard = store.lock().unwrap();
        for i in 0..5 {
            let trajectory = create_test_trajectory(&format!("session-{}", i), "Agent", 2);
            guard.save_trajectory(&trajectory).expect("Failed to save trajectory");
        }
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
    {
        let mut guard = store.lock().unwrap();
        let trajectory = create_test_trajectory("my-session", "TestAgent", 3);
        guard.save_trajectory(&trajectory).expect("Failed to save trajectory");
    }

    TrajectoryViewFixture::refresh(&view, cx);
    view.assert_that(cx).has_no_selection();

    // Select the trajectory
    TrajectoryViewFixture::select_trajectory(&view, "my-session", cx);

    view.assert_that(cx)
        .has_selection()
        .has_selected_id("my-session");
}

#[gpui::test]
fn test_select_trajectory_loads_steps(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add a trajectory with steps
    {
        let mut guard = store.lock().unwrap();
        let trajectory = create_test_trajectory("step-session", "StepAgent", 5);
        guard.save_trajectory(&trajectory).expect("Failed to save trajectory");
    }

    TrajectoryViewFixture::refresh(&view, cx);
    view.assert_that(cx).has_no_selected_steps();

    // Select the trajectory
    TrajectoryViewFixture::select_trajectory(&view, "step-session", cx);

    view.assert_that(cx)
        .has_selected_steps()
        .has_selected_step_count(5);
}

#[gpui::test]
fn test_select_different_trajectory_changes_selection(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add two trajectories with different step counts
    {
        let mut guard = store.lock().unwrap();
        let traj1 = create_test_trajectory("session-a", "Agent", 2);
        let traj2 = create_test_trajectory("session-b", "Agent", 7);
        guard.save_trajectory(&traj1).expect("Failed to save trajectory");
        guard.save_trajectory(&traj2).expect("Failed to save trajectory");
    }

    TrajectoryViewFixture::refresh(&view, cx);

    // Select first trajectory
    TrajectoryViewFixture::select_trajectory(&view, "session-a", cx);
    view.assert_that(cx)
        .has_selected_id("session-a")
        .has_selected_step_count(2);

    // Select second trajectory
    TrajectoryViewFixture::select_trajectory(&view, "session-b", cx);
    view.assert_that(cx)
        .has_selected_id("session-b")
        .has_selected_step_count(7);
}

#[gpui::test]
fn test_select_nonexistent_trajectory_clears_steps(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add a trajectory
    {
        let mut guard = store.lock().unwrap();
        let trajectory = create_test_trajectory("real-session", "Agent", 3);
        guard.save_trajectory(&trajectory).expect("Failed to save trajectory");
    }

    TrajectoryViewFixture::refresh(&view, cx);

    // Select the real trajectory
    TrajectoryViewFixture::select_trajectory(&view, "real-session", cx);
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
    {
        let mut guard = store.lock().unwrap();
        let trajectory = create_test_trajectory("empty-session", "Agent", 0);
        guard.save_trajectory(&trajectory).expect("Failed to save trajectory");
    }

    TrajectoryViewFixture::refresh(&view, cx);
    TrajectoryViewFixture::select_trajectory(&view, "empty-session", cx);

    view.assert_that(cx)
        .has_selection()
        .has_no_selected_steps();
}

#[gpui::test]
fn test_multiple_refreshes_are_idempotent(cx: &mut TestAppContext) {
    let (view, store) = TrajectoryViewFixture::create_with_store(cx);

    // Add trajectories
    {
        let mut guard = store.lock().unwrap();
        for i in 0..3 {
            let trajectory = create_test_trajectory(&format!("session-{}", i), "Agent", 1);
            guard.save_trajectory(&trajectory).expect("Failed to save trajectory");
        }
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

fn create_test_trajectory(session_id: &str, agent_name: &str, step_count: usize) -> Trajectory {
    let steps: Vec<Step> = (0..step_count)
        .map(|i| Step {
            id: format!("step-{}", i),
            source: if i % 2 == 0 { StepSource::User } else { StepSource::Agent },
            message: format!("Test message {}", i),
            timestamp: Utc::now(),
            tool_calls: vec![],
            tool_results: vec![],
            metadata: Default::default(),
        })
        .collect();

    Trajectory {
        session_id: session_id.to_string(),
        agent_name: agent_name.to_string(),
        model_name: Some("test-model".to_string()),
        started_at: Utc::now(),
        completed_at: if step_count > 0 { Some(Utc::now()) } else { None },
        steps,
        final_result: None,
        metadata: Default::default(),
    }
}
