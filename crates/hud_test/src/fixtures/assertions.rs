//! Assertion helpers for HUD testing
//!
//! Provides fluent assertions for GraphView state verification,
//! equivalent to TypeScript SVGFlowAssertions.

use gpui::{Entity, TestAppContext};
use hud::GraphView;

/// Assertion helpers for GraphView - equivalent to TypeScript SVGFlowAssertions
pub struct GraphAssertions<'a> {
    view: &'a Entity<GraphView>,
    cx: &'a TestAppContext,
}

impl<'a> GraphAssertions<'a> {
    /// Create new assertions for a GraphView entity
    pub fn new(view: &'a Entity<GraphView>, cx: &'a TestAppContext) -> Self {
        Self { view, cx }
    }

    // =========================================================================
    // Node Assertions
    // =========================================================================

    /// Assert a node exists
    pub fn expect_node_exists(&self, node_id: &str) -> &Self {
        let exists = self.cx.read(|cx| self.view.read(cx).has_node(node_id));
        assert!(exists, "Expected node '{}' to exist", node_id);
        self
    }

    /// Assert a node does not exist
    pub fn expect_node_not_exists(&self, node_id: &str) -> &Self {
        let exists = self.cx.read(|cx| self.view.read(cx).has_node(node_id));
        assert!(!exists, "Expected node '{}' to not exist", node_id);
        self
    }

    /// Assert node count equals expected
    pub fn expect_node_count(&self, expected: usize) -> &Self {
        let count = self.cx.read(|cx| self.view.read(cx).node_count());
        assert_eq!(count, expected, "Expected {} nodes, found {}", expected, count);
        self
    }

    /// Assert node count is at least minimum
    pub fn expect_node_count_at_least(&self, minimum: usize) -> &Self {
        let count = self.cx.read(|cx| self.view.read(cx).node_count());
        assert!(
            count >= minimum,
            "Expected at least {} nodes, found {}",
            minimum,
            count
        );
        self
    }

    // =========================================================================
    // Selection Assertions
    // =========================================================================

    /// Assert node is selected
    pub fn expect_node_selected(&self, node_id: &str) -> &Self {
        let selected = self.cx.read(|cx| self.view.read(cx).is_node_selected(node_id));
        assert!(selected, "Expected node '{}' to be selected", node_id);
        self
    }

    /// Assert node is not selected
    pub fn expect_node_not_selected(&self, node_id: &str) -> &Self {
        let selected = self.cx.read(|cx| self.view.read(cx).is_node_selected(node_id));
        assert!(!selected, "Expected node '{}' to not be selected", node_id);
        self
    }

    /// Assert selection count
    pub fn expect_selection_count(&self, expected: usize) -> &Self {
        let count = self.cx.read(|cx| self.view.read(cx).selection_count());
        assert_eq!(
            count, expected,
            "Expected {} selected, found {}",
            expected, count
        );
        self
    }

    /// Assert selection is empty
    pub fn expect_no_selection(&self) -> &Self {
        self.expect_selection_count(0)
    }

    // =========================================================================
    // View State Assertions
    // =========================================================================

    /// Assert zoom level within tolerance
    pub fn expect_zoom_level(&self, expected: f32, tolerance: f32) -> &Self {
        let zoom = self.cx.read(|cx| self.view.read(cx).zoom());
        assert!(
            (zoom - expected).abs() <= tolerance,
            "Expected zoom {} +/- {}, found {}",
            expected,
            tolerance,
            zoom
        );
        self
    }

    /// Assert zoom is at 100% (1.0)
    pub fn expect_zoom_reset(&self) -> &Self {
        self.expect_zoom_level(1.0, 0.01)
    }

    /// Assert pan is at origin
    pub fn expect_pan_reset(&self) -> &Self {
        let (pan_x, pan_y) = self.cx.read(|cx| {
            let v = self.view.read(cx);
            (v.pan_x(), v.pan_y())
        });
        assert!(
            pan_x.abs() < 0.1 && pan_y.abs() < 0.1,
            "Expected pan at origin, found ({}, {})",
            pan_x,
            pan_y
        );
        self
    }

    /// Assert pan has changed from origin
    pub fn expect_pan_changed(&self) -> &Self {
        let (pan_x, pan_y) = self.cx.read(|cx| {
            let v = self.view.read(cx);
            (v.pan_x(), v.pan_y())
        });
        assert!(
            pan_x.abs() > 0.1 || pan_y.abs() > 0.1,
            "Expected pan to have changed, but still at origin ({}, {})",
            pan_x,
            pan_y
        );
        self
    }

    // =========================================================================
    // APM Assertions
    // =========================================================================

    /// Assert APM value within tolerance
    pub fn expect_apm_value(&self, expected_apm: f64, tolerance: f64) -> &Self {
        let apm = self.cx.read(|cx| self.view.read(cx).current_apm());
        assert!(
            (apm - expected_apm).abs() <= tolerance,
            "Expected APM {} +/- {}, found {}",
            expected_apm,
            tolerance,
            apm
        );
        self
    }

    /// Assert APM is positive
    pub fn expect_apm_positive(&self) -> &Self {
        let apm = self.cx.read(|cx| self.view.read(cx).current_apm());
        assert!(apm > 0.0, "Expected positive APM, found {}", apm);
        self
    }

    /// Assert APM is valid (not NaN or Infinity)
    pub fn expect_apm_valid(&self) -> &Self {
        let apm = self.cx.read(|cx| self.view.read(cx).current_apm());
        assert!(
            apm.is_finite(),
            "Expected valid APM, found {} (not finite)",
            apm
        );
        self
    }

    // =========================================================================
    // Error State Assertions
    // =========================================================================

    /// Assert error is visible with expected message
    pub fn expect_error_visible(&self, expected_message: &str) -> &Self {
        let error = self.cx.read(|cx| self.view.read(cx).current_error());
        match error {
            Some(msg) => assert!(
                msg.contains(expected_message),
                "Expected error containing '{}', found '{}'",
                expected_message,
                msg
            ),
            None => panic!(
                "Expected error '{}' but no error displayed",
                expected_message
            ),
        }
        self
    }

    /// Assert no error is displayed
    pub fn expect_no_error(&self) -> &Self {
        let error = self.cx.read(|cx| self.view.read(cx).current_error());
        assert!(error.is_none(), "Expected no error, found {:?}", error);
        self
    }

    /// Assert error count
    pub fn expect_error_count(&self, expected: usize) -> &Self {
        let count = self.cx.read(|cx| self.view.read(cx).error_count());
        assert_eq!(
            count, expected,
            "Expected {} errors, found {}",
            expected, count
        );
        self
    }

    // =========================================================================
    // Connection State Assertions
    // =========================================================================

    /// Assert connected state
    pub fn expect_connected(&self) -> &Self {
        let connected = self.cx.read(|cx| self.view.read(cx).is_connected());
        assert!(connected, "Expected to be connected");
        self
    }

    /// Assert disconnected state
    pub fn expect_disconnected(&self) -> &Self {
        let connected = self.cx.read(|cx| self.view.read(cx).is_connected());
        assert!(!connected, "Expected to be disconnected");
        self
    }

    // =========================================================================
    // Session Assertions
    // =========================================================================

    /// Assert active session
    pub fn expect_session_active(&self) -> &Self {
        let session = self.cx.read(|cx| self.view.read(cx).current_session_id());
        assert!(session.is_some(), "Expected active session");
        self
    }

    /// Assert no active session
    pub fn expect_no_session(&self) -> &Self {
        let session = self.cx.read(|cx| self.view.read(cx).current_session_id());
        assert!(session.is_none(), "Expected no active session");
        self
    }

    /// Assert session ID matches
    pub fn expect_session_id(&self, expected: &str) -> &Self {
        let session = self.cx.read(|cx| self.view.read(cx).current_session_id());
        assert_eq!(
            session,
            Some(expected.to_string()),
            "Expected session '{}', found {:?}",
            expected,
            session
        );
        self
    }

    // =========================================================================
    // Message Count Assertions
    // =========================================================================

    /// Assert message count
    pub fn expect_message_count(&self, expected: usize) -> &Self {
        let count = self.cx.read(|cx| self.view.read(cx).message_count());
        assert_eq!(
            count, expected,
            "Expected {} messages, found {}",
            expected, count
        );
        self
    }

    /// Assert at least N messages received
    pub fn expect_messages_at_least(&self, minimum: usize) -> &Self {
        let count = self.cx.read(|cx| self.view.read(cx).message_count());
        assert!(
            count >= minimum,
            "Expected at least {} messages, found {}",
            minimum,
            count
        );
        self
    }
}

/// Extension trait for fluent assertions on GraphView entities
pub trait GraphViewAssertExt {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> GraphAssertions<'a>;
}

impl GraphViewAssertExt for Entity<GraphView> {
    fn assert_that<'a>(&'a self, cx: &'a TestAppContext) -> GraphAssertions<'a> {
        GraphAssertions::new(self, cx)
    }
}
