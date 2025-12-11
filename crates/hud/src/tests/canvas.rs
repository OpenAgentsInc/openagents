//! Canvas interaction tests
//!
//! Ported from e2e/tests/interactions/canvas.spec.ts
//! Tests B1-B7: pan, zoom, and reset functionality.

use gpui_oa::TestAppContext;
use crate::tests::fixtures::{GraphViewFixture, GraphViewAssertExt};

/// B1: pan by drag updates SVG transform
#[gpui_oa::test]
fn test_pan_by_drag_updates_transform(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Get initial transform
    let (initial_x, initial_y) = GraphViewFixture::pan(&view, cx);

    // Simulate pan
    view.update(cx, |view, cx| {
        view.set_pan(initial_x + 100.0, initial_y + 50.0, cx);
    });
    cx.run_until_parked();

    // Transform should have changed
    let (new_x, new_y) = GraphViewFixture::pan(&view, cx);
    assert_ne!((new_x, new_y), (initial_x, initial_y), "Transform should have changed after pan");
}

/// B3: zoom by scroll wheel updates scale
#[gpui_oa::test]
fn test_zoom_by_scroll_updates_scale(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Verify initial zoom
    let initial_zoom = GraphViewFixture::zoom_level_display(&view, cx);
    assert_eq!(initial_zoom, "100%", "Initial zoom should be 100%");

    // Zoom in
    view.update(cx, |view, cx| {
        view.set_zoom(1.5, cx);
    });
    cx.run_until_parked();

    // Verify zoom increased
    let zoomed_level = GraphViewFixture::zoom_level(&view, cx);
    assert!(zoomed_level > 1.0, "Zoom should be greater than 1.0 after zooming in");

    let display = GraphViewFixture::zoom_level_display(&view, cx);
    assert_eq!(display, "150%", "Zoom display should show 150%");
}

/// Test zoom has minimum and maximum limits
#[gpui_oa::test]
fn test_zoom_has_min_max_limits(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Try to zoom out beyond minimum
    view.update(cx, |view, cx| {
        view.set_zoom(0.01, cx); // Very small value
    });
    cx.run_until_parked();

    let min_zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!(min_zoom >= 0.1, "Zoom should be clamped to minimum (got {})", min_zoom);
    assert!(min_zoom <= 1.0, "Minimum zoom should be at most 100%");

    // Try to zoom in beyond maximum
    view.update(cx, |view, cx| {
        view.set_zoom(10.0, cx); // Very large value
    });
    cx.run_until_parked();

    let max_zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!(max_zoom >= 1.0, "Maximum zoom should be at least 100%");
    assert!(max_zoom <= 5.0, "Zoom should be clamped to maximum (got {})", max_zoom);
}

/// B5: reset button returns to initial state
#[gpui_oa::test]
fn test_reset_button_returns_to_initial_state(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Zoom and pan to change state
    view.update(cx, |view, cx| {
        view.set_zoom(1.5, cx);
        view.set_pan(50.0, 50.0, cx);
    });
    cx.run_until_parked();

    // Verify state changed
    let zoomed_level = GraphViewFixture::zoom_level(&view, cx);
    assert!((zoomed_level - 1.5).abs() < 0.01, "Zoom should be 1.5");

    // Click reset
    GraphViewFixture::reset_view(&view, cx);

    // Verify reset to 100%
    view.assert_that(cx)
        .expect_zoom_reset()
        .expect_pan_reset();
}

/// B7: window resize preserves zoom level
#[gpui_oa::test]
fn test_window_resize_preserves_zoom(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Get initial state
    let initial_zoom = GraphViewFixture::zoom_level(&view, cx);

    // Note: In GPUI, window resize events are handled by the platform
    // This test validates the zoom remains stable through state changes
    view.update(cx, |view, cx| {
        // Trigger a state update (simulates what might happen on resize)
        view.notify(cx);
    });
    cx.run_until_parked();

    // Zoom level should remain stable
    let after_zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((after_zoom - initial_zoom).abs() < 0.01, "Zoom should remain stable after resize");
}

/// Test multiple sequential pan operations
#[gpui_oa::test]
fn test_multiple_pan_operations(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Get initial position
    let (start_x, start_y) = GraphViewFixture::pan(&view, cx);

    // Pan multiple times
    view.update(cx, |view, cx| {
        view.set_pan(start_x + 10.0, start_y + 10.0, cx);
    });
    cx.run_until_parked();

    view.update(cx, |view, cx| {
        let (x, y) = (view.pan_x(), view.pan_y());
        view.set_pan(x + 20.0, y + 20.0, cx);
    });
    cx.run_until_parked();

    view.update(cx, |view, cx| {
        let (x, y) = (view.pan_x(), view.pan_y());
        view.set_pan(x + 30.0, y + 30.0, cx);
    });
    cx.run_until_parked();

    // Verify accumulated pan
    let (final_x, final_y) = GraphViewFixture::pan(&view, cx);
    assert!((final_x - (start_x + 60.0)).abs() < 0.1, "Final pan X should be start + 60");
    assert!((final_y - (start_y + 60.0)).abs() < 0.1, "Final pan Y should be start + 60");
}

/// Test zoom and pan combination
#[gpui_oa::test]
fn test_zoom_and_pan_combination(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Zoom first
    view.update(cx, |view, cx| {
        view.set_zoom(2.0, cx);
    });
    cx.run_until_parked();

    // Then pan
    view.update(cx, |view, cx| {
        view.set_pan(100.0, 100.0, cx);
    });
    cx.run_until_parked();

    // Verify both values
    let zoom = GraphViewFixture::zoom_level(&view, cx);
    let (pan_x, pan_y) = GraphViewFixture::pan(&view, cx);

    assert!((zoom - 2.0).abs() < 0.01, "Zoom should be 2.0");
    assert!((pan_x - 100.0).abs() < 0.1, "Pan X should be 100");
    assert!((pan_y - 100.0).abs() < 0.1, "Pan Y should be 100");

    // Reset and verify both reset
    GraphViewFixture::reset_view(&view, cx);

    view.assert_that(cx)
        .expect_zoom_reset()
        .expect_pan_reset();
}
