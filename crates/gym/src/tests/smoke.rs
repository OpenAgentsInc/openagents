//! Smoke tests for GymScreen
//!
//! Basic tests to verify GymScreen renders and initializes correctly.

use gpui_oa::TestAppContext;
use crate::tests::fixtures::{GymScreenFixture, GymScreenAssertExt};
use crate::tests::fixtures::types::GymTab;

#[gpui_oa::test]
fn test_gym_screen_renders(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Should start with RegexCrusade tab (default)
    view.assert_that(cx)
        .has_tab(GymTab::RegexCrusade);
}

#[gpui_oa::test]
fn test_gym_screen_tab_switching(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    // Switch to TBCC
    GymScreenFixture::go_to_tbcc(&view, cx);
    view.assert_that(cx).is_on_tbcc();

    // Switch to HillClimber
    GymScreenFixture::go_to_hillclimber(&view, cx);
    view.assert_that(cx).is_on_hillclimber();

    // Switch to TestGen
    GymScreenFixture::go_to_testgen(&view, cx);
    view.assert_that(cx).is_on_testgen();

    // Switch to Trajectories
    GymScreenFixture::go_to_trajectories(&view, cx);
    view.assert_that(cx).is_on_trajectories();
}

#[gpui_oa::test]
fn test_gym_screen_switch_to_all_tabs(cx: &mut TestAppContext) {
    let view = GymScreenFixture::create(cx);

    for tab in GymTab::all() {
        GymScreenFixture::switch_tab(&view, *tab, cx);
        let current = GymScreenFixture::current_tab(&view, cx);
        assert_eq!(current, *tab, "Tab should switch to {:?}", tab);
    }
}
