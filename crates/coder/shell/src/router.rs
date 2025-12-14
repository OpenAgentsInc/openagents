//! Router - URL ↔ View mapping.
//!
//! The router maps URLs to views and handles navigation between them.

use coder_domain::ids::{ProjectId, ThreadId};
use coder_ui_runtime::Signal;

/// Application routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Home/landing page.
    Home,

    /// Chat thread view.
    Chat { thread_id: ThreadId },

    /// Project view.
    Project { project_id: ProjectId },

    /// Settings view.
    Settings,

    /// 404 - Not found.
    NotFound { path: String },
}

impl Route {
    /// Get the URL path for this route.
    pub fn to_path(&self) -> String {
        match self {
            Route::Home => "/".to_string(),
            Route::Chat { thread_id } => format!("/chat/{}", thread_id),
            Route::Project { project_id } => format!("/project/{}", project_id),
            Route::Settings => "/settings".to_string(),
            Route::NotFound { path } => path.clone(),
        }
    }

    /// Parse a URL path into a route.
    pub fn from_path(path: &str) -> Self {
        let path = path.trim_start_matches('/');
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        match segments.as_slice() {
            [] => Route::Home,
            ["chat", id] => {
                if let Ok(thread_id) = ThreadId::parse(id) {
                    Route::Chat { thread_id }
                } else {
                    Route::NotFound {
                        path: format!("/chat/{}", id),
                    }
                }
            }
            ["project", id] => {
                if let Ok(project_id) = ProjectId::parse(id) {
                    Route::Project { project_id }
                } else {
                    Route::NotFound {
                        path: format!("/project/{}", id),
                    }
                }
            }
            ["settings"] => Route::Settings,
            _ => Route::NotFound {
                path: format!("/{}", path),
            },
        }
    }

    /// Check if this is the home route.
    pub fn is_home(&self) -> bool {
        matches!(self, Route::Home)
    }

    /// Check if this is a chat route.
    pub fn is_chat(&self) -> bool {
        matches!(self, Route::Chat { .. })
    }

    /// Check if this is a not found route.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Route::NotFound { .. })
    }
}

impl Default for Route {
    fn default() -> Self {
        Route::Home
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_path())
    }
}

/// The application router.
pub struct Router {
    /// Current active route.
    current: Signal<Route>,

    /// Navigation history.
    history: Vec<Route>,

    /// Current position in history.
    history_index: usize,

    /// Maximum history size.
    max_history: usize,
}

impl Router {
    /// Create a new router.
    pub fn new() -> Self {
        Self {
            current: Signal::new(Route::Home),
            history: vec![Route::Home],
            history_index: 0,
            max_history: 100,
        }
    }

    /// Create a router with an initial route.
    pub fn with_route(route: Route) -> Self {
        Self {
            current: Signal::new(route.clone()),
            history: vec![route],
            history_index: 0,
            max_history: 100,
        }
    }

    /// Get the current route.
    pub fn current(&self) -> Route {
        self.current.get_untracked()
    }

    /// Get the current route signal for reactive tracking.
    pub fn current_signal(&self) -> &Signal<Route> {
        &self.current
    }

    /// Navigate to a new route.
    pub fn navigate(&mut self, route: Route) {
        // Don't navigate to the same route
        if self.current.get_untracked() == route {
            return;
        }

        // If we're not at the end of history, truncate
        if self.history_index < self.history.len() - 1 {
            self.history.truncate(self.history_index + 1);
        }

        // Add new route to history
        self.history.push(route.clone());
        self.history_index = self.history.len() - 1;

        // Trim history if too long
        if self.history.len() > self.max_history {
            let excess = self.history.len() - self.max_history;
            self.history.drain(0..excess);
            self.history_index -= excess;
        }

        // Update current route
        self.current.set(route);
    }

    /// Navigate back in history.
    pub fn back(&mut self) -> bool {
        if self.history_index > 0 {
            self.history_index -= 1;
            let route = self.history[self.history_index].clone();
            self.current.set(route);
            true
        } else {
            false
        }
    }

    /// Navigate forward in history.
    pub fn forward(&mut self) -> bool {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            let route = self.history[self.history_index].clone();
            self.current.set(route);
            true
        } else {
            false
        }
    }

    /// Check if we can go back.
    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    /// Check if we can go forward.
    pub fn can_go_forward(&self) -> bool {
        self.history_index < self.history.len() - 1
    }

    /// Get the history length.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Clear navigation history (keeps current route).
    pub fn clear_history(&mut self) {
        let current = self.current.get_untracked();
        self.history = vec![current];
        self.history_index = 0;
    }

    /// Replace the current route without adding to history.
    pub fn replace(&mut self, route: Route) {
        if self.history_index < self.history.len() {
            self.history[self.history_index] = route.clone();
        }
        self.current.set(route);
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_parsing() {
        assert_eq!(Route::from_path("/"), Route::Home);
        assert_eq!(Route::from_path(""), Route::Home);
        assert_eq!(Route::from_path("/settings"), Route::Settings);

        // Invalid UUID should result in NotFound
        let result = Route::from_path("/chat/invalid");
        assert!(matches!(result, Route::NotFound { .. }));
    }

    #[test]
    fn test_route_to_path() {
        assert_eq!(Route::Home.to_path(), "/");
        assert_eq!(Route::Settings.to_path(), "/settings");

        let thread_id = ThreadId::new();
        let chat_route = Route::Chat { thread_id };
        assert!(chat_route.to_path().starts_with("/chat/"));
    }

    #[test]
    fn test_router_navigation() {
        let mut router = Router::new();

        // Initial state
        assert!(router.current().is_home());
        assert!(!router.can_go_back());
        assert!(!router.can_go_forward());

        // Navigate to settings
        router.navigate(Route::Settings);
        assert_eq!(router.current(), Route::Settings);
        assert!(router.can_go_back());
        assert!(!router.can_go_forward());

        // Navigate back
        assert!(router.back());
        assert!(router.current().is_home());
        assert!(!router.can_go_back());
        assert!(router.can_go_forward());

        // Navigate forward
        assert!(router.forward());
        assert_eq!(router.current(), Route::Settings);
    }

    #[test]
    fn test_router_history_truncation() {
        let mut router = Router::new();

        // Build up some history
        router.navigate(Route::Settings);
        let thread_id = ThreadId::new();
        router.navigate(Route::Chat { thread_id });

        assert_eq!(router.history_len(), 3);

        // Go back and navigate somewhere else
        router.back();
        router.navigate(Route::Home);

        // History should be truncated
        assert_eq!(router.history_len(), 3); // Home -> Settings -> Home
    }

    #[test]
    fn test_router_replace() {
        let mut router = Router::new();
        router.navigate(Route::Settings);

        let thread_id = ThreadId::new();
        router.replace(Route::Chat { thread_id });

        // Should have same history length
        assert_eq!(router.history_len(), 2);

        // Can still go back to home
        assert!(router.back());
        assert!(router.current().is_home());
    }

    #[test]
    fn test_route_equality() {
        let thread1 = ThreadId::new();
        let thread2 = ThreadId::new();

        let route1 = Route::Chat { thread_id: thread1 };
        let route2 = Route::Chat { thread_id: thread1 };
        let route3 = Route::Chat { thread_id: thread2 };

        assert_eq!(route1, route2);
        assert_ne!(route1, route3);
    }
}
