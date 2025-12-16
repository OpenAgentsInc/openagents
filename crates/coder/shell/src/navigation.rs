//! Navigation - History management and navigation controls.
//!
//! This module provides navigation controls and history tracking
//! for the application.

use crate::router::{Route, Router};
use coder_ui_runtime::Signal;

/// Navigation controller.
///
/// Wraps the router and provides additional navigation features
/// like breadcrumbs and deep linking.
pub struct Navigation {
    /// The underlying router.
    router: Router,

    /// Whether navigation is locked (e.g., during a modal).
    locked: Signal<bool>,

    /// Pending navigation (when locked).
    pending_route: Option<Route>,
}

impl Navigation {
    /// Create a new navigation controller.
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            locked: Signal::new(false),
            pending_route: None,
        }
    }

    /// Create with an initial route.
    pub fn with_route(route: Route) -> Self {
        Self {
            router: Router::with_route(route),
            locked: Signal::new(false),
            pending_route: None,
        }
    }

    /// Get the current route.
    pub fn current(&self) -> Route {
        self.router.current()
    }

    /// Get the current route signal.
    pub fn current_signal(&self) -> &Signal<Route> {
        self.router.current_signal()
    }

    /// Navigate to a route.
    pub fn navigate(&mut self, route: Route) {
        if self.locked.get_untracked() {
            // Store for when unlocked
            self.pending_route = Some(route);
        } else {
            self.router.navigate(route);
        }
    }

    /// Navigate to a path string.
    pub fn navigate_to_path(&mut self, path: &str) {
        self.navigate(Route::from_path(path));
    }

    /// Go back in history.
    pub fn back(&mut self) -> bool {
        if self.locked.get_untracked() {
            false
        } else {
            self.router.back()
        }
    }

    /// Go forward in history.
    pub fn forward(&mut self) -> bool {
        if self.locked.get_untracked() {
            false
        } else {
            self.router.forward()
        }
    }

    /// Check if we can go back.
    pub fn can_go_back(&self) -> bool {
        !self.locked.get_untracked() && self.router.can_go_back()
    }

    /// Check if we can go forward.
    pub fn can_go_forward(&self) -> bool {
        !self.locked.get_untracked() && self.router.can_go_forward()
    }

    /// Lock navigation (e.g., during unsaved changes).
    pub fn lock(&mut self) {
        self.locked.set(true);
    }

    /// Unlock navigation.
    pub fn unlock(&mut self) {
        self.locked.set(false);

        // Process pending navigation
        if let Some(route) = self.pending_route.take() {
            self.router.navigate(route);
        }
    }

    /// Check if navigation is locked.
    pub fn is_locked(&self) -> bool {
        self.locked.get_untracked()
    }

    /// Get the underlying router.
    pub fn router(&self) -> &Router {
        &self.router
    }

    /// Get mutable access to the router.
    pub fn router_mut(&mut self) -> &mut Router {
        &mut self.router
    }

    /// Generate breadcrumbs for the current route.
    pub fn breadcrumbs(&self) -> Vec<Breadcrumb> {
        let route = self.current();
        match route {
            Route::Home => vec![Breadcrumb::new("Home", Route::Home)],
            Route::Chat { thread_id } => vec![
                Breadcrumb::new("Home", Route::Home),
                Breadcrumb::new(&format!("Chat {}", &thread_id.to_string()[..8]), route),
            ],
            Route::Project { project_id } => vec![
                Breadcrumb::new("Home", Route::Home),
                Breadcrumb::new(&format!("Project {}", &project_id.to_string()[..8]), route),
            ],
            Route::Settings => vec![
                Breadcrumb::new("Home", Route::Home),
                Breadcrumb::new("Settings", Route::Settings),
            ],
            Route::NotFound { ref path } => vec![
                Breadcrumb::new("Home", Route::Home),
                Breadcrumb::new(&format!("Not Found: {}", path), route.clone()),
            ],
        }
    }
}

impl Default for Navigation {
    fn default() -> Self {
        Self::new()
    }
}

/// A breadcrumb item.
#[derive(Debug, Clone)]
pub struct Breadcrumb {
    /// Display label.
    pub label: String,
    /// Route to navigate to.
    pub route: Route,
}

impl Breadcrumb {
    /// Create a new breadcrumb.
    pub fn new(label: &str, route: Route) -> Self {
        Self {
            label: label.to_string(),
            route,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_basic() {
        let mut nav = Navigation::new();

        assert!(nav.current().is_home());

        nav.navigate(Route::Settings);
        assert_eq!(nav.current(), Route::Settings);
    }

    #[test]
    fn test_navigation_locking() {
        let mut nav = Navigation::new();

        nav.lock();
        assert!(nav.is_locked());

        // Navigation should be blocked
        nav.navigate(Route::Settings);
        assert!(nav.current().is_home()); // Still at home

        nav.unlock();
        // Pending navigation should execute
        assert_eq!(nav.current(), Route::Settings);
    }

    #[test]
    fn test_breadcrumbs() {
        let nav = Navigation::new();

        let crumbs = nav.breadcrumbs();
        assert_eq!(crumbs.len(), 1);
        assert_eq!(crumbs[0].label, "Home");

        let mut nav = Navigation::with_route(Route::Settings);
        let crumbs = nav.breadcrumbs();
        assert_eq!(crumbs.len(), 2);
        assert_eq!(crumbs[0].label, "Home");
        assert_eq!(crumbs[1].label, "Settings");
    }

    #[test]
    fn test_navigate_to_path() {
        let mut nav = Navigation::new();

        nav.navigate_to_path("/settings");
        assert_eq!(nav.current(), Route::Settings);

        nav.navigate_to_path("/");
        assert!(nav.current().is_home());
    }
}
