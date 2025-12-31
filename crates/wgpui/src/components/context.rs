use crate::action::{Action, ActionListeners, AnyAction, PendingAction};
use crate::keymap::KeyContext;
use crate::{Bounds, FocusChain, FocusHandle, FocusId, Point, Scene, TextSystem};
use std::collections::HashMap;

pub struct PaintContext<'a> {
    pub scene: &'a mut Scene,
    pub text: &'a mut TextSystem,
    pub scale_factor: f32,
    pub scroll_offset: Point,
}

impl<'a> PaintContext<'a> {
    pub fn new(scene: &'a mut Scene, text: &'a mut TextSystem, scale_factor: f32) -> Self {
        Self {
            scene,
            text,
            scale_factor,
            scroll_offset: Point::ZERO,
        }
    }

    pub fn with_scroll_offset(
        scene: &'a mut Scene,
        text: &'a mut TextSystem,
        scale_factor: f32,
        scroll_offset: Point,
    ) -> Self {
        Self {
            scene,
            text,
            scale_factor,
            scroll_offset,
        }
    }
}

pub struct EventContext {
    pub focused: Option<u64>,
    pub hovered: Option<u64>,
    pub scroll_offset: Point,
    focus_chain: FocusChain,

    // Action system fields
    key_context: KeyContext,
    action_listeners: HashMap<u64, ActionListeners>,
    pending_action: Option<PendingAction>,
}

impl EventContext {
    pub fn new() -> Self {
        Self {
            focused: None,
            hovered: None,
            scroll_offset: Point::ZERO,
            focus_chain: FocusChain::new(),
            key_context: KeyContext::new(),
            action_listeners: HashMap::new(),
            pending_action: None,
        }
    }

    // =========================================================================
    // Focus Management (existing)
    // =========================================================================

    pub fn set_focus(&mut self, id: u64) {
        self.focused = Some(id);
        self.focus_chain.set_focus(FocusId::new(id));
    }

    pub fn clear_focus(&mut self) {
        self.focused = None;
        self.focus_chain.clear_focus();
    }

    pub fn has_focus(&self, id: u64) -> bool {
        self.focused == Some(id)
    }

    pub fn focused_id(&self) -> Option<u64> {
        self.focused
    }

    pub fn register_focusable(&mut self, id: u64, bounds: Bounds, tab_index: i32) -> FocusHandle {
        let focus_id = FocusId::new(id);
        self.focus_chain.register(focus_id, bounds, tab_index);
        FocusHandle::new(id)
    }

    pub fn clear_focusables(&mut self) {
        self.focus_chain.clear_entries();
    }

    pub fn focus_next(&mut self) -> Option<u64> {
        let next = self.focus_chain.focus_next();
        self.focused = next.map(FocusId::value);
        self.focused
    }

    pub fn focus_prev(&mut self) -> Option<u64> {
        let prev = self.focus_chain.focus_prev();
        self.focused = prev.map(FocusId::value);
        self.focused
    }

    // =========================================================================
    // Hover Management (existing)
    // =========================================================================

    pub fn set_hover(&mut self, id: u64) {
        self.hovered = Some(id);
    }

    pub fn clear_hover(&mut self) {
        self.hovered = None;
    }

    // =========================================================================
    // Key Context
    // =========================================================================

    /// Push a context identifier onto the stack.
    ///
    /// Call this when entering a component that defines a keybinding context.
    ///
    /// # Example
    /// ```ignore
    /// cx.push_context("Modal");
    /// // ... handle events
    /// cx.pop_context();
    /// ```
    pub fn push_context(&mut self, identifier: impl Into<String>) {
        self.key_context.push(identifier);
    }

    /// Pop the most recent context identifier.
    pub fn pop_context(&mut self) -> Option<String> {
        self.key_context.pop()
    }

    /// Get the current key context.
    pub fn key_context(&self) -> &KeyContext {
        &self.key_context
    }

    /// Get mutable access to the key context.
    pub fn key_context_mut(&mut self) -> &mut KeyContext {
        &mut self.key_context
    }

    /// Clear all key contexts.
    pub fn clear_contexts(&mut self) {
        self.key_context.clear();
    }

    // =========================================================================
    // Action Listeners
    // =========================================================================

    /// Register an action handler for a component.
    ///
    /// The handler will be called when an action of type `A` is dispatched
    /// and the component is in the dispatch path.
    ///
    /// # Example
    /// ```ignore
    /// cx.on_action::<Cancel>(component_id, |action| {
    ///     // Handle cancel action
    ///     true // Return true if handled
    /// });
    /// ```
    pub fn on_action<A: Action>(
        &mut self,
        component_id: u64,
        handler: impl FnMut(&A) -> bool + 'static,
    ) {
        self.action_listeners
            .entry(component_id)
            .or_insert_with(ActionListeners::new)
            .on_action(handler);
    }

    /// Remove all action listeners for a component.
    pub fn remove_action_listeners(&mut self, component_id: u64) {
        self.action_listeners.remove(&component_id);
    }

    /// Clear all action listeners.
    pub fn clear_action_listeners(&mut self) {
        self.action_listeners.clear();
    }

    // =========================================================================
    // Action Dispatch
    // =========================================================================

    /// Queue an action for dispatch.
    ///
    /// The action will be dispatched to the focused component and bubble up
    /// through its ancestors.
    pub fn dispatch_action(&mut self, action: Box<dyn AnyAction>) {
        self.pending_action = Some(PendingAction::new(action));
    }

    /// Queue an action targeted at a specific component.
    pub fn dispatch_action_to(&mut self, action: Box<dyn AnyAction>, target: u64) {
        self.pending_action = Some(PendingAction::targeted(action, target));
    }

    /// Take the pending action (if any).
    ///
    /// Called by the event loop to process pending actions.
    pub fn take_pending_action(&mut self) -> Option<PendingAction> {
        self.pending_action.take()
    }

    /// Check if there's a pending action.
    pub fn has_pending_action(&self) -> bool {
        self.pending_action.is_some()
    }

    /// Try to handle an action with the listeners for a specific component.
    ///
    /// Returns `true` if the action was handled.
    pub fn try_handle_action(&mut self, action: &dyn AnyAction, component_id: u64) -> bool {
        if let Some(listeners) = self.action_listeners.get_mut(&component_id) {
            listeners.handle(action)
        } else {
            false
        }
    }

    /// Dispatch an action through the component hierarchy.
    ///
    /// Tries handlers in order of the provided component IDs (typically from
    /// focused to root). Returns the ID of the component that handled it.
    pub fn dispatch_to_hierarchy(
        &mut self,
        action: &dyn AnyAction,
        component_ids: &[u64],
    ) -> Option<u64> {
        for &id in component_ids {
            if self.try_handle_action(action, id) {
                return Some(id);
            }
        }
        None
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::standard::Cancel;

    #[test]
    fn test_event_context_focus() {
        let mut cx = EventContext::new();

        assert!(cx.focused.is_none());

        cx.set_focus(42);
        assert!(cx.has_focus(42));
        assert!(!cx.has_focus(99));

        cx.clear_focus();
        assert!(cx.focused.is_none());
    }

    #[test]
    fn test_event_context_hover() {
        let mut cx = EventContext::new();

        assert!(cx.hovered.is_none());

        cx.set_hover(42);
        assert_eq!(cx.hovered, Some(42));

        cx.clear_hover();
        assert!(cx.hovered.is_none());
    }

    #[test]
    fn test_key_context() {
        let mut cx = EventContext::new();

        assert!(cx.key_context().is_empty());

        cx.push_context("Window");
        cx.push_context("Modal");

        assert_eq!(cx.key_context().depth(), 2);
        assert!(cx.key_context().contains("Modal"));
        assert!(cx.key_context().contains("Window"));

        let popped = cx.pop_context();
        assert_eq!(popped, Some("Modal".to_string()));
        assert_eq!(cx.key_context().depth(), 1);

        cx.clear_contexts();
        assert!(cx.key_context().is_empty());
    }

    #[test]
    fn test_action_dispatch() {
        let mut cx = EventContext::new();

        // Register a handler
        let mut handled = false;
        cx.on_action::<Cancel>(42, move |_| {
            handled = true;
            true
        });

        // Dispatch action
        let action = Cancel;
        let result = cx.try_handle_action(&action, 42);
        assert!(result);

        // Wrong component ID
        let result = cx.try_handle_action(&action, 99);
        assert!(!result);
    }

    #[test]
    fn test_pending_action() {
        let mut cx = EventContext::new();

        assert!(!cx.has_pending_action());

        cx.dispatch_action(Box::new(Cancel));
        assert!(cx.has_pending_action());

        let pending = cx.take_pending_action();
        assert!(pending.is_some());
        assert!(!cx.has_pending_action());
    }

    #[test]
    fn test_dispatch_to_hierarchy() {
        let mut cx = EventContext::new();

        // Register handlers at different components
        cx.on_action::<Cancel>(1, |_| false); // Don't handle
        cx.on_action::<Cancel>(2, |_| true); // Handle
        cx.on_action::<Cancel>(3, |_| true); // Would handle, but won't be reached

        let action = Cancel;
        let hierarchy = [1, 2, 3];
        let handler_id = cx.dispatch_to_hierarchy(&action, &hierarchy);

        assert_eq!(handler_id, Some(2));
    }
}
