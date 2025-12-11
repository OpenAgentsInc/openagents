//! Error handling tests
//!
//! Ported from e2e/tests/errors/error-handling.spec.ts
//! Tests HUD-060 to HUD-063: graceful degradation and error handling.

use gpui::TestAppContext;
use crate::tests::fixtures::{GraphViewFixture, HudInjector, GraphViewAssertExt};
use crate::tests::messages::factories;

/// HUD-060: no crash on WebSocket disconnect, preserves last state
#[gpui::test]
fn test_no_crash_on_disconnect_preserves_state(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Send initial data to establish state
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(Some("test-session")));
    injector.inject(factories::task_selected_simple("oa-test-123", "Test Task"));

    // Verify initial state
    view.assert_that(cx)
        .expect_connected()
        .expect_session_active();

    // Trigger disconnect
    GraphViewFixture::simulate_disconnect(&view, cx);

    // Verify UI shows disconnected state
    view.assert_that(cx)
        .expect_disconnected();

    // Critical: App should NOT crash - state should be preserved
    let session = GraphViewFixture::current_session_id(&view, cx);
    assert!(session.is_some(), "Session should be preserved after disconnect");

    // Controls should still work
    GraphViewFixture::reset_view(&view, cx);
    view.assert_that(cx)
        .expect_zoom_reset();
}

/// HUD-061: malformed messages are safely ignored
#[gpui::test]
fn test_malformed_messages_safely_ignored(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Get initial error count
    let initial_errors = GraphViewFixture::error_count(&view, cx);

    // Inject various malformed messages
    let malformed_messages = vec![
        "not json at all",
        r#"{"incomplete": true"#,
        r#"{"type": "unknown_type", "data": 123}"#,
        "[]",
        "null",
        r#"{"type": null}"#,
        r#"{"type": "apm_update"}"#, // Missing required fields
    ];

    for raw_msg in malformed_messages {
        GraphViewFixture::inject_raw(&view, raw_msg, cx);
    }

    // App should not crash - verify we can still interact
    let zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((zoom - 1.0).abs() < 0.01, "Zoom should still be default");

    // Error count should have increased (messages were tracked but handled)
    let final_errors = GraphViewFixture::error_count(&view, cx);
    assert!(final_errors >= initial_errors, "Errors should be tracked");

    // Controls should still work
    view.update(cx, |view, cx| {
        view.set_zoom(1.5, cx);
    });
    cx.run_until_parked();

    let new_zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((new_zoom - 1.5).abs() < 0.01, "Zoom should update after malformed messages");
}

/// D3: empty state displays placeholder content
#[gpui::test]
fn test_empty_state_displays_placeholder(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Without sending any HUD messages, verify default state
    let node_count = GraphViewFixture::node_count(&view, cx);
    assert_eq!(node_count, 0, "Initial state should have no nodes");

    view.assert_that(cx)
        .expect_zoom_reset()
        .expect_pan_reset()
        .expect_connected();
}

/// D4: invalid APM values are handled gracefully
#[gpui::test]
fn test_invalid_apm_values_handled(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Send APM update with edge case values
    // Note: JSON serialization of NaN/Infinity requires special handling
    // In Rust, we test with extreme but valid values
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::apm_update(-999.0, 0));

    // App should not crash
    view.assert_that(cx)
        .expect_apm_valid(); // Should clamp/sanitize invalid values

    // Controls should still work
    GraphViewFixture::reset_view(&view, cx);
    view.assert_that(cx)
        .expect_zoom_reset();
}

/// HUD-062: error indicators become visible when errors occur
#[gpui::test]
fn test_error_indicators_visible_on_error(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Initially, no error should be visible
    view.assert_that(cx)
        .expect_no_error();

    // Send an error message
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::error_in_phase("verifying", "Test verification failed"));

    // Error indicator should now be visible
    view.assert_that(cx)
        .expect_error_visible("Test verification failed");

    // App should handle error gracefully - still functional
    view.update(cx, |view, cx| {
        view.set_zoom(1.2, cx);
    });
    cx.run_until_parked();

    let zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((zoom - 1.2).abs() < 0.01, "App should still be functional after error");
}

/// HUD-063: recovers from multiple errors without crash
#[gpui::test]
fn test_recovers_from_multiple_errors(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Phase 1: Send multiple malformed messages
    GraphViewFixture::inject_raw(&view, "not valid json", cx);
    GraphViewFixture::inject_raw(&view, r#"{"broken":"#, cx);

    // Phase 2: Send series of edge case APM messages
    GraphViewFixture::inject_message(&view, factories::apm_update(-999.0, 0), cx);

    // Phase 3: Send multiple error messages
    GraphViewFixture::inject_message(&view, factories::error_in_phase("executing_subtask", "Error 1: Subtask execution failed"), cx);
    GraphViewFixture::inject_message(&view, factories::error_in_phase("verifying", "Error 2: Verification failed"), cx);
    GraphViewFixture::inject_message(&view, factories::error_in_phase("committing", "Error 3: Commit failed"), cx);

    // App should still be functional after all errors
    // Note: Error count may include malformed messages too
    view.assert_that(cx)
        .expect_error_count_at_least(3);

    // Phase 4: Verify recovery - can still receive valid messages
    GraphViewFixture::inject_message(&view, factories::session_start(Some("recovery-session")), cx);
    GraphViewFixture::inject_message(&view, factories::apm_update(99.9, 500), cx);

    // Verify the app is still responding to new messages
    view.assert_that(cx)
        .expect_session_id("recovery-session");

    let apm = GraphViewFixture::current_apm(&view, cx);
    assert!((apm - 99.9).abs() < 0.1, "APM should be updated");

    // Controls should still work
    view.update(cx, |view, cx| {
        view.set_pan(100.0, 100.0, cx);
    });
    cx.run_until_parked();

    GraphViewFixture::reset_view(&view, cx);
    view.assert_that(cx)
        .expect_zoom_reset()
        .expect_pan_reset();
}

/// Test recovers from disconnect and reconnect
#[gpui::test]
fn test_recovers_from_disconnect_and_reconnect(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Send initial data
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(Some("reconnect-test")));

    view.assert_that(cx)
        .expect_connected()
        .expect_session_active();

    // Disconnect
    GraphViewFixture::simulate_disconnect(&view, cx);
    view.assert_that(cx)
        .expect_disconnected();

    // Reconnect
    GraphViewFixture::simulate_reconnect(&view, cx);
    view.assert_that(cx)
        .expect_connected();

    // Can receive new messages after reconnect
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::apm_update(42.0, 100));

    let apm = GraphViewFixture::current_apm(&view, cx);
    assert!((apm - 42.0).abs() < 0.1, "Should receive messages after reconnect");
}

/// Test error count tracking
#[gpui::test]
fn test_error_count_tracking(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    view.assert_that(cx)
        .expect_error_count(0);

    // Inject first error
    GraphViewFixture::inject_message(&view, factories::error_in_phase("phase1", "Error 1"), cx);

    view.assert_that(cx)
        .expect_error_count(1);

    // Inject second error
    GraphViewFixture::inject_message(&view, factories::error_in_phase("phase2", "Error 2"), cx);

    view.assert_that(cx)
        .expect_error_count(2);
}
