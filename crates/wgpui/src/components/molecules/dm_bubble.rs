//! DM bubble molecule for displaying direct messages.
//!
//! Shows individual DM with sender, timestamp, and encryption status.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Message direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DmDirection {
    Incoming,
    Outgoing,
}

/// Encryption status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EncryptionStatus {
    Encrypted,
    Decrypted,
    Failed,
}

impl EncryptionStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            EncryptionStatus::Encrypted => "\u{1F512}", // Lock
            EncryptionStatus::Decrypted => "\u{1F513}", // Unlock
            EncryptionStatus::Failed => "\u{26A0}",     // Warning
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            EncryptionStatus::Encrypted => Hsla::new(120.0, 0.6, 0.45, 1.0),
            EncryptionStatus::Decrypted => theme::text::MUTED,
            EncryptionStatus::Failed => Hsla::new(0.0, 0.7, 0.5, 1.0),
        }
    }
}

/// DM message info
#[derive(Debug, Clone)]
pub struct DmMessage {
    pub id: String,
    pub content: String,
    pub direction: DmDirection,
    pub timestamp: String,
    pub sender_name: Option<String>,
    pub encryption: EncryptionStatus,
    pub read: bool,
}

impl DmMessage {
    pub fn new(id: impl Into<String>, content: impl Into<String>, direction: DmDirection) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            direction,
            timestamp: "Just now".to_string(),
            sender_name: None,
            encryption: EncryptionStatus::Decrypted,
            read: true,
        }
    }

    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = timestamp.into();
        self
    }

    pub fn sender(mut self, name: impl Into<String>) -> Self {
        self.sender_name = Some(name.into());
        self
    }

    pub fn encryption(mut self, status: EncryptionStatus) -> Self {
        self.encryption = status;
        self
    }

    pub fn read(mut self, read: bool) -> Self {
        self.read = read;
        self
    }
}

/// DM bubble component
pub struct DmBubble {
    id: Option<ComponentId>,
    message: DmMessage,
    hovered: bool,
}

impl DmBubble {
    pub fn new(message: DmMessage) -> Self {
        Self {
            id: None,
            message,
            hovered: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Component for DmBubble {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let max_bubble_width = bounds.size.width * 0.75;
        let is_outgoing = self.message.direction == DmDirection::Outgoing;

        // Calculate bubble width based on content
        let content_width =
            (self.message.content.len() as f32 * 7.0).min(max_bubble_width - padding * 2.0);
        let bubble_width = content_width + padding * 2.0;

        // Position bubble based on direction
        let bubble_x = if is_outgoing {
            bounds.origin.x + bounds.size.width - bubble_width - padding
        } else {
            bounds.origin.x + padding
        };

        let bubble_bounds = Bounds::new(
            bubble_x,
            bounds.origin.y + 4.0,
            bubble_width,
            bounds.size.height - 8.0,
        );

        // Background color based on direction
        let bg = if is_outgoing {
            theme::accent::PRIMARY.with_alpha(0.2)
        } else if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };

        let border_color = if is_outgoing {
            theme::accent::PRIMARY
        } else {
            theme::border::DEFAULT
        };

        cx.scene.draw_quad(
            Quad::new(bubble_bounds)
                .with_background(bg)
                .with_border(border_color, 1.0),
        );

        let mut y = bubble_bounds.origin.y + 8.0;

        // Sender name (for incoming only)
        if !is_outgoing && let Some(sender) = &self.message.sender_name {
            let sender_run = cx.text.layout_mono(
                sender,
                Point::new(bubble_bounds.origin.x + padding, y),
                theme::font_size::XS,
                theme::accent::PRIMARY,
            );
            cx.scene.draw_text(sender_run);
            y += 16.0;
        }

        // Content
        let content = if self.message.content.len() > 100 {
            format!("{}...", &self.message.content[..97])
        } else {
            self.message.content.clone()
        };
        let content_run = cx.text.layout_mono(
            &content,
            Point::new(bubble_bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(content_run);

        // Footer: timestamp and encryption
        let footer_y = bubble_bounds.origin.y + bubble_bounds.size.height - 18.0;

        // Encryption status
        let enc_run = cx.text.layout_mono(
            self.message.encryption.icon(),
            Point::new(bubble_bounds.origin.x + padding, footer_y),
            theme::font_size::XS,
            self.message.encryption.color(),
        );
        cx.scene.draw_text(enc_run);

        // Timestamp
        let time_run = cx.text.layout_mono(
            &self.message.timestamp,
            Point::new(bubble_bounds.origin.x + padding + 18.0, footer_y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(time_run);

        // Read status (for outgoing)
        if is_outgoing && self.message.read {
            let read_run = cx.text.layout_mono(
                "\u{2713}\u{2713}", // Double check
                Point::new(
                    bubble_bounds.origin.x + bubble_width - padding - 16.0,
                    footer_y,
                ),
                theme::font_size::XS,
                Hsla::new(200.0, 0.6, 0.5, 1.0),
            );
            cx.scene.draw_text(read_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        if let InputEvent::MouseMove { x, y } = event {
            let was_hovered = self.hovered;
            self.hovered = bounds.contains(Point::new(*x, *y));
            if was_hovered != self.hovered {
                return EventResult::Handled;
            }
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let base_height = 60.0;
        let extra = if self.message.sender_name.is_some()
            && self.message.direction == DmDirection::Incoming
        {
            16.0
        } else {
            0.0
        };
        (None, Some(base_height + extra))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dm_message() {
        let msg = DmMessage::new("1", "Hello world!", DmDirection::Outgoing)
            .timestamp("2 min ago")
            .encryption(EncryptionStatus::Encrypted);

        assert_eq!(msg.content, "Hello world!");
        assert_eq!(msg.direction, DmDirection::Outgoing);
    }

    #[test]
    fn test_encryption_status() {
        assert!(EncryptionStatus::Encrypted.icon().len() > 0);
        assert!(EncryptionStatus::Failed.icon().len() > 0);
    }
}
