//! Smoke tests for GymScreen
//!
//! Basic tests to verify GymScreen renders and initializes correctly.

use gpui::TestAppContext;
use crate::tests::fixtures::{GymScreenFixture, GymScreenAssertExt};
use crate::tests::fixtures::types::GymTab;

#[gpui::test]
fn test_gym_screen_renders(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Should start with Trajectories tab (default)
    view.assert_that(cx)
        .is_on_trajectories()
        .sidebar_is_expanded();
}

#[gpui::test]
fn test_gym_screen_default_tab_is_trajectories(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    let tab = GymScreenFixture::current_tab(&view, cx);
    assert_eq!(tab, GymTab::Trajectories, "Default tab should be Trajectories");
}

#[gpui::test]
fn test_gym_screen_sidebar_starts_expanded(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    let collapsed = GymScreenFixture::sidebar_collapsed(&view, cx);
    assert!(!collapsed, "Sidebar should start expanded");
}

#[gpui::test]
fn test_gym_screen_sidebar_toggle(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Initially expanded
    assert!(!GymScreenFixture::sidebar_collapsed(&view, cx));

    // Toggle to collapsed
    GymScreenFixture::toggle_sidebar(&view, cx);
    assert!(GymScreenFixture::sidebar_collapsed(&view, cx));

    // Toggle back to expanded
    GymScreenFixture::toggle_sidebar(&view, cx);
    assert!(!GymScreenFixture::sidebar_collapsed(&view, cx));
}

#[gpui::test]
fn test_gym_screen_tab_switching(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Start on Trajectories
    view.assert_that(cx).is_on_trajectories();

    // Switch to TBCC
    GymScreenFixture::go_to_tbcc(&view, cx);
    view.assert_that(cx).is_on_tbcc();

    // Switch to HillClimber
    GymScreenFixture::go_to_hillclimber(&view, cx);
    view.assert_that(cx).is_on_hillclimber();

    // Switch to TestGen
    GymScreenFixture::go_to_testgen(&view, cx);
    view.assert_that(cx).is_on_testgen();

    // Switch back to Trajectories
    GymScreenFixture::go_to_trajectories(&view, cx);
    view.assert_that(cx).is_on_trajectories();
}

#[gpui::test]
fn test_gym_screen_switch_to_all_tabs(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    for tab in GymTab::all() {
        GymScreenFixture::switch_tab(&view, *tab, cx);
        let current = GymScreenFixture::current_tab(&view, cx);
        assert_eq!(current, *tab, "Tab should switch to {:?}", tab);
    }
}
