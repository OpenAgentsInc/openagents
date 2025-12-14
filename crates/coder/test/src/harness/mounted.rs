//! MountedWidget - a widget that has been mounted in a test harness.

use coder_widgets::widget::Widget;
use wgpui::Bounds;

/// A widget that has been mounted in a test harness.
///
/// This provides a convenient API for rendering and interacting
/// with a widget in isolation.
pub struct MountedWidget<W: Widget> {
    /// The widget being tested.
    widget: W,
    /// The bounds of the widget.
    bounds: Bounds,
}

impl<W: Widget> MountedWidget<W> {
    /// Create a new mounted widget.
    pub fn new(widget: W, bounds: Bounds) -> Self {
        Self { widget, bounds }
    }

    /// Get a reference to the widget.
    pub fn widget(&self) -> &W {
        &self.widget
    }

    /// Get a mutable reference to the widget.
    pub fn widget_mut(&mut self) -> &mut W {
        &mut self.widget
    }

    /// Get the bounds of the widget.
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Set new bounds for the widget.
    pub fn set_bounds(&mut self, bounds: Bounds) {
        self.bounds = bounds;
    }

    /// Get the widget's size hint.
    pub fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        self.widget.size_hint()
    }

    /// Get the widget's ID if it has one.
    pub fn id(&self) -> Option<u64> {
        self.widget.id()
    }

    /// Consume this mounted widget and return the inner widget.
    pub fn into_inner(self) -> W {
        self.widget
    }
}

impl<W: Widget + Default> Default for MountedWidget<W> {
    fn default() -> Self {
        Self {
            widget: W::default(),
            bounds: Bounds::new(0.0, 0.0, 800.0, 600.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_widgets::context::{EventContext, PaintContext};
    use coder_widgets::widget::EventResult;
    use wgpui::InputEvent;

    // Simple test widget
    struct TestWidget {
        id: u64,
        paint_count: u32,
    }

    impl TestWidget {
        fn new(id: u64) -> Self {
            Self { id, paint_count: 0 }
        }
    }

    impl Widget for TestWidget {
        fn paint(&mut self, _bounds: Bounds, _cx: &mut PaintContext) {
            self.paint_count += 1;
        }

        fn event(
            &mut self,
            _event: &InputEvent,
            _bounds: Bounds,
            _cx: &mut EventContext,
        ) -> EventResult {
            EventResult::Ignored
        }

        fn id(&self) -> Option<u64> {
            Some(self.id)
        }

        fn size_hint(&self) -> (Option<f32>, Option<f32>) {
            (Some(100.0), Some(50.0))
        }
    }

    #[test]
    fn test_mounted_widget_basic() {
        let widget = TestWidget::new(42);
        let bounds = Bounds::new(10.0, 20.0, 100.0, 50.0);
        let mounted = MountedWidget::new(widget, bounds);

        assert_eq!(mounted.id(), Some(42));
        assert_eq!(mounted.bounds(), bounds);
        assert_eq!(mounted.size_hint(), (Some(100.0), Some(50.0)));
    }

    #[test]
    fn test_mounted_widget_set_bounds() {
        let widget = TestWidget::new(1);
        let mut mounted = MountedWidget::new(widget, Bounds::ZERO);

        let new_bounds = Bounds::new(0.0, 0.0, 200.0, 100.0);
        mounted.set_bounds(new_bounds);

        assert_eq!(mounted.bounds(), new_bounds);
    }

    #[test]
    fn test_mounted_widget_into_inner() {
        let widget = TestWidget::new(99);
        let mounted = MountedWidget::new(widget, Bounds::ZERO);

        let inner = mounted.into_inner();
        assert_eq!(inner.id, 99);
    }
}
