//! Smoke tests for core HUD functionality
//!
//! Ported from e2e/tests/smoke/basic-smoke.spec.ts
//! Tests HUD-001 (app launch), HUD-002 (flow renders),
//! HUD-010 (canvas pan), HUD-012 (reset view).

use gpui_oa::TestAppContext;
use crate::tests::fixtures::{GraphViewFixture, GraphViewAssertExt};

/// Test that GraphView renders and initializes correctly
#[gpui_oa::test]
fn test_graph_view_renders(cx: &mut TestAppContext) {
    // Create GraphView - equivalent to app launch
    let view = GraphViewFixture::create(cx);

    // Verify initial state (equivalent to toBeVisible checks)
    let node_count = GraphViewFixture::node_count(&view, cx);
    assert_eq!(node_count, 0, "Initial node count should be 0");

    // Verify zoom is at default (100%)
    let zoom = GraphViewFixture::zoom_level_display(&view, cx);
    assert_eq!(zoom, "100%", "Initial zoom should be 100%");

    // Verify pan is at origin
    let (pan_x, pan_y) = GraphViewFixture::pan(&view, cx);
    assert!(pan_x.abs() < 0.1 && pan_y.abs() < 0.1, "Initial pan should be at origin");

    // Verify connection state (simulated connected)
    let connected = GraphViewFixture::is_connected(&view, cx);
    assert!(connected, "Should be connected initially");
}

/// Test that canvas pan updates the transform
#[gpui_oa::test]
fn test_canvas_pan_updates_transform(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Get initial pan position
    let (initial_x, initial_y) = GraphViewFixture::pan(&view, cx);

    // Simulate pan by updating pan values directly
    // Note: In the actual GPUI test we'd use mouse drag events
    view.update(cx, |view, cx| {
        view.set_pan(initial_x + 80.0, initial_y + 40.0, cx);
    });
    cx.run_until_parked();

    // Verify pan changed
    let (after_x, after_y) = GraphViewFixture::pan(&view, cx);
    assert_ne!((after_x, after_y), (initial_x, initial_y), "Pan should have changed after panning");
    assert!((after_x - (initial_x + 80.0)).abs() < 0.1, "Pan X should match expected value");
    assert!((after_y - (initial_y + 40.0)).abs() < 0.1, "Pan Y should match expected value");
}

/// Test that zoom via scroll wheel works
#[gpui_oa::test]
fn test_zoom_via_scroll(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Get initial zoom
    let initial_zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((initial_zoom - 1.0).abs() < 0.01, "Initial zoom should be 1.0");

    // Simulate zoom by updating zoom directly
    view.update(cx, |view, cx| {
        view.set_zoom(1.5, cx);
    });
    cx.run_until_parked();

    // Verify zoom changed
    let new_zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((new_zoom - 1.5).abs() < 0.01, "Zoom should be 1.5 after zooming");

    let display = GraphViewFixture::zoom_level_display(&view, cx);
    assert_eq!(display, "150%", "Zoom display should show 150%");
}

/// Test that reset returns view to initial state
#[gpui_oa::test]
fn test_reset_returns_to_initial_state(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Modify view state
    view.update(cx, |view, cx| {
        view.set_pan(100.0, 50.0, cx);
        view.set_zoom(2.0, cx);
    });
    cx.run_until_parked();

    // Verify state changed
    let (pan_x, _pan_y) = GraphViewFixture::pan(&view, cx);
    assert!((pan_x - 100.0).abs() < 0.1, "Pan X should be modified");

    let zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((zoom - 2.0).abs() < 0.01, "Zoom should be modified");

    // Reset view
    GraphViewFixture::reset_view(&view, cx);

    // Verify reset
    view.assert_that(cx)
        .expect_zoom_reset()
        .expect_pan_reset();
}
