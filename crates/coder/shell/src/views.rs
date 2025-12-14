//! Views - View registry and rendering.
//!
//! Views are the top-level surfaces that correspond to routes.
//! Each view manages its own state and renders its content.

use crate::router::Route;
use coder_widgets::Widget;

/// View identifier.
pub type ViewId = u64;

/// A view that can be rendered.
pub trait View {
    /// Get the view's unique identifier.
    fn id(&self) -> ViewId;

    /// Get the route this view handles.
    fn route(&self) -> &Route;

    /// Check if this view matches a route.
    fn matches(&self, route: &Route) -> bool {
        std::mem::discriminant(self.route()) == std::mem::discriminant(route)
    }

    /// Called when the view becomes active.
    fn on_activate(&mut self) {}

    /// Called when the view becomes inactive.
    fn on_deactivate(&mut self) {}

    /// Get the view's root widget.
    fn widget(&mut self) -> &mut dyn Widget;

    /// Get the view's title for display.
    fn title(&self) -> String;
}

/// A boxed view.
pub struct AnyView {
    inner: Box<dyn View>,
}

impl AnyView {
    /// Create a new boxed view.
    pub fn new<V: View + 'static>(view: V) -> Self {
        Self {
            inner: Box::new(view),
        }
    }

    /// Get the view ID.
    pub fn id(&self) -> ViewId {
        self.inner.id()
    }

    /// Get the route.
    pub fn route(&self) -> &Route {
        self.inner.route()
    }

    /// Check if matches a route.
    pub fn matches(&self, route: &Route) -> bool {
        self.inner.matches(route)
    }

    /// Activate the view.
    pub fn activate(&mut self) {
        self.inner.on_activate();
    }

    /// Deactivate the view.
    pub fn deactivate(&mut self) {
        self.inner.on_deactivate();
    }

    /// Get the widget.
    pub fn widget(&mut self) -> &mut dyn Widget {
        self.inner.widget()
    }

    /// Get the title.
    pub fn title(&self) -> String {
        self.inner.title()
    }
}

/// View registry that manages available views.
pub struct ViewRegistry {
    /// Registered views.
    views: Vec<AnyView>,

    /// Currently active view ID.
    active: Option<ViewId>,
}

impl ViewRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            views: Vec::new(),
            active: None,
        }
    }

    /// Register a view.
    pub fn register<V: View + 'static>(&mut self, view: V) {
        self.views.push(AnyView::new(view));
    }

    /// Get the active view.
    pub fn active(&mut self) -> Option<&mut AnyView> {
        let active_id = self.active?;
        self.views.iter_mut().find(|v| v.id() == active_id)
    }

    /// Get a view by ID.
    pub fn get(&mut self, id: ViewId) -> Option<&mut AnyView> {
        self.views.iter_mut().find(|v| v.id() == id)
    }

    /// Find a view that matches a route.
    pub fn find_for_route(&mut self, route: &Route) -> Option<&mut AnyView> {
        self.views.iter_mut().find(|v| v.matches(route))
    }

    /// Activate a view by route.
    pub fn activate_for_route(&mut self, route: &Route) -> Option<ViewId> {
        // Deactivate current
        if let Some(current_id) = self.active {
            if let Some(view) = self.views.iter_mut().find(|v| v.id() == current_id) {
                view.deactivate();
            }
        }

        // Find and activate new view
        for view in &mut self.views {
            if view.matches(route) {
                view.activate();
                self.active = Some(view.id());
                return Some(view.id());
            }
        }

        self.active = None;
        None
    }

    /// Get the active view's title.
    pub fn active_title(&self) -> Option<String> {
        let active_id = self.active?;
        self.views
            .iter()
            .find(|v| v.id() == active_id)
            .map(|v| v.title())
    }

    /// Get the number of registered views.
    pub fn len(&self) -> usize {
        self.views.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }
}

impl Default for ViewRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_widgets::{context::PaintContext, Div, EventResult};
    use wgpui::InputEvent;

    struct TestView {
        id: ViewId,
        route: Route,
        activated: bool,
        widget: Div,
    }

    impl TestView {
        fn new(id: ViewId, route: Route) -> Self {
            Self {
                id,
                route,
                activated: false,
                widget: Div::new(),
            }
        }
    }

    impl View for TestView {
        fn id(&self) -> ViewId {
            self.id
        }

        fn route(&self) -> &Route {
            &self.route
        }

        fn on_activate(&mut self) {
            self.activated = true;
        }

        fn on_deactivate(&mut self) {
            self.activated = false;
        }

        fn widget(&mut self) -> &mut dyn Widget {
            &mut self.widget
        }

        fn title(&self) -> String {
            format!("Test View {}", self.id)
        }
    }

    #[test]
    fn test_view_registry() {
        let mut registry = ViewRegistry::new();

        registry.register(TestView::new(1, Route::Home));
        registry.register(TestView::new(2, Route::Settings));

        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_view_activation() {
        let mut registry = ViewRegistry::new();

        registry.register(TestView::new(1, Route::Home));
        registry.register(TestView::new(2, Route::Settings));

        // Activate home
        let id = registry.activate_for_route(&Route::Home);
        assert_eq!(id, Some(1));

        // Activate settings
        let id = registry.activate_for_route(&Route::Settings);
        assert_eq!(id, Some(2));
    }

    #[test]
    fn test_view_matching() {
        let view = TestView::new(1, Route::Home);

        assert!(view.matches(&Route::Home));
        assert!(!view.matches(&Route::Settings));
    }
}
