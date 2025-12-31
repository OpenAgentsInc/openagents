//! Event inspector organism for viewing Nostr event details.
//!
//! Provides a detailed view of Nostr events with NIP-specific parsing and display.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Event kind categorization
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EventCategory {
    #[default]
    Note,
    Metadata,
    Contacts,
    DirectMessage,
    Deletion,
    Repost,
    Reaction,
    Zap,
    Channel,
    Marketplace,
    Custom(u32),
}

impl EventCategory {
    pub fn from_kind(kind: u32) -> Self {
        match kind {
            0 => Self::Metadata,
            1 => Self::Note,
            2 => Self::Contacts,
            3 => Self::Contacts,
            4 => Self::DirectMessage,
            5 => Self::Deletion,
            6 => Self::Repost,
            7 => Self::Reaction,
            9735 => Self::Zap,
            40..=49 => Self::Channel,
            30000..=39999 => Self::Marketplace,
            _ => Self::Custom(kind),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Note => "Text Note",
            Self::Metadata => "Metadata",
            Self::Contacts => "Contacts",
            Self::DirectMessage => "Direct Message",
            Self::Deletion => "Deletion",
            Self::Repost => "Repost",
            Self::Reaction => "Reaction",
            Self::Zap => "Zap",
            Self::Channel => "Channel",
            Self::Marketplace => "Marketplace",
            Self::Custom(_) => "Custom",
        }
    }
}

/// Inspector view mode
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum InspectorView {
    #[default]
    Summary,
    Raw,
    Tags,
    Signature,
}

/// Tag data for display
#[derive(Debug, Clone)]
pub struct TagData {
    pub key: String,
    pub values: Vec<String>,
}

impl TagData {
    pub fn new(key: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            key: key.into(),
            values,
        }
    }
}

/// Nostr event data for the inspector
#[derive(Debug, Clone)]
pub struct EventData {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub content: String,
    pub tags: Vec<TagData>,
    pub sig: String,
    pub verified: bool,
}

impl Default for EventData {
    fn default() -> Self {
        Self {
            id: String::new(),
            pubkey: String::new(),
            created_at: 0,
            kind: 1,
            content: String::new(),
            tags: Vec::new(),
            sig: String::new(),
            verified: false,
        }
    }
}

impl EventData {
    pub fn new(id: impl Into<String>, pubkey: impl Into<String>, kind: u32) -> Self {
        Self {
            id: id.into(),
            pubkey: pubkey.into(),
            kind,
            ..Default::default()
        }
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn created_at(mut self, created_at: u64) -> Self {
        self.created_at = created_at;
        self
    }

    pub fn tags(mut self, tags: Vec<TagData>) -> Self {
        self.tags = tags;
        self
    }

    pub fn sig(mut self, sig: impl Into<String>) -> Self {
        self.sig = sig.into();
        self
    }

    pub fn verified(mut self, verified: bool) -> Self {
        self.verified = verified;
        self
    }

    pub fn category(&self) -> EventCategory {
        EventCategory::from_kind(self.kind)
    }

    pub fn format_time(&self) -> String {
        let secs = self.created_at;
        if secs == 0 {
            return "Unknown".to_string();
        }
        // Simple time formatting (would use proper time lib in production)
        format!("{}", secs)
    }

    pub fn truncated_id(&self) -> String {
        if self.id.len() > 16 {
            format!("{}...{}", &self.id[..8], &self.id[self.id.len() - 8..])
        } else {
            self.id.clone()
        }
    }

    pub fn truncated_pubkey(&self) -> String {
        if self.pubkey.len() > 16 {
            format!(
                "{}...{}",
                &self.pubkey[..8],
                &self.pubkey[self.pubkey.len() - 8..]
            )
        } else {
            self.pubkey.clone()
        }
    }
}

/// Event inspector organism
pub struct EventInspector {
    id: Option<ComponentId>,
    event: EventData,
    view: InspectorView,
    scroll_offset: f32,
    hovered_tab: Option<InspectorView>,
    copy_button_hovered: bool,
    on_copy: Option<Box<dyn FnMut(String)>>,
    on_view_change: Option<Box<dyn FnMut(InspectorView)>>,
}

impl EventInspector {
    pub fn new(event: EventData) -> Self {
        Self {
            id: None,
            event,
            view: InspectorView::Summary,
            scroll_offset: 0.0,
            hovered_tab: None,
            copy_button_hovered: false,
            on_copy: None,
            on_view_change: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn view(mut self, view: InspectorView) -> Self {
        self.view = view;
        self
    }

    pub fn on_copy<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_copy = Some(Box::new(f));
        self
    }

    pub fn on_view_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(InspectorView) + 'static,
    {
        self.on_view_change = Some(Box::new(f));
        self
    }

    fn header_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 50.0)
    }

    fn tabs_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 50.0,
            bounds.size.width,
            36.0,
        )
    }

    fn content_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 86.0,
            bounds.size.width,
            bounds.size.height - 86.0,
        )
    }

    fn tab_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let tabs = self.tabs_bounds(bounds);
        let tab_width = 80.0;
        let padding = 12.0;
        Bounds::new(
            tabs.origin.x + padding + index as f32 * (tab_width + 8.0),
            tabs.origin.y + 6.0,
            tab_width,
            24.0,
        )
    }

    fn copy_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 60.0,
            bounds.origin.y + 14.0,
            50.0,
            22.0,
        )
    }

    fn tab_views() -> &'static [InspectorView] {
        &[
            InspectorView::Summary,
            InspectorView::Raw,
            InspectorView::Tags,
            InspectorView::Signature,
        ]
    }

    fn tab_label(view: InspectorView) -> &'static str {
        match view {
            InspectorView::Summary => "Summary",
            InspectorView::Raw => "Raw",
            InspectorView::Tags => "Tags",
            InspectorView::Signature => "Sig",
        }
    }
}

impl Component for EventInspector {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let header = self.header_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(header).with_background(theme::bg::MUTED));

        // Category badge
        let category = self.event.category();
        let category_color = match category {
            EventCategory::Note => theme::accent::PRIMARY,
            EventCategory::Metadata => theme::status::INFO,
            EventCategory::DirectMessage => theme::status::WARNING,
            EventCategory::Zap => theme::status::SUCCESS,
            EventCategory::Deletion => theme::status::ERROR,
            _ => theme::text::MUTED,
        };

        let badge_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + 14.0,
            category.label().len() as f32 * 7.0 + 16.0,
            22.0,
        );
        cx.scene.draw_quad(
            Quad::new(badge_bounds)
                .with_background(category_color.with_alpha(0.2))
                .with_border(category_color, 1.0),
        );
        let badge_run = cx.text.layout(
            category.label(),
            Point::new(badge_bounds.origin.x + 8.0, badge_bounds.origin.y + 5.0),
            theme::font_size::XS,
            category_color,
        );
        cx.scene.draw_text(badge_run);

        // Event ID
        let id_text = format!("ID: {}", self.event.truncated_id());
        let id_run = cx.text.layout(
            &id_text,
            Point::new(
                badge_bounds.origin.x + badge_bounds.size.width + 12.0,
                bounds.origin.y + 18.0,
            ),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(id_run);

        // Verification indicator
        if self.event.verified {
            let check_run = cx.text.layout(
                "✓",
                Point::new(
                    bounds.origin.x + bounds.size.width - 100.0,
                    bounds.origin.y + 16.0,
                ),
                theme::font_size::SM,
                theme::status::SUCCESS,
            );
            cx.scene.draw_text(check_run);
        }

        // Copy button
        let copy_bounds = self.copy_button_bounds(&bounds);
        let copy_bg = if self.copy_button_hovered {
            theme::bg::HOVER
        } else {
            theme::bg::MUTED
        };
        cx.scene.draw_quad(
            Quad::new(copy_bounds)
                .with_background(copy_bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );
        let copy_run = cx.text.layout(
            "Copy",
            Point::new(copy_bounds.origin.x + 10.0, copy_bounds.origin.y + 5.0),
            theme::font_size::XS,
            theme::text::SECONDARY,
        );
        cx.scene.draw_text(copy_run);

        // Tabs
        let tabs = self.tabs_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(tabs).with_background(theme::bg::APP));

        for (i, view) in Self::tab_views().iter().enumerate() {
            let tab = self.tab_bounds(&bounds, i);
            let is_active = self.view == *view;
            let is_hovered = self.hovered_tab == Some(*view);

            let bg = if is_active {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::MUTED
            };

            cx.scene.draw_quad(Quad::new(tab).with_background(bg));

            let text_color = if is_active {
                theme::accent::PRIMARY
            } else {
                theme::text::SECONDARY
            };

            let tab_run = cx.text.layout(
                Self::tab_label(*view),
                Point::new(tab.origin.x + 8.0, tab.origin.y + 6.0),
                theme::font_size::XS,
                text_color,
            );
            cx.scene.draw_text(tab_run);
        }

        // Content area
        let content = self.content_bounds(&bounds);

        match self.view {
            InspectorView::Summary => {
                let mut y = content.origin.y + padding;

                // Pubkey
                let pubkey_label = cx.text.layout(
                    "Pubkey:",
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(pubkey_label);

                let pubkey_value = cx.text.layout(
                    &self.event.truncated_pubkey(),
                    Point::new(content.origin.x + padding + 60.0, y),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(pubkey_value);
                y += 24.0;

                // Kind
                let kind_label = cx.text.layout(
                    "Kind:",
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(kind_label);

                let kind_value = cx.text.layout(
                    &format!("{}", self.event.kind),
                    Point::new(content.origin.x + padding + 60.0, y),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(kind_value);
                y += 24.0;

                // Created at
                let time_label = cx.text.layout(
                    "Time:",
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(time_label);

                let time_value = cx.text.layout(
                    &self.event.format_time(),
                    Point::new(content.origin.x + padding + 60.0, y),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(time_value);
                y += 32.0;

                // Content preview
                let content_label = cx.text.layout(
                    "Content:",
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(content_label);
                y += 20.0;

                let preview = if self.event.content.len() > 200 {
                    format!("{}...", &self.event.content[..200])
                } else {
                    self.event.content.clone()
                };

                let content_preview = cx.text.layout(
                    &preview,
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(content_preview);
            }
            InspectorView::Raw => {
                // Show raw JSON representation
                let raw_text = format!(
                    "{{\n  \"id\": \"{}\",\n  \"pubkey\": \"{}\",\n  \"kind\": {},\n  \"created_at\": {},\n  \"content\": \"...\",\n  \"tags\": [...],\n  \"sig\": \"...\"\n}}",
                    self.event.truncated_id(),
                    self.event.truncated_pubkey(),
                    self.event.kind,
                    self.event.created_at
                );

                let raw_run = cx.text.layout(
                    &raw_text,
                    Point::new(content.origin.x + padding, content.origin.y + padding),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(raw_run);
            }
            InspectorView::Tags => {
                let mut y = content.origin.y + padding;

                if self.event.tags.is_empty() {
                    let empty_run = cx.text.layout(
                        "No tags",
                        Point::new(content.origin.x + padding, y),
                        theme::font_size::SM,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(empty_run);
                } else {
                    for tag in &self.event.tags {
                        let tag_str = format!("[{}]: {}", tag.key, tag.values.join(", "));
                        let tag_run = cx.text.layout(
                            &tag_str,
                            Point::new(content.origin.x + padding, y),
                            theme::font_size::XS,
                            theme::text::PRIMARY,
                        );
                        cx.scene.draw_text(tag_run);
                        y += 20.0;
                    }
                }
            }
            InspectorView::Signature => {
                let mut y = content.origin.y + padding;

                // Signature
                let sig_label = cx.text.layout(
                    "Signature:",
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(sig_label);
                y += 20.0;

                let sig_display = if self.event.sig.len() > 64 {
                    format!("{}...", &self.event.sig[..64])
                } else if self.event.sig.is_empty() {
                    "N/A".to_string()
                } else {
                    self.event.sig.clone()
                };

                let sig_run = cx.text.layout(
                    &sig_display,
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(sig_run);
                y += 32.0;

                // Verification status
                let status_label = cx.text.layout(
                    "Status:",
                    Point::new(content.origin.x + padding, y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(status_label);

                let (status_text, status_color) = if self.event.verified {
                    ("Verified ✓", theme::status::SUCCESS)
                } else {
                    ("Unverified", theme::text::MUTED)
                };

                let status_run = cx.text.layout(
                    status_text,
                    Point::new(content.origin.x + padding + 60.0, y),
                    theme::font_size::XS,
                    status_color,
                );
                cx.scene.draw_text(status_run);
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered_tab;
                let was_copy_hovered = self.copy_button_hovered;

                self.hovered_tab = None;
                self.copy_button_hovered = self.copy_button_bounds(&bounds).contains(point);

                for (i, view) in Self::tab_views().iter().enumerate() {
                    if self.tab_bounds(&bounds, i).contains(point) {
                        self.hovered_tab = Some(*view);
                        break;
                    }
                }

                if was_hovered != self.hovered_tab || was_copy_hovered != self.copy_button_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    // Check copy button
                    if self.copy_button_bounds(&bounds).contains(point) {
                        if let Some(ref mut callback) = self.on_copy {
                            callback(self.event.id.clone());
                        }
                        return EventResult::Handled;
                    }

                    // Check tab clicks
                    for (i, view) in Self::tab_views().iter().enumerate() {
                        if self.tab_bounds(&bounds, i).contains(point) {
                            self.view = *view;
                            if let Some(ref mut callback) = self.on_view_change {
                                callback(*view);
                            }
                            return EventResult::Handled;
                        }
                    }
                }
            }
            InputEvent::Scroll { dy, .. } => {
                let content = self.content_bounds(&bounds);
                if bounds.contains(Point::new(bounds.origin.x + 1.0, bounds.origin.y + 1.0)) {
                    self.scroll_offset = (self.scroll_offset - *dy * 20.0)
                        .max(0.0)
                        .min(content.size.height);
                    return EventResult::Handled;
                }
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (Some(400.0), Some(350.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_data_creation() {
        let event = EventData::new("abc123", "npub1xyz", 1)
            .content("Hello world")
            .created_at(1700000000)
            .verified(true);

        assert_eq!(event.id, "abc123");
        assert_eq!(event.pubkey, "npub1xyz");
        assert_eq!(event.kind, 1);
        assert_eq!(event.content, "Hello world");
        assert!(event.verified);
    }

    #[test]
    fn test_event_category() {
        assert_eq!(EventCategory::from_kind(0), EventCategory::Metadata);
        assert_eq!(EventCategory::from_kind(1), EventCategory::Note);
        assert_eq!(EventCategory::from_kind(4), EventCategory::DirectMessage);
        assert_eq!(EventCategory::from_kind(9735), EventCategory::Zap);
        assert_eq!(EventCategory::from_kind(30023), EventCategory::Marketplace);
    }

    #[test]
    fn test_truncated_id() {
        let event = EventData::new("abcdef1234567890abcdef1234567890", "pubkey", 1);
        let truncated = event.truncated_id();
        assert!(truncated.contains("..."));
        assert!(truncated.len() < event.id.len());
    }

    #[test]
    fn test_inspector_tabs() {
        assert_eq!(EventInspector::tab_views().len(), 4);
        assert_eq!(EventInspector::tab_label(InspectorView::Summary), "Summary");
        assert_eq!(EventInspector::tab_label(InspectorView::Raw), "Raw");
    }

    #[test]
    fn test_event_inspector() {
        let event = EventData::new("test_id", "test_pubkey", 1);
        let inspector = EventInspector::new(event).view(InspectorView::Tags);

        assert_eq!(inspector.view, InspectorView::Tags);
    }
}
