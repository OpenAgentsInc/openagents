//! Integration tests for Gym components
//!
//! Tests for cross-component interactions and navigation flows.

use gpui::TestAppContext;
use gym_test::fixtures::{GymScreenFixture, GymScreenAssertExt};
use gym_test::types::GymTab;

// ============================================================================
// Tab Navigation Integration Tests
// ============================================================================

#[gpui::test]
fn test_navigate_through_all_tabs_sequentially(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Navigate through all tabs in order
    view.assert_that(cx).is_on_trajectories();

    GymScreenFixture::go_to_tbcc(&view, cx);
    view.assert_that(cx).is_on_tbcc();

    GymScreenFixture::go_to_hillclimber(&view, cx);
    view.assert_that(cx).is_on_hillclimber();

    GymScreenFixture::go_to_testgen(&view, cx);
    view.assert_that(cx).is_on_testgen();
}

#[gpui::test]
fn test_navigate_tabs_reverse_order(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Start at TestGen
    GymScreenFixture::go_to_testgen(&view, cx);
    view.assert_that(cx).is_on_testgen();

    // Navigate backwards
    GymScreenFixture::go_to_hillclimber(&view, cx);
    view.assert_that(cx).is_on_hillclimber();

    GymScreenFixture::go_to_tbcc(&view, cx);
    view.assert_that(cx).is_on_tbcc();

    GymScreenFixture::go_to_trajectories(&view, cx);
    view.assert_that(cx).is_on_trajectories();
}

#[gpui::test]
fn test_rapid_tab_switching(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Rapidly switch tabs back and forth
    for _ in 0..5 {
        GymScreenFixture::go_to_tbcc(&view, cx);
        GymScreenFixture::go_to_hillclimber(&view, cx);
        GymScreenFixture::go_to_testgen(&view, cx);
        GymScreenFixture::go_to_trajectories(&view, cx);
    }

    // Should end on Trajectories
    view.assert_that(cx).is_on_trajectories();
}

#[gpui::test]
fn test_switch_to_same_tab_is_idempotent(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    GymScreenFixture::go_to_tbcc(&view, cx);
    view.assert_that(cx).is_on_tbcc();

    // Switching to same tab should be fine
    GymScreenFixture::go_to_tbcc(&view, cx);
    view.assert_that(cx).is_on_tbcc();

    // Multiple times
    for _ in 0..10 {
        GymScreenFixture::go_to_tbcc(&view, cx);
    }
    view.assert_that(cx).is_on_tbcc();
}

// ============================================================================
// Sidebar + Tab Integration Tests
// ============================================================================

#[gpui::test]
fn test_sidebar_state_persists_across_tabs(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Collapse sidebar
    GymScreenFixture::toggle_sidebar(&view, cx);
    view.assert_that(cx).sidebar_is_collapsed();

    // Switch through tabs
    GymScreenFixture::go_to_tbcc(&view, cx);
    view.assert_that(cx).sidebar_is_collapsed();

    GymScreenFixture::go_to_hillclimber(&view, cx);
    view.assert_that(cx).sidebar_is_collapsed();

    GymScreenFixture::go_to_testgen(&view, cx);
    view.assert_that(cx).sidebar_is_collapsed();

    // Sidebar should still be collapsed
    view.assert_that(cx).sidebar_is_collapsed();
}

#[gpui::test]
fn test_sidebar_can_toggle_on_any_tab(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // On TBCC tab
    GymScreenFixture::go_to_tbcc(&view, cx);
    GymScreenFixture::toggle_sidebar(&view, cx);
    view.assert_that(cx).sidebar_is_collapsed();
    GymScreenFixture::toggle_sidebar(&view, cx);
    view.assert_that(cx).sidebar_is_expanded();

    // On HillClimber tab
    GymScreenFixture::go_to_hillclimber(&view, cx);
    GymScreenFixture::toggle_sidebar(&view, cx);
    view.assert_that(cx).sidebar_is_collapsed();
    GymScreenFixture::toggle_sidebar(&view, cx);
    view.assert_that(cx).sidebar_is_expanded();

    // On TestGen tab
    GymScreenFixture::go_to_testgen(&view, cx);
    GymScreenFixture::toggle_sidebar(&view, cx);
    view.assert_that(cx).sidebar_is_collapsed();
    GymScreenFixture::toggle_sidebar(&view, cx);
    view.assert_that(cx).sidebar_is_expanded();
}

// ============================================================================
// Tab All Iterator Tests
// ============================================================================

#[gpui::test]
fn test_gym_tab_all_contains_four_tabs() {
    let tabs = GymTab::all();
    assert_eq!(tabs.len(), 4, "Should have 4 tabs");
    assert!(tabs.contains(&GymTab::Trajectories));
    assert!(tabs.contains(&GymTab::TBCC));
    assert!(tabs.contains(&GymTab::HillClimber));
    assert!(tabs.contains(&GymTab::TestGen));
}

#[gpui::test]
fn test_gym_tab_labels() {
    assert_eq!(GymTab::Trajectories.label(), "Trajectories");
    assert_eq!(GymTab::TBCC.label(), "TBCC");
    assert_eq!(GymTab::HillClimber.label(), "HillClimber");
    assert_eq!(GymTab::TestGen.label(), "TestGen");
}

#[gpui::test]
fn test_default_gym_tab_is_trajectories() {
    let default_tab = GymTab::default();
    assert_eq!(default_tab, GymTab::Trajectories);
}

// ============================================================================
// State Isolation Tests
// ============================================================================

#[gpui::test]
fn test_multiple_gym_screens_are_independent(cx: &mut TestAppContext) {
    let view1 = GymScreenFixture::create(cx);
    let view2 = GymScreenFixture::create(cx);

    // Both start on Trajectories
    view1.assert_that(cx).is_on_trajectories();
    view2.assert_that(cx).is_on_trajectories();

    // Change view1 to TBCC
    GymScreenFixture::go_to_tbcc(&view1, cx);

    // view1 should be on TBCC, view2 should still be on Trajectories
    view1.assert_that(cx).is_on_tbcc();
    view2.assert_that(cx).is_on_trajectories();

    // Collapse sidebar on view2
    GymScreenFixture::toggle_sidebar(&view2, cx);

    // view2 sidebar collapsed, view1 sidebar still expanded
    view1.assert_that(cx).sidebar_is_expanded();
    view2.assert_that(cx).sidebar_is_collapsed();
}
