//! TBCC component tests
//!
//! Tests for the TerminalBench Command Center components.

use gpui::TestAppContext;
use crate::tests::fixtures::{TBCCFixture, TBCCAssertExt};
use crate::tests::fixtures::types::TBCCTab;

// ============================================================================
// TBCCScreen Tests
// ============================================================================

#[gpui::test]
fn test_tbcc_screen_renders(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    // Should start with Dashboard tab
    view.assert_that(cx).is_on_dashboard();
}

#[gpui::test]
fn test_tbcc_screen_default_tab_is_dashboard(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    let tab = TBCCFixture::current_tab(&view, cx);
    assert_eq!(tab, TBCCTab::Dashboard, "Default tab should be Dashboard");
}

#[gpui::test]
fn test_tbcc_screen_tab_switching(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    // Start on Dashboard
    view.assert_that(cx).is_on_dashboard();

    // Switch to Tasks
    TBCCFixture::go_to_tasks(&view, cx);
    view.assert_that(cx).is_on_tasks();

    // Switch to Runs
    TBCCFixture::go_to_runs(&view, cx);
    view.assert_that(cx).is_on_runs();

    // Switch to Settings
    TBCCFixture::go_to_settings(&view, cx);
    view.assert_that(cx).is_on_settings();

    // Switch back to Dashboard
    TBCCFixture::go_to_dashboard(&view, cx);
    view.assert_that(cx).is_on_dashboard();
}

#[gpui::test]
fn test_tbcc_screen_switch_to_all_tabs(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    for tab in TBCCTab::all() {
        TBCCFixture::switch_tab(&view, *tab, cx);
        let current = TBCCFixture::current_tab(&view, cx);
        assert_eq!(current, *tab, "Tab should switch to {:?}", tab);
    }
}

// ============================================================================
// Dashboard Tab Tests
// ============================================================================

#[gpui::test]
fn test_tbcc_dashboard_tab(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    TBCCFixture::go_to_dashboard(&view, cx);
    assert!(TBCCFixture::is_on_dashboard(&view, cx));
}

// ============================================================================
// Tasks Tab Tests
// ============================================================================

#[gpui::test]
fn test_tbcc_tasks_tab(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    TBCCFixture::go_to_tasks(&view, cx);
    assert!(TBCCFixture::is_on_tasks(&view, cx));
}

// ============================================================================
// Runs Tab Tests
// ============================================================================

#[gpui::test]
fn test_tbcc_runs_tab(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    TBCCFixture::go_to_runs(&view, cx);
    assert!(TBCCFixture::is_on_runs(&view, cx));
}

// ============================================================================
// Settings Tab Tests
// ============================================================================

#[gpui::test]
fn test_tbcc_settings_tab(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    TBCCFixture::go_to_settings(&view, cx);
    assert!(TBCCFixture::is_on_settings(&view, cx));
}

// ============================================================================
// Tab Persistence Tests
// ============================================================================

#[gpui::test]
fn test_tbcc_tab_state_persists(cx: &mut TestAppContext) {
    let view = TBCCFixture::create(cx);

    // Switch to Settings
    TBCCFixture::go_to_settings(&view, cx);
    view.assert_that(cx).is_on_settings();

    // Verify state persists
    let tab = TBCCFixture::current_tab(&view, cx);
    assert_eq!(tab, TBCCTab::Settings);
}
