//! Interactive component extension trait.
//!
//! This module provides the `Interactive` trait that adds fluent action handling
//! to any component implementing the `Component` trait.
//!
//! # Example
//!
//! ```ignore
//! use wgpui::{Interactive, Button};
//! use wgpui::action::standard::{Cancel, Confirm};
//!
//! let button = Button::new("Submit")
//!     .on_action::<Confirm>(|action| {
//!         println!("Confirmed!");
//!         true
//!     })
//!     .on_action::<Cancel>(|action| {
//!         println!("Cancelled!");
//!         true
//!     });
//! ```

use crate::action::Action;
use crate::components::{Component, EventContext, EventResult, PaintContext};
use crate::{Bounds, ComponentId, InputEvent};
use std::any::Any;
use std::marker::PhantomData;

/// Extension trait for adding declarative action handlers to components.
///
/// This trait is automatically implemented for all types that implement `Component`.
pub trait Interactive: Component + Sized {
    /// Register an action handler for this component.
    ///
    /// Returns a wrapper component that intercepts actions of type `A` and
    /// calls the handler. If the handler returns `true`, the action is
    /// considered handled and won't bubble further.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let button = Button::new("Save")
    ///     .on_action::<Save>(|action| {
    ///         save_document();
    ///         true // Action handled
    ///     });
    /// ```
    fn on_action<A: Action>(self, handler: impl FnMut(&A) -> bool + 'static) -> WithAction<Self, A>
    where
        Self: Sized,
    {
        WithAction::new(self, handler)
    }

    /// Set a key context for this component.
    ///
    /// The context is pushed when the component handles events and popped after.
    /// This affects which keybindings match during event handling.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let modal = Modal::new()
    ///     .key_context("Modal")
    ///     .on_action::<Cancel>(|_| {
    ///         close_modal();
    ///         true
    ///     });
    /// ```
    fn key_context(self, context: impl Into<String>) -> WithContext<Self>
    where
        Self: Sized,
    {
        WithContext::new(self, context.into())
    }
}

// Implement Interactive for all Components
impl<C: Component + Sized> Interactive for C {}

/// Wrapper that adds action handling to a component.
pub struct WithAction<C, A> {
    inner: C,
    handler: Box<dyn FnMut(&dyn Any) -> bool>,
    _action: PhantomData<A>,
}

impl<C: Component, A: Action> WithAction<C, A> {
    fn new(inner: C, mut handler: impl FnMut(&A) -> bool + 'static) -> Self {
        Self {
            inner,
            handler: Box::new(move |action: &dyn Any| {
                if let Some(typed) = action.downcast_ref::<A>() {
                    handler(typed)
                } else {
                    false
                }
            }),
            _action: PhantomData,
        }
    }
}

impl<C: Component, A: Action> Component for WithAction<C, A> {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.inner.paint(bounds, cx);
    }

    fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        // Check for pending action
        if let Some(pending) = cx.take_pending_action() {
            // Try to handle if it's our action type
            if pending.action.action_id() == std::any::TypeId::of::<A>() {
                if (self.handler)(pending.action.as_any()) {
                    return EventResult::Handled;
                }
            }
            // Put it back if we didn't handle it
            cx.dispatch_action(pending.action);
        }

        // Delegate to inner component
        self.inner.event(event, bounds, cx)
    }

    fn id(&self) -> Option<ComponentId> {
        self.inner.id()
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        self.inner.size_hint()
    }
}

// Note: WithAction automatically gets Interactive via the blanket impl
// since it implements Component

/// Wrapper that adds a key context to a component.
pub struct WithContext<C> {
    inner: C,
    context: String,
}

impl<C: Component> WithContext<C> {
    fn new(inner: C, context: String) -> Self {
        Self { inner, context }
    }
}

impl<C: Component> Component for WithContext<C> {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.inner.paint(bounds, cx);
    }

    fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        // Push context before handling events
        cx.push_context(&self.context);

        let result = self.inner.event(event, bounds, cx);

        // Pop context after handling
        cx.pop_context();

        result
    }

    fn id(&self) -> Option<ComponentId> {
        self.inner.id()
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        self.inner.size_hint()
    }
}

// Note: WithContext automatically gets Interactive via the blanket impl
// since it implements Component

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::standard::Cancel;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Simple test component
    struct TestComponent {
        id: u64,
    }

    impl Component for TestComponent {
        fn paint(&mut self, _bounds: Bounds, _cx: &mut PaintContext) {}

        fn event(
            &mut self,
            _event: &InputEvent,
            _bounds: Bounds,
            _cx: &mut EventContext,
        ) -> EventResult {
            EventResult::Ignored
        }

        fn id(&self) -> Option<ComponentId> {
            Some(self.id)
        }
    }

    #[test]
    fn test_on_action_chainable() {
        let handled = Arc::new(AtomicBool::new(false));
        let handled_clone = handled.clone();

        let _component = TestComponent { id: 1 }
            .on_action::<Cancel>(move |_| {
                handled_clone.store(true, Ordering::SeqCst);
                true
            });

        // Component should compile and be usable
    }

    #[test]
    fn test_key_context_chainable() {
        let _component = TestComponent { id: 1 }
            .key_context("TestContext");

        // Component should compile and be usable
    }

    #[test]
    fn test_chain_multiple() {
        use crate::action::standard::{Cancel, Confirm};

        let _component = TestComponent { id: 1 }
            .key_context("Modal")
            .on_action::<Cancel>(|_| true)
            .on_action::<Confirm>(|_| true);

        // Should be able to chain context and multiple action handlers
    }

    #[test]
    fn test_with_context_pushes_and_pops() {
        let mut component = TestComponent { id: 1 }.key_context("TestModal");

        let mut cx = EventContext::new();
        let bounds = Bounds::new(0.0, 0.0, 100.0, 100.0);
        let event = InputEvent::MouseMove { x: 50.0, y: 50.0 };

        // Before event
        assert!(cx.key_context().is_empty());

        // During event handling, context is pushed then popped
        // (We can't easily verify during, but we verify it's empty after)
        component.event(&event, bounds, &mut cx);

        // After event
        assert!(cx.key_context().is_empty());
    }
}
