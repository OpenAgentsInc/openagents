//! Integration tests for Gym components
//!
//! Tests for cross-component interactions and navigation flows.

use gpui::TestAppContext;
use crate::tests::fixtures::{GymScreenFixture, GymScreenAssertExt};
use crate::tests::fixtures::types::GymTab;

// ============================================================================
// Tab Navigation Integration Tests
// ============================================================================

#[gpui::test]
fn test_navigate_through_all_tabs_sequentially(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Start at RegexCrusade (default), then navigate through tabs
    GymScreenFixture::go_to_trajectories(&view, cx);
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
// Tab All Iterator Tests
// ============================================================================

#[gpui::test]
fn test_gym_tab_all_contains_five_tabs() {
    let tabs = GymTab::all();
    assert_eq!(tabs.len(), 5, "Should have 5 tabs");
    assert!(tabs.contains(&GymTab::Trajectories));
    assert!(tabs.contains(&GymTab::TBCC));
    assert!(tabs.contains(&GymTab::HillClimber));
    assert!(tabs.contains(&GymTab::TestGen));
    assert!(tabs.contains(&GymTab::RegexCrusade));
}

#[gpui::test]
fn test_gym_tab_labels() {
    assert_eq!(GymTab::Trajectories.label(), "Trajectories");
    assert_eq!(GymTab::TBCC.label(), "TBCC");
    assert_eq!(GymTab::HillClimber.label(), "HillClimber");
    assert_eq!(GymTab::TestGen.label(), "TestGen");
    assert_eq!(GymTab::RegexCrusade.label(), "Crusade");
}

#[gpui::test]
fn test_default_gym_tab_is_regex_crusade() {
    let default_tab = GymTab::default();
    assert_eq!(default_tab, GymTab::RegexCrusade);
}

// ============================================================================
// State Isolation Tests
// ============================================================================

#[gpui::test]
fn test_multiple_gym_screens_are_independent(cx: &mut TestAppContext) {
    let view1 = GymScreenFixture::create(cx);
    let view2 = GymScreenFixture::create(cx);

    // Both start on RegexCrusade (default)
    view1.assert_that(cx).has_tab(GymTab::RegexCrusade);
    view2.assert_that(cx).has_tab(GymTab::RegexCrusade);

    // Change view1 to TBCC
    GymScreenFixture::go_to_tbcc(&view1, cx);

    // view1 should be on TBCC, view2 should still be on RegexCrusade
    view1.assert_that(cx).is_on_tbcc();
    view2.assert_that(cx).has_tab(GymTab::RegexCrusade);
}
