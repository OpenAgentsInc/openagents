//! GraphViewFixture: Page Object Model for GraphView testing
//!
//! This fixture mirrors the TypeScript MainviewPage class, providing
//! a clean API for interacting with GraphView in tests.
//!
//! Note: This module requires the `test-support` feature of GPUI to be enabled
//! and tests should use the `#[gpui_oa::test]` macro.

use gpui_oa::{Entity, Point, Pixels, px, point};
use crate::{GraphView, GraphStyle};

use super::super::protocol::HudMessage;

// Re-export for convenience
pub use gpui_oa::TestAppContext;

/// Page Object Model for GraphView - equivalent to TypeScript MainviewPage
///
/// This provides a high-level API for testing GraphView components.
/// The actual GPUI test context should be managed by the test function.
pub struct GraphViewFixture;

impl GraphViewFixture {
    /// Create a GraphView in a test window
    ///
    /// Returns the entity that can be used with TestAppContext methods.
    pub fn create(cx: &mut gpui_oa::TestAppContext) -> Entity<GraphView> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| GraphView::new(cx));
        view
    }

    /// Create with custom style
    pub fn create_with_style(style: GraphStyle, cx: &mut gpui_oa::TestAppContext) -> Entity<GraphView> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| {
            let mut gv = GraphView::new(cx);
            gv.set_style(style);
            gv
        });
        view
    }

    // =========================================================================
    // Query helpers (static methods that work with cx and view)
    // =========================================================================

    /// Get current zoom level (1.0 = 100%)
    pub fn zoom_level(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> f32 {
        cx.read(|cx| view.read(cx).zoom())
    }

    /// Get current zoom level as percentage string (e.g., "100%")
    pub fn zoom_level_display(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> String {
        format!("{}%", (Self::zoom_level(view, cx) * 100.0).round() as i32)
    }

    /// Get number of nodes in the graph
    pub fn node_count(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> usize {
        cx.read(|cx| view.read(cx).node_count())
    }

    /// Check if a node exists
    pub fn has_node(view: &Entity<GraphView>, node_id: &str, cx: &gpui_oa::TestAppContext) -> bool {
        cx.read(|cx| view.read(cx).has_node(node_id))
    }

    /// Get node IDs
    pub fn node_ids(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> Vec<String> {
        cx.read(|cx| view.read(cx).node_ids())
    }

    /// Get pan offset
    pub fn pan(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> (f32, f32) {
        cx.read(|cx| {
            let v = view.read(cx);
            (v.pan_x(), v.pan_y())
        })
    }

    /// Get selected node IDs
    pub fn selected_nodes(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> Vec<String> {
        cx.read(|cx| view.read(cx).selected_ids())
    }

    /// Get selection count
    pub fn selection_count(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> usize {
        Self::selected_nodes(view, cx).len()
    }

    /// Check if simulation is running
    pub fn is_simulating(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> bool {
        cx.read(|cx| view.read(cx).is_simulating())
    }

    /// Get current APM value (if tracked)
    pub fn current_apm(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> f64 {
        cx.read(|cx| view.read(cx).current_apm())
    }

    /// Get current error message (if any)
    pub fn current_error(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> Option<String> {
        cx.read(|cx| view.read(cx).current_error())
    }

    /// Get error count
    pub fn error_count(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> usize {
        cx.read(|cx| view.read(cx).error_count())
    }

    /// Get message count (total received)
    pub fn message_count(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> usize {
        cx.read(|cx| view.read(cx).message_count())
    }

    /// Check if connected (simulated state)
    pub fn is_connected(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> bool {
        cx.read(|cx| view.read(cx).is_connected())
    }

    /// Get current session ID
    pub fn current_session_id(view: &Entity<GraphView>, cx: &gpui_oa::TestAppContext) -> Option<String> {
        cx.read(|cx| view.read(cx).current_session_id())
    }

    // =========================================================================
    // Mutation helpers
    // =========================================================================

    /// Reset view to default zoom/pan
    pub fn reset_view(view: &Entity<GraphView>, cx: &mut gpui_oa::TestAppContext) {
        view.update(cx, |view, cx| {
            view.reset_view(cx);
        });
        cx.run_until_parked();
    }

    /// Clear selection
    pub fn clear_selection(view: &Entity<GraphView>, cx: &mut gpui_oa::TestAppContext) {
        view.update(cx, |view, cx| {
            view.clear_selection(cx);
        });
        cx.run_until_parked();
    }

    /// Inject a HUD message
    pub fn inject_message(view: &Entity<GraphView>, message: HudMessage, cx: &mut gpui_oa::TestAppContext) {
        let json = serde_json::to_value(&message).expect("HudMessage should serialize");
        view.update(cx, |view, cx| {
            view.handle_hud_message(json, cx);
        });
        cx.run_until_parked();
    }

    /// Inject raw message data (for error testing)
    pub fn inject_raw(view: &Entity<GraphView>, data: &str, cx: &mut gpui_oa::TestAppContext) {
        let data = data.to_string();
        view.update(cx, |view, cx| {
            view.handle_raw_message(&data, cx);
        });
        cx.run_until_parked();
    }

    /// Simulate disconnect
    pub fn simulate_disconnect(view: &Entity<GraphView>, cx: &mut gpui_oa::TestAppContext) {
        view.update(cx, |view, cx| {
            view.handle_disconnect(cx);
        });
        cx.run_until_parked();
    }

    /// Simulate reconnect
    pub fn simulate_reconnect(view: &Entity<GraphView>, cx: &mut gpui_oa::TestAppContext) {
        view.update(cx, |view, cx| {
            view.handle_reconnect(cx);
        });
        cx.run_until_parked();
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Get default canvas center
    pub fn canvas_center() -> Point<Pixels> {
        point(px(400.0), px(300.0))
    }
}
