//! TestGen Visualizer tests
//!
//! Tests for the TestGen test generation visualization component.

use gpui::TestAppContext;
use gym_test::fixtures::{TestGenFixture, TestGenAssertExt};
use gym_test::types::{TestGenStatus, TestCategory};

// ============================================================================
// TestGenVisualizer Smoke Tests
// ============================================================================

#[gpui::test]
fn test_testgen_visualizer_renders(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Should have a sample session by default
    view.assert_that(cx).has_session();
}

#[gpui::test]
fn test_testgen_has_sample_session(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    assert!(TestGenFixture::has_session(&view, cx), "Should have sample session");
}

#[gpui::test]
fn test_testgen_sample_session_status(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Sample session should be generating
    view.assert_that(cx)
        .has_session()
        .is_generating();
}

// ============================================================================
// Session State Tests
// ============================================================================

#[gpui::test]
fn test_testgen_session_iteration(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Sample session is on iteration 3
    view.assert_that(cx).has_iteration(3);

    let max = TestGenFixture::max_iterations(&view, cx);
    assert_eq!(max, Some(5), "Max iterations should be 5");
}

#[gpui::test]
fn test_testgen_session_comprehensiveness(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Sample session has 72% comprehensiveness
    view.assert_that(cx).has_comprehensiveness_at_least(0.7);

    let score = TestGenFixture::comprehensiveness(&view, cx);
    assert!(score.is_some());
    assert!((score.unwrap() - 0.72).abs() < 0.01, "Comprehensiveness should be ~0.72");
}

#[gpui::test]
fn test_testgen_session_target_not_met(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Sample session hasn't met target (72% < 85%)
    let target = TestGenFixture::target_comprehensiveness(&view, cx);
    assert_eq!(target, Some(0.85), "Target should be 85%");

    let is_met = TestGenFixture::is_target_met(&view, cx);
    assert!(!is_met, "Target should not be met yet");
}

#[gpui::test]
fn test_testgen_session_task_info(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    // Sample session is for regex-log task
    view.assert_that(cx).has_task_name("Regex Log Parser");

    let task_id = TestGenFixture::task_id(&view, cx);
    assert_eq!(task_id, Some("regex-log".to_string()));
}

// ============================================================================
// Category Tests
// ============================================================================

#[gpui::test]
fn test_testgen_category_labels() {
    assert_eq!(TestCategory::AntiCheat.label(), "Anti-Cheat");
    assert_eq!(TestCategory::Existence.label(), "Existence");
    assert_eq!(TestCategory::Correctness.label(), "Correctness");
    assert_eq!(TestCategory::Boundary.label(), "Boundary");
    assert_eq!(TestCategory::Integration.label(), "Integration");
}

#[gpui::test]
fn test_testgen_category_descriptions() {
    assert_eq!(TestCategory::AntiCheat.description(), "Prevent hardcoded solutions");
    assert_eq!(TestCategory::Existence.description(), "Basic functionality exists");
    assert_eq!(TestCategory::Correctness.description(), "Outputs are correct");
    assert_eq!(TestCategory::Boundary.description(), "Edge cases handled");
    assert_eq!(TestCategory::Integration.description(), "Components work together");
}

#[gpui::test]
fn test_testgen_category_icons() {
    assert_eq!(TestCategory::AntiCheat.icon(), "AC");
    assert_eq!(TestCategory::Existence.icon(), "EX");
    assert_eq!(TestCategory::Correctness.icon(), "CO");
    assert_eq!(TestCategory::Boundary.icon(), "BO");
    assert_eq!(TestCategory::Integration.icon(), "IN");
}

// ============================================================================
// Status Label Tests
// ============================================================================

#[gpui::test]
fn test_testgen_status_labels() {
    assert_eq!(TestGenStatus::Idle.label(), "Idle");
    assert_eq!(TestGenStatus::Generating.label(), "Generating");
    assert_eq!(TestGenStatus::Completed.label(), "Completed");
    assert_eq!(TestGenStatus::Failed.label(), "Failed");
}

// ============================================================================
// Selected Test Tests
// ============================================================================

#[gpui::test]
fn test_testgen_no_test_selected_initially(cx: &mut TestAppContext) {
    let view = TestGenFixture::create(cx);

    let selected = TestGenFixture::selected_test_id(&view, cx);
    assert!(selected.is_none(), "No test should be selected initially");
}
