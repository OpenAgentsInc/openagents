//! Chrome - Window chrome and global UI elements.
//!
//! The chrome provides the frame around views, including:
//! - Title bar / header
//! - Status bar
//! - Navigation controls
//! - Global notifications

use crate::navigation::Breadcrumb;
use coder_widgets::EventResult;
use coder_widgets::context::{EventContext, PaintContext};
use wgpui::{Bounds, InputEvent, Point, Quad};

/// Status bar item.
#[derive(Debug, Clone)]
pub struct StatusItem {
    /// Item ID.
    pub id: u64,
    /// Display text.
    pub text: String,
    /// Icon (optional).
    pub icon: Option<String>,
    /// Tooltip.
    pub tooltip: Option<String>,
}

impl StatusItem {
    /// Create a new status item.
    pub fn new(id: u64, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
            icon: None,
            tooltip: None,
        }
    }

    /// Set an icon.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Set a tooltip.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

/// Application chrome (frame around views).
pub struct Chrome {
    /// Header height.
    header_height: f32,

    /// Status bar height.
    status_bar_height: f32,

    /// Application title.
    title: String,

    /// Status items.
    status_items: Vec<StatusItem>,

    /// Show header.
    show_header: bool,

    /// Show status bar.
    show_status_bar: bool,

    /// Show navigation controls.
    show_nav_controls: bool,

    /// Current breadcrumbs.
    breadcrumbs: Vec<Breadcrumb>,
}

impl Chrome {
    /// Create a new chrome.
    pub fn new() -> Self {
        Self {
            header_height: 48.0,
            status_bar_height: 24.0,
            title: "Coder".to_string(),
            status_items: Vec::new(),
            show_header: true,
            show_status_bar: true,
            show_nav_controls: true,
            breadcrumbs: Vec::new(),
        }
    }

    /// Set the application title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set header height.
    pub fn header_height(mut self, height: f32) -> Self {
        self.header_height = height;
        self
    }

    /// Set status bar height.
    pub fn status_bar_height(mut self, height: f32) -> Self {
        self.status_bar_height = height;
        self
    }

    /// Show or hide header.
    pub fn show_header(mut self, show: bool) -> Self {
        self.show_header = show;
        self
    }

    /// Show or hide status bar.
    pub fn show_status_bar(mut self, show: bool) -> Self {
        self.show_status_bar = show;
        self
    }

    /// Add a status item.
    pub fn add_status_item(&mut self, item: StatusItem) {
        self.status_items.push(item);
    }

    /// Remove a status item.
    pub fn remove_status_item(&mut self, id: u64) {
        self.status_items.retain(|item| item.id != id);
    }

    /// Update breadcrumbs.
    pub fn set_breadcrumbs(&mut self, breadcrumbs: Vec<Breadcrumb>) {
        self.breadcrumbs = breadcrumbs;
    }

    /// Get the content bounds (area inside chrome).
    pub fn content_bounds(&self, window_bounds: Bounds) -> Bounds {
        let header = if self.show_header {
            self.header_height
        } else {
            0.0
        };
        let status = if self.show_status_bar {
            self.status_bar_height
        } else {
            0.0
        };

        Bounds::new(
            window_bounds.origin.x,
            window_bounds.origin.y + header,
            window_bounds.size.width,
            window_bounds.size.height - header - status,
        )
    }

    /// Paint the chrome.
    pub fn paint(&self, bounds: Bounds, cx: &mut PaintContext) {
        // Paint header
        if self.show_header {
            self.paint_header(bounds, cx);
        }

        // Paint status bar
        if self.show_status_bar {
            self.paint_status_bar(bounds, cx);
        }
    }

    /// Paint the header.
    fn paint_header(&self, bounds: Bounds, cx: &mut PaintContext) {
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            self.header_height,
        );

        // Background
        cx.scene.draw_quad(
            Quad::new(header_bounds)
                .with_background(wgpui::theme::bg::SURFACE)
                .with_border(wgpui::theme::border::DEFAULT, 1.0),
        );

        // Title
        let title_run = cx.text.layout(
            &self.title,
            Point::new(bounds.origin.x + 16.0, bounds.origin.y + 16.0),
            14.0,
            wgpui::theme::accent::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Breadcrumbs
        let mut x = bounds.origin.x + 120.0;
        for (i, crumb) in self.breadcrumbs.iter().enumerate() {
            if i > 0 {
                // Separator
                let sep_run = cx.text.layout(
                    "/",
                    Point::new(x, bounds.origin.y + 16.0),
                    14.0,
                    wgpui::theme::text::MUTED,
                );
                cx.scene.draw_text(sep_run);
                x += 16.0;
            }

            let crumb_run = cx.text.layout(
                &crumb.label,
                Point::new(x, bounds.origin.y + 16.0),
                14.0,
                wgpui::theme::text::SECONDARY,
            );
            cx.scene.draw_text(crumb_run);
            x += crumb.label.len() as f32 * 8.0 + 8.0;
        }
    }

    /// Paint the status bar.
    fn paint_status_bar(&self, bounds: Bounds, cx: &mut PaintContext) {
        let status_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - self.status_bar_height,
            bounds.size.width,
            self.status_bar_height,
        );

        // Background
        cx.scene.draw_quad(
            Quad::new(status_bounds)
                .with_background(wgpui::theme::bg::SURFACE)
                .with_border(wgpui::theme::border::SUBTLE, 1.0),
        );

        // Status items
        let mut x = bounds.origin.x + 8.0;
        for item in &self.status_items {
            let item_run = cx.text.layout(
                &item.text,
                Point::new(x, status_bounds.origin.y + 6.0),
                12.0,
                wgpui::theme::text::SECONDARY,
            );
            cx.scene.draw_text(item_run);
            x += item.text.len() as f32 * 7.0 + 16.0;
        }
    }

    /// Handle chrome events.
    pub fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        // TODO: Handle clicks on breadcrumbs, nav controls, etc.
        EventResult::Ignored
    }
}

impl Default for Chrome {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_creation() {
        let chrome = Chrome::new()
            .title("My App")
            .header_height(56.0)
            .status_bar_height(28.0);

        assert_eq!(chrome.title, "My App");
        assert_eq!(chrome.header_height, 56.0);
        assert_eq!(chrome.status_bar_height, 28.0);
    }

    #[test]
    fn test_content_bounds() {
        let chrome = Chrome::new()
            .header_height(48.0)
            .status_bar_height(24.0);

        let window = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let content = chrome.content_bounds(window);

        assert_eq!(content.origin.x, 0.0);
        assert_eq!(content.origin.y, 48.0); // After header
        assert_eq!(content.size.width, 800.0);
        assert_eq!(content.size.height, 528.0); // 600 - 48 - 24
    }

    #[test]
    fn test_status_items() {
        let mut chrome = Chrome::new();

        chrome.add_status_item(StatusItem::new(1, "Ready"));
        chrome.add_status_item(StatusItem::new(2, "Line 42"));

        assert_eq!(chrome.status_items.len(), 2);

        chrome.remove_status_item(1);
        assert_eq!(chrome.status_items.len(), 1);
        assert_eq!(chrome.status_items[0].id, 2);
    }

    #[test]
    fn test_hide_chrome() {
        let chrome = Chrome::new()
            .show_header(false)
            .show_status_bar(false);

        let window = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let content = chrome.content_bounds(window);

        // Should be full window
        assert_eq!(content.origin.y, 0.0);
        assert_eq!(content.size.height, 600.0);
    }
}
