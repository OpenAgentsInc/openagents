//! HillClimber Monitor tests
//!
//! Tests for the HillClimber real-time visualization component.

use gpui::TestAppContext;
use gym_test::fixtures::{HillClimberFixture, HillClimberAssertExt};
use gym_test::types::{HCSessionStatus, HCMode};

// ============================================================================
// HillClimberMonitor Smoke Tests
// ============================================================================

#[gpui::test]
fn test_hillclimber_monitor_renders(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    // Should have a sample session by default
    view.assert_that(cx).has_session();
}

#[gpui::test]
fn test_hillclimber_has_sample_session(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    assert!(HillClimberFixture::has_session(&view, cx), "Should have sample session");
}

#[gpui::test]
fn test_hillclimber_sample_session_status(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    // Sample session should be running
    view.assert_that(cx)
        .has_session()
        .is_running();
}

// ============================================================================
// Session State Tests
// ============================================================================

#[gpui::test]
fn test_hillclimber_session_mode(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    // Sample session uses Standard mode
    view.assert_that(cx)
        .has_mode(HCMode::Standard);
}

#[gpui::test]
fn test_hillclimber_session_turn_progress(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    // Sample session is on turn 4
    view.assert_that(cx).has_turn(4);

    let max_turns = HillClimberFixture::max_turns(&view, cx);
    assert_eq!(max_turns, Some(10), "Max turns should be 10 for Standard mode");
}

#[gpui::test]
fn test_hillclimber_session_test_results(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    // Sample session has 7/12 tests passing
    view.assert_that(cx)
        .has_tests_passed(7)
        .has_tests_total(12);
}

#[gpui::test]
fn test_hillclimber_session_pass_rate(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    let rate = HillClimberFixture::pass_rate(&view, cx);
    assert!(rate.is_some());

    // 7/12 = 58.33%
    let rate = rate.unwrap();
    assert!(rate > 58.0 && rate < 59.0, "Pass rate should be ~58.33%, got {}", rate);
}

#[gpui::test]
fn test_hillclimber_session_task_info(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    // Sample session is for regex-log task
    view.assert_that(cx).has_task_name("Regex Log Parser");

    let task_id = HillClimberFixture::task_id(&view, cx);
    assert_eq!(task_id, Some("regex-log".to_string()));
}

// ============================================================================
// Panel State Tests
// ============================================================================

#[gpui::test]
fn test_hillclimber_left_panel_starts_expanded(cx: &mut TestAppContext) {
    let view = HillClimberFixture::create(cx);

    let collapsed = HillClimberFixture::left_collapsed(&view, cx);
    assert!(!collapsed, "Left panel should start expanded");
}

// ============================================================================
// Mode Label Tests
// ============================================================================

#[gpui::test]
fn test_hillclimber_mode_labels() {
    assert_eq!(HCMode::Quick.label(), "Quick (3)");
    assert_eq!(HCMode::Standard.label(), "Standard (10)");
    assert_eq!(HCMode::Full.label(), "Full (25)");
}

#[gpui::test]
fn test_hillclimber_mode_max_turns() {
    assert_eq!(HCMode::Quick.max_turns(), 3);
    assert_eq!(HCMode::Standard.max_turns(), 10);
    assert_eq!(HCMode::Full.max_turns(), 25);
}

// ============================================================================
// Status Label Tests
// ============================================================================

#[gpui::test]
fn test_hillclimber_status_labels() {
    assert_eq!(HCSessionStatus::Idle.label(), "Idle");
    assert_eq!(HCSessionStatus::Running.label(), "Running");
    assert_eq!(HCSessionStatus::Paused.label(), "Paused");
    assert_eq!(HCSessionStatus::Completed.label(), "Completed");
    assert_eq!(HCSessionStatus::Failed.label(), "Failed");
}
