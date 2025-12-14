//! Widget trait - the core abstraction for UI components.
//!
//! All widgets implement this trait to participate in the
//! layout, paint, and event handling pipeline.

use crate::context::{EventContext, PaintContext};
use wgpui::{Bounds, InputEvent};

/// Unique identifier for a widget instance.
pub type WidgetId = u64;

/// Result of handling an input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// Event was handled, stop propagation.
    Handled,
    /// Event was not handled, continue propagation.
    Ignored,
}

impl EventResult {
    /// Returns true if the event was handled.
    pub fn is_handled(&self) -> bool {
        matches!(self, EventResult::Handled)
    }

    /// Combine two results (handled takes precedence).
    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (EventResult::Handled, _) | (_, EventResult::Handled) => EventResult::Handled,
            _ => EventResult::Ignored,
        }
    }
}

/// The core widget trait.
///
/// Widgets go through two phases:
/// 1. **Paint**: Draw to the scene
/// 2. **Event**: Handle input events
///
/// Layout is handled separately through wgpui's LayoutEngine.
pub trait Widget {
    /// Paint the widget to the scene.
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext);

    /// Handle an input event.
    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }

    /// Get the unique ID for this widget instance.
    fn id(&self) -> Option<WidgetId> {
        None
    }

    /// Get the preferred size hint.
    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

/// A boxed widget that erases the concrete type.
pub struct AnyWidget {
    inner: Box<dyn AnyWidgetTrait>,
}

trait AnyWidgetTrait {
    fn paint_any(&mut self, bounds: Bounds, cx: &mut PaintContext);
    fn event_any(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult;
    fn id_any(&self) -> Option<WidgetId>;
    fn size_hint_any(&self) -> (Option<f32>, Option<f32>);
}

struct WidgetWrapper<W: Widget> {
    widget: W,
}

impl<W: Widget + 'static> AnyWidgetTrait for WidgetWrapper<W> {
    fn paint_any(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.widget.paint(bounds, cx);
    }

    fn event_any(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        self.widget.event(event, bounds, cx)
    }

    fn id_any(&self) -> Option<WidgetId> {
        self.widget.id()
    }

    fn size_hint_any(&self) -> (Option<f32>, Option<f32>) {
        self.widget.size_hint()
    }
}

impl AnyWidget {
    /// Create a new type-erased widget.
    pub fn new<W: Widget + 'static>(widget: W) -> Self {
        Self {
            inner: Box::new(WidgetWrapper { widget }),
        }
    }

    /// Paint this widget.
    pub fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.inner.paint_any(bounds, cx);
    }

    /// Handle an event.
    pub fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        self.inner.event_any(event, bounds, cx)
    }

    /// Get the widget ID.
    pub fn id(&self) -> Option<WidgetId> {
        self.inner.id_any()
    }

    /// Get the size hint.
    pub fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        self.inner.size_hint_any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_result() {
        assert!(EventResult::Handled.is_handled());
        assert!(!EventResult::Ignored.is_handled());

        assert_eq!(
            EventResult::Handled.or(EventResult::Ignored),
            EventResult::Handled
        );
        assert_eq!(
            EventResult::Ignored.or(EventResult::Handled),
            EventResult::Handled
        );
        assert_eq!(
            EventResult::Ignored.or(EventResult::Ignored),
            EventResult::Ignored
        );
    }
}
