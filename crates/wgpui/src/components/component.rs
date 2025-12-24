use crate::components::context::{EventContext, PaintContext};
use crate::{Bounds, InputEvent};

pub type ComponentId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Handled,
    Ignored,
}

impl EventResult {
    pub fn is_handled(&self) -> bool {
        matches!(self, EventResult::Handled)
    }

    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (EventResult::Handled, _) | (_, EventResult::Handled) => EventResult::Handled,
            _ => EventResult::Ignored,
        }
    }
}

pub trait Component {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext);

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        None
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

pub struct AnyComponent {
    inner: Box<dyn AnyComponentTrait>,
}

trait AnyComponentTrait {
    fn paint_any(&mut self, bounds: Bounds, cx: &mut PaintContext);
    fn event_any(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult;
    fn id_any(&self) -> Option<ComponentId>;
    fn size_hint_any(&self) -> (Option<f32>, Option<f32>);
}

struct ComponentWrapper<C: Component> {
    component: C,
}

impl<C: Component + 'static> AnyComponentTrait for ComponentWrapper<C> {
    fn paint_any(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.component.paint(bounds, cx);
    }

    fn event_any(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        self.component.event(event, bounds, cx)
    }

    fn id_any(&self) -> Option<ComponentId> {
        self.component.id()
    }

    fn size_hint_any(&self) -> (Option<f32>, Option<f32>) {
        self.component.size_hint()
    }
}

impl AnyComponent {
    pub fn new<C: Component + 'static>(component: C) -> Self {
        Self {
            inner: Box::new(ComponentWrapper { component }),
        }
    }

    pub fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.inner.paint_any(bounds, cx);
    }

    pub fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        self.inner.event_any(event, bounds, cx)
    }

    pub fn id(&self) -> Option<ComponentId> {
        self.inner.id_any()
    }

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
