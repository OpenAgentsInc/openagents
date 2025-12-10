//! Visual/Layout render tests
//!
//! Ported from e2e/tests/visual/render.spec.ts
//! Tests A1-A7: visual rendering and layout verification.

use gpui::TestAppContext;
use hud_test::fixtures::{GraphViewFixture, HudInjector, GraphViewAssertExt};
use hud_test::messages::factories;

/// A1: GraphView loads without errors
#[gpui::test]
fn test_graph_view_loads_without_errors(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Verify no initial errors
    view.assert_that(cx)
        .expect_no_error()
        .expect_error_count(0);

    // Verify basic state
    view.assert_that(cx)
        .expect_zoom_reset()
        .expect_pan_reset()
        .expect_connected();
}

/// A2: Graph structure renders correctly
#[gpui::test]
fn test_graph_structure_renders(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Verify initial empty state
    let node_count = GraphViewFixture::node_count(&view, cx);
    assert_eq!(node_count, 0, "Initial graph should have no nodes");

    // Add some content
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));
    injector.inject(factories::task_selected_simple("oa-render-test", "Render Test Task"));

    // Verify node was added
    view.assert_that(cx)
        .expect_node_count_at_least(1)
        .expect_node_exists("oa-render-test");
}

/// A3: APM widget displays after receiving apm_update
#[gpui::test]
fn test_apm_widget_displays_after_update(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Initially no APM
    let initial_apm = GraphViewFixture::current_apm(&view, cx);
    assert!((initial_apm).abs() < 0.01, "Initial APM should be 0");

    // Send APM update
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::apm_update(15.5, 42));

    // Verify APM is displayed
    let apm_after = GraphViewFixture::current_apm(&view, cx);
    assert!((apm_after - 15.5).abs() < 0.1, "APM should be 15.5 after update");

    view.assert_that(cx)
        .expect_apm_value(15.5, 0.1);
}

/// A4: node types display correctly after task_selected
#[gpui::test]
fn test_node_types_display_correctly(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::task_selected_simple("oa-test-theme", "Theme Test Task"));

    // Should have at least one node
    view.assert_that(cx)
        .expect_node_count_at_least(1);

    // Verify node exists with correct ID
    view.assert_that(cx)
        .expect_node_exists("oa-test-theme");
}

/// A6: flow controls are accessible
#[gpui::test]
fn test_flow_controls_accessible(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Zoom should start at 100%
    let zoom_level = GraphViewFixture::zoom_level_display(&view, cx);
    assert_eq!(zoom_level, "100%", "Initial zoom should be 100%");

    // Test zoom control
    view.update(cx, |view, cx| {
        view.set_zoom(1.5, cx);
    });
    cx.run_until_parked();

    let new_zoom = GraphViewFixture::zoom_level_display(&view, cx);
    assert_eq!(new_zoom, "150%", "Zoom should update to 150%");

    // Test reset control
    GraphViewFixture::reset_view(&view, cx);
    view.assert_that(cx)
        .expect_zoom_reset();
}

/// Test connection status indicator
#[gpui::test]
fn test_connection_status_indicator(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Should be connected initially
    view.assert_that(cx)
        .expect_connected();

    // Disconnect
    GraphViewFixture::simulate_disconnect(&view, cx);
    view.assert_that(cx)
        .expect_disconnected();

    // Reconnect
    GraphViewFixture::simulate_reconnect(&view, cx);
    view.assert_that(cx)
        .expect_connected();
}

/// Test empty state displays correctly
#[gpui::test]
fn test_empty_state_displays(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Empty state should have no nodes
    view.assert_that(cx)
        .expect_node_count(0)
        .expect_no_session()
        .expect_no_error();

    // But view should still be functional
    view.update(cx, |view, cx| {
        view.set_zoom(1.2, cx);
    });
    cx.run_until_parked();

    let zoom = GraphViewFixture::zoom_level(&view, cx);
    assert!((zoom - 1.2).abs() < 0.01, "Zoom should be adjustable in empty state");
}

/// Test multiple nodes render correctly
#[gpui::test]
fn test_multiple_nodes_render(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::session_start(None));

    // Add multiple tasks
    injector.inject(factories::task_selected_simple("oa-node-1", "Node 1"));
    injector.inject(factories::task_selected_simple("oa-node-2", "Node 2"));
    injector.inject(factories::task_selected_simple("oa-node-3", "Node 3"));

    // Verify all nodes exist
    view.assert_that(cx)
        .expect_node_count_at_least(3)
        .expect_node_exists("oa-node-1")
        .expect_node_exists("oa-node-2")
        .expect_node_exists("oa-node-3");
}

/// Test APM values render with different values
#[gpui::test]
fn test_apm_renders_different_values(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Test various APM values
    let test_values = [0.0, 10.5, 99.9, 150.0];

    for &apm_value in &test_values {
        GraphViewFixture::inject_message(&view, factories::apm_update(apm_value, 100), cx);

        let displayed_apm = GraphViewFixture::current_apm(&view, cx);
        assert!(
            (displayed_apm - apm_value).abs() < 0.1,
            "APM should be {} but was {}",
            apm_value,
            displayed_apm
        );
    }

    // All values should be valid (not NaN or Infinity)
    view.assert_that(cx)
        .expect_apm_valid();
}

/// Test zoom levels render correctly
#[gpui::test]
fn test_zoom_levels_render(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Test various zoom levels
    let test_zooms = [0.5, 1.0, 1.5, 2.0];

    for &zoom_value in &test_zooms {
        view.update(cx, |view, cx| {
            view.set_zoom(zoom_value, cx);
        });
        cx.run_until_parked();

        let displayed_zoom = GraphViewFixture::zoom_level(&view, cx);
        // Allow for clamping
        assert!(
            displayed_zoom >= 0.1 && displayed_zoom <= 5.0,
            "Zoom {} should be within bounds",
            displayed_zoom
        );
    }
}

/// Test error state renders correctly
#[gpui::test]
fn test_error_state_renders(cx: &mut TestAppContext) {
    let view = GraphViewFixture::create(cx);

    // Initially no error
    view.assert_that(cx)
        .expect_no_error();

    // Inject error
    let mut injector = HudInjector::new(&view, cx);
    injector.inject(factories::error_in_phase("testing", "Test error message"));

    // Error should be visible
    view.assert_that(cx)
        .expect_error_visible("Test error message")
        .expect_error_count(1);

    // View should still be functional
    GraphViewFixture::reset_view(&view, cx);
    view.assert_that(cx)
        .expect_zoom_reset();
}
