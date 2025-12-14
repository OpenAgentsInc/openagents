//! Mock platform for headless testing.

use wgpui::Size;

/// Mock platform that simulates platform operations without real resources.
///
/// Provides clipboard, window, and other platform operations in a
/// controlled, deterministic way for testing.
pub struct MockPlatform {
    /// Clipboard content.
    clipboard: Option<String>,
    /// Window size.
    window_size: Size,
    /// Scale factor.
    scale_factor: f32,
    /// Whether the window has focus.
    has_focus: bool,
    /// Current cursor style.
    cursor: String,
}

impl MockPlatform {
    /// Create a new mock platform with default settings.
    pub fn new() -> Self {
        Self {
            clipboard: None,
            window_size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            has_focus: true,
            cursor: "default".to_string(),
        }
    }

    /// Create with custom window size.
    pub fn with_window_size(mut self, width: f32, height: f32) -> Self {
        self.window_size = Size::new(width, height);
        self
    }

    /// Create with custom scale factor.
    pub fn with_scale_factor(mut self, factor: f32) -> Self {
        self.scale_factor = factor;
        self
    }

    // Clipboard operations

    /// Get clipboard text.
    pub fn clipboard_text(&self) -> Option<&str> {
        self.clipboard.as_deref()
    }

    /// Set clipboard text.
    pub fn set_clipboard_text(&mut self, text: impl Into<String>) {
        self.clipboard = Some(text.into());
    }

    /// Clear clipboard.
    pub fn clear_clipboard(&mut self) {
        self.clipboard = None;
    }

    // Window operations

    /// Get window size.
    pub fn window_size(&self) -> Size {
        self.window_size
    }

    /// Set window size.
    pub fn set_window_size(&mut self, size: Size) {
        self.window_size = size;
    }

    /// Resize window.
    pub fn resize(&mut self, width: f32, height: f32) {
        self.window_size = Size::new(width, height);
    }

    /// Get scale factor.
    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Set scale factor.
    pub fn set_scale_factor(&mut self, factor: f32) {
        self.scale_factor = factor;
    }

    // Focus operations

    /// Check if window has focus.
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    /// Set focus state.
    pub fn set_focus(&mut self, focused: bool) {
        self.has_focus = focused;
    }

    /// Simulate gaining focus.
    pub fn focus_in(&mut self) {
        self.has_focus = true;
    }

    /// Simulate losing focus.
    pub fn focus_out(&mut self) {
        self.has_focus = false;
    }

    // Cursor operations

    /// Get current cursor style.
    pub fn cursor(&self) -> &str {
        &self.cursor
    }

    /// Set cursor style.
    pub fn set_cursor(&mut self, cursor: impl Into<String>) {
        self.cursor = cursor.into();
    }

    /// Request specific cursor style (verifiable in tests).
    pub fn request_cursor(&mut self, cursor: &str) {
        self.cursor = cursor.to_string();
    }

    // Assertions

    /// Assert clipboard contains expected text.
    pub fn assert_clipboard_contains(&self, expected: &str) {
        match &self.clipboard {
            Some(text) => assert!(
                text.contains(expected),
                "Clipboard '{}' does not contain '{}'",
                text,
                expected
            ),
            None => panic!("Clipboard is empty, expected to contain '{}'", expected),
        }
    }

    /// Assert clipboard equals expected text.
    pub fn assert_clipboard_equals(&self, expected: &str) {
        match &self.clipboard {
            Some(text) => assert_eq!(
                text, expected,
                "Clipboard '{}' != expected '{}'",
                text, expected
            ),
            None => panic!("Clipboard is empty, expected '{}'", expected),
        }
    }

    /// Assert cursor is the expected style.
    pub fn assert_cursor(&self, expected: &str) {
        assert_eq!(
            self.cursor, expected,
            "Cursor '{}' != expected '{}'",
            self.cursor, expected
        );
    }

    /// Assert window has focus.
    pub fn assert_has_focus(&self) {
        assert!(self.has_focus, "Expected window to have focus");
    }

    /// Assert window does not have focus.
    pub fn assert_no_focus(&self) {
        assert!(!self.has_focus, "Expected window to not have focus");
    }
}

impl Default for MockPlatform {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_platform_clipboard() {
        let mut platform = MockPlatform::new();

        assert!(platform.clipboard_text().is_none());

        platform.set_clipboard_text("Hello, world!");
        assert_eq!(platform.clipboard_text(), Some("Hello, world!"));

        platform.clear_clipboard();
        assert!(platform.clipboard_text().is_none());
    }

    #[test]
    fn test_mock_platform_window() {
        let mut platform = MockPlatform::new();

        assert_eq!(platform.window_size().width, 1280.0);
        assert_eq!(platform.window_size().height, 720.0);

        platform.resize(800.0, 600.0);
        assert_eq!(platform.window_size().width, 800.0);
        assert_eq!(platform.window_size().height, 600.0);
    }

    #[test]
    fn test_mock_platform_scale_factor() {
        let platform = MockPlatform::new().with_scale_factor(2.0);
        assert_eq!(platform.scale_factor(), 2.0);
    }

    #[test]
    fn test_mock_platform_focus() {
        let mut platform = MockPlatform::new();

        assert!(platform.has_focus());

        platform.focus_out();
        assert!(!platform.has_focus());

        platform.focus_in();
        assert!(platform.has_focus());
    }

    #[test]
    fn test_mock_platform_cursor() {
        let mut platform = MockPlatform::new();

        assert_eq!(platform.cursor(), "default");

        platform.set_cursor("pointer");
        assert_eq!(platform.cursor(), "pointer");

        platform.assert_cursor("pointer");
    }

    #[test]
    fn test_mock_platform_builder() {
        let platform = MockPlatform::new()
            .with_window_size(1920.0, 1080.0)
            .with_scale_factor(1.5);

        assert_eq!(platform.window_size().width, 1920.0);
        assert_eq!(platform.window_size().height, 1080.0);
        assert_eq!(platform.scale_factor(), 1.5);
    }
}
