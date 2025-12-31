//! Zap card molecule for displaying Lightning zaps.
//!
//! Shows zap amount, sender, message, and timestamp.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Zap info
#[derive(Debug, Clone)]
pub struct ZapInfo {
    pub id: String,
    pub amount_sats: u64,
    pub sender_name: Option<String>,
    pub sender_npub: String,
    pub message: Option<String>,
    pub timestamp: String,
    pub event_id: Option<String>,
}

impl ZapInfo {
    pub fn new(id: impl Into<String>, amount_sats: u64, sender_npub: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            amount_sats,
            sender_name: None,
            sender_npub: sender_npub.into(),
            message: None,
            timestamp: "Just now".to_string(),
            event_id: None,
        }
    }

    pub fn sender_name(mut self, name: impl Into<String>) -> Self {
        self.sender_name = Some(name.into());
        self
    }

    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = ts.into();
        self
    }

    pub fn event(mut self, event_id: impl Into<String>) -> Self {
        self.event_id = Some(event_id.into());
        self
    }

    fn short_npub(&self) -> String {
        if self.sender_npub.len() > 16 {
            format!("{}...", &self.sender_npub[..12])
        } else {
            self.sender_npub.clone()
        }
    }
}

/// Zap card component
pub struct ZapCard {
    id: Option<ComponentId>,
    zap: ZapInfo,
    hovered: bool,
    on_click: Option<Box<dyn FnMut(String)>>,
}

impl ZapCard {
    pub fn new(zap: ZapInfo) -> Self {
        Self {
            id: None,
            zap,
            hovered: false,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    fn format_amount(&self) -> String {
        let sats = self.zap.amount_sats;
        if sats >= 1_000_000 {
            format!("{:.2}M", sats as f64 / 1_000_000.0)
        } else if sats >= 1_000 {
            format!("{:.1}K", sats as f64 / 1_000.0)
        } else {
            format!("{}", sats)
        }
    }
}

impl Component for ZapCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let zap_color = Hsla::new(35.0, 0.9, 0.5, 1.0); // Bitcoin orange

        // Background
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Lightning bolt indicator
        let bolt_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, 4.0, bounds.size.height);
        cx.scene
            .draw_quad(Quad::new(bolt_bounds).with_background(zap_color));

        let mut y = bounds.origin.y + padding;

        // Zap icon and amount
        let zap_icon = "\u{26A1}"; // Lightning bolt
        let amount_text = format!("{} {} sats", zap_icon, self.format_amount());
        let amount_run = cx.text.layout(
            &amount_text,
            Point::new(bounds.origin.x + padding + 6.0, y),
            theme::font_size::SM,
            zap_color,
        );
        cx.scene.draw_text(amount_run);

        // Timestamp
        let time_run = cx.text.layout(
            &self.zap.timestamp,
            Point::new(bounds.origin.x + bounds.size.width - padding - 80.0, y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(time_run);

        y += 20.0;

        // Sender
        let sender = self
            .zap
            .sender_name
            .clone()
            .unwrap_or_else(|| self.zap.short_npub());
        let from_text = format!("from {}", sender);
        let from_run = cx.text.layout(
            &from_text,
            Point::new(bounds.origin.x + padding + 6.0, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(from_run);

        y += 18.0;

        // Message if present
        if let Some(msg) = &self.zap.message {
            let msg_truncated = if msg.len() > 60 {
                format!("\"{}...\"", &msg[..57])
            } else {
                format!("\"{}\"", msg)
            };
            let msg_run = cx.text.layout(
                &msg_truncated,
                Point::new(bounds.origin.x + padding + 6.0, y),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(msg_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == crate::MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(callback) = &mut self.on_click {
                        callback(self.zap.id.clone());
                    }
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
        let height = if self.zap.message.is_some() {
            80.0
        } else {
            65.0
        };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zap_info() {
        let zap = ZapInfo::new("z1", 50000, "npub1abc...")
            .sender_name("Alice")
            .message("Great post!")
            .timestamp("5 min ago");

        assert_eq!(zap.amount_sats, 50000);
        assert_eq!(zap.sender_name, Some("Alice".to_string()));
    }

    #[test]
    fn test_format_amount() {
        let zap = ZapInfo::new("z1", 1500000, "npub1...");
        let card = ZapCard::new(zap);
        assert_eq!(card.format_amount(), "1.50M");
    }
}
