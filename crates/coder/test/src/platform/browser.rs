//! Mock browser API for testing WASM code without a real browser.

use std::collections::HashMap;

/// Mock browser APIs for testing web-specific functionality.
///
/// Simulates localStorage, sessionStorage, URL navigation, and
/// other browser APIs without requiring a real browser.
pub struct MockBrowserAPI {
    /// Local storage data.
    local_storage: HashMap<String, String>,
    /// Session storage data.
    session_storage: HashMap<String, String>,
    /// Current URL.
    url: String,
    /// Navigation history.
    history: Vec<String>,
    /// Current history index.
    history_index: usize,
    /// Document title.
    title: String,
    /// Cookies.
    cookies: HashMap<String, String>,
    /// User agent string.
    user_agent: String,
    /// Is online.
    online: bool,
}

impl MockBrowserAPI {
    /// Create a new mock browser API.
    pub fn new() -> Self {
        Self {
            local_storage: HashMap::new(),
            session_storage: HashMap::new(),
            url: "http://localhost:3000".to_string(),
            history: vec!["http://localhost:3000".to_string()],
            history_index: 0,
            title: "Test".to_string(),
            cookies: HashMap::new(),
            user_agent: "Mozilla/5.0 (Test) MockBrowser/1.0".to_string(),
            online: true,
        }
    }

    // Local Storage

    /// Get a value from local storage.
    pub fn get_local_storage(&self, key: &str) -> Option<&str> {
        self.local_storage.get(key).map(|s| s.as_str())
    }

    /// Set a value in local storage.
    pub fn set_local_storage(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.local_storage.insert(key.into(), value.into());
    }

    /// Remove a value from local storage.
    pub fn remove_local_storage(&mut self, key: &str) {
        self.local_storage.remove(key);
    }

    /// Clear all local storage.
    pub fn clear_local_storage(&mut self) {
        self.local_storage.clear();
    }

    /// Get all local storage keys.
    pub fn local_storage_keys(&self) -> Vec<&str> {
        self.local_storage.keys().map(|s| s.as_str()).collect()
    }

    // Session Storage

    /// Get a value from session storage.
    pub fn get_session_storage(&self, key: &str) -> Option<&str> {
        self.session_storage.get(key).map(|s| s.as_str())
    }

    /// Set a value in session storage.
    pub fn set_session_storage(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.session_storage.insert(key.into(), value.into());
    }

    /// Remove a value from session storage.
    pub fn remove_session_storage(&mut self, key: &str) {
        self.session_storage.remove(key);
    }

    /// Clear all session storage.
    pub fn clear_session_storage(&mut self) {
        self.session_storage.clear();
    }

    // Navigation

    /// Get the current URL.
    pub fn current_url(&self) -> &str {
        &self.url
    }

    /// Navigate to a new URL.
    pub fn navigate(&mut self, url: impl Into<String>) {
        let url = url.into();
        // Truncate forward history when navigating
        self.history.truncate(self.history_index + 1);
        self.history.push(url.clone());
        self.history_index = self.history.len() - 1;
        self.url = url;
    }

    /// Go back in history.
    pub fn back(&mut self) -> bool {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.url = self.history[self.history_index].clone();
            true
        } else {
            false
        }
    }

    /// Go forward in history.
    pub fn forward(&mut self) -> bool {
        if self.history_index < self.history.len() - 1 {
            self.history_index += 1;
            self.url = self.history[self.history_index].clone();
            true
        } else {
            false
        }
    }

    /// Get the navigation history.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Get the history length.
    pub fn history_length(&self) -> usize {
        self.history.len()
    }

    // Document

    /// Get the document title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set the document title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    // Cookies

    /// Get a cookie value.
    pub fn get_cookie(&self, name: &str) -> Option<&str> {
        self.cookies.get(name).map(|s| s.as_str())
    }

    /// Set a cookie.
    pub fn set_cookie(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.cookies.insert(name.into(), value.into());
    }

    /// Delete a cookie.
    pub fn delete_cookie(&mut self, name: &str) {
        self.cookies.remove(name);
    }

    /// Get all cookie names.
    pub fn cookie_names(&self) -> Vec<&str> {
        self.cookies.keys().map(|s| s.as_str()).collect()
    }

    // Network

    /// Get the user agent string.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Set the user agent string.
    pub fn set_user_agent(&mut self, user_agent: impl Into<String>) {
        self.user_agent = user_agent.into();
    }

    /// Check if online.
    pub fn is_online(&self) -> bool {
        self.online
    }

    /// Set online status.
    pub fn set_online(&mut self, online: bool) {
        self.online = online;
    }

    /// Simulate going offline.
    pub fn go_offline(&mut self) {
        self.online = false;
    }

    /// Simulate going online.
    pub fn go_online(&mut self) {
        self.online = true;
    }

    // Assertions

    /// Assert current URL matches.
    pub fn assert_url(&self, expected: &str) {
        assert_eq!(
            self.url, expected,
            "URL '{}' != expected '{}'",
            self.url, expected
        );
    }

    /// Assert URL contains substring.
    pub fn assert_url_contains(&self, substring: &str) {
        assert!(
            self.url.contains(substring),
            "URL '{}' does not contain '{}'",
            self.url, substring
        );
    }

    /// Assert local storage contains key.
    pub fn assert_local_storage_has(&self, key: &str) {
        assert!(
            self.local_storage.contains_key(key),
            "Local storage does not contain key '{}'",
            key
        );
    }

    /// Assert local storage value equals.
    pub fn assert_local_storage_equals(&self, key: &str, expected: &str) {
        match self.get_local_storage(key) {
            Some(actual) => assert_eq!(
                actual, expected,
                "Local storage '{}' value '{}' != expected '{}'",
                key, actual, expected
            ),
            None => panic!("Local storage key '{}' not found", key),
        }
    }

    /// Assert title equals.
    pub fn assert_title(&self, expected: &str) {
        assert_eq!(
            self.title, expected,
            "Title '{}' != expected '{}'",
            self.title, expected
        );
    }
}

impl Default for MockBrowserAPI {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_storage() {
        let mut browser = MockBrowserAPI::new();

        browser.set_local_storage("key1", "value1");
        assert_eq!(browser.get_local_storage("key1"), Some("value1"));

        browser.remove_local_storage("key1");
        assert!(browser.get_local_storage("key1").is_none());
    }

    #[test]
    fn test_navigation() {
        let mut browser = MockBrowserAPI::new();

        assert_eq!(browser.current_url(), "http://localhost:3000");

        browser.navigate("http://localhost:3000/page1");
        assert_eq!(browser.current_url(), "http://localhost:3000/page1");

        browser.navigate("http://localhost:3000/page2");
        assert_eq!(browser.current_url(), "http://localhost:3000/page2");

        assert!(browser.back());
        assert_eq!(browser.current_url(), "http://localhost:3000/page1");

        assert!(browser.forward());
        assert_eq!(browser.current_url(), "http://localhost:3000/page2");
    }

    #[test]
    fn test_navigation_truncates_forward() {
        let mut browser = MockBrowserAPI::new();

        browser.navigate("/a");
        browser.navigate("/b");
        browser.navigate("/c");
        browser.back();
        browser.back();

        // Now at /a, navigating to /d should truncate /b and /c
        browser.navigate("/d");

        assert!(!browser.forward()); // No forward history
        assert_eq!(browser.current_url(), "/d");
    }

    #[test]
    fn test_cookies() {
        let mut browser = MockBrowserAPI::new();

        browser.set_cookie("session", "abc123");
        assert_eq!(browser.get_cookie("session"), Some("abc123"));

        browser.delete_cookie("session");
        assert!(browser.get_cookie("session").is_none());
    }

    #[test]
    fn test_online_status() {
        let mut browser = MockBrowserAPI::new();

        assert!(browser.is_online());

        browser.go_offline();
        assert!(!browser.is_online());

        browser.go_online();
        assert!(browser.is_online());
    }

    #[test]
    fn test_document_title() {
        let mut browser = MockBrowserAPI::new();

        browser.set_title("My Page");
        assert_eq!(browser.title(), "My Page");
        browser.assert_title("My Page");
    }
}
