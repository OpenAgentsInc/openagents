//! Bech32 entity display for Nostr (NIP-19).
//!
//! Displays npub, nsec, note, nevent, nprofile, naddr entities with
//! appropriate styling and truncation.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Type of bech32-encoded Nostr entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bech32Type {
    /// npub - Public key
    #[default]
    Npub,
    /// nsec - Private key (secret)
    Nsec,
    /// note - Event ID
    Note,
    /// nevent - Event with relay hints
    Nevent,
    /// nprofile - Profile with relay hints
    Nprofile,
    /// naddr - Parameterized replaceable event
    Naddr,
    /// nrelay - Relay URL
    Nrelay,
}

impl Bech32Type {
    pub fn prefix(&self) -> &'static str {
        match self {
            Bech32Type::Npub => "npub",
            Bech32Type::Nsec => "nsec",
            Bech32Type::Note => "note",
            Bech32Type::Nevent => "nevent",
            Bech32Type::Nprofile => "nprofile",
            Bech32Type::Naddr => "naddr",
            Bech32Type::Nrelay => "nrelay",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Bech32Type::Npub => "Public Key",
            Bech32Type::Nsec => "Private Key",
            Bech32Type::Note => "Note ID",
            Bech32Type::Nevent => "Event",
            Bech32Type::Nprofile => "Profile",
            Bech32Type::Naddr => "Address",
            Bech32Type::Nrelay => "Relay",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            Bech32Type::Npub => Hsla::new(280.0, 0.7, 0.6, 1.0), // Purple
            Bech32Type::Nsec => Hsla::new(0.0, 0.8, 0.55, 1.0),  // Red (danger)
            Bech32Type::Note => Hsla::new(180.0, 0.7, 0.5, 1.0), // Cyan
            Bech32Type::Nevent => Hsla::new(200.0, 0.7, 0.55, 1.0), // Blue
            Bech32Type::Nprofile => Hsla::new(320.0, 0.7, 0.55, 1.0), // Pink
            Bech32Type::Naddr => Hsla::new(50.0, 0.8, 0.5, 1.0), // Gold
            Bech32Type::Nrelay => Hsla::new(140.0, 0.6, 0.5, 1.0), // Green
        }
    }

    /// Try to detect type from string prefix
    pub fn from_str_prefix(s: &str) -> Option<Self> {
        if s.starts_with("npub") {
            Some(Bech32Type::Npub)
        } else if s.starts_with("nsec") {
            Some(Bech32Type::Nsec)
        } else if s.starts_with("note") {
            Some(Bech32Type::Note)
        } else if s.starts_with("nevent") {
            Some(Bech32Type::Nevent)
        } else if s.starts_with("nprofile") {
            Some(Bech32Type::Nprofile)
        } else if s.starts_with("naddr") {
            Some(Bech32Type::Naddr)
        } else if s.starts_with("nrelay") {
            Some(Bech32Type::Nrelay)
        } else {
            None
        }
    }

    pub fn is_secret(&self) -> bool {
        matches!(self, Bech32Type::Nsec)
    }
}

/// Display a bech32-encoded Nostr entity
pub struct Bech32Entity {
    id: Option<ComponentId>,
    entity_type: Bech32Type,
    value: String,
    truncate: bool,
    show_prefix_badge: bool,
    hide_secret: bool,
    hovered: bool,
}

impl Bech32Entity {
    pub fn new(entity_type: Bech32Type, value: impl Into<String>) -> Self {
        Self {
            id: None,
            entity_type,
            value: value.into(),
            truncate: true,
            show_prefix_badge: true,
            hide_secret: true,
            hovered: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn truncate(mut self, truncate: bool) -> Self {
        self.truncate = truncate;
        self
    }

    pub fn show_prefix_badge(mut self, show: bool) -> Self {
        self.show_prefix_badge = show;
        self
    }

    pub fn hide_secret(mut self, hide: bool) -> Self {
        self.hide_secret = hide;
        self
    }

    /// Get the truncated display value
    fn display_value(&self) -> String {
        if self.entity_type.is_secret() && self.hide_secret {
            return "••••••••".to_string();
        }

        if self.truncate && self.value.len() > 24 {
            format!(
                "{}...{}",
                &self.value[..12],
                &self.value[self.value.len() - 8..]
            )
        } else {
            self.value.clone()
        }
    }
}

impl From<&str> for Bech32Entity {
    fn from(value: &str) -> Self {
        let entity_type = Bech32Type::from_str_prefix(value).unwrap_or_default();
        Self::new(entity_type, value)
    }
}

impl From<String> for Bech32Entity {
    fn from(value: String) -> Self {
        let entity_type = Bech32Type::from_str_prefix(&value).unwrap_or_default();
        Self::new(entity_type, value)
    }
}

impl Component for Bech32Entity {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.entity_type.color();
        let bg = if self.hovered {
            Hsla::new(color.h, color.s * 0.2, 0.15, 0.95)
        } else {
            Hsla::new(color.h, color.s * 0.1, 0.1, 0.9)
        };

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color.with_alpha(0.5), 1.0),
        );

        let padding = 8.0;
        let mut x = bounds.origin.x + padding;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::SM) / 2.0;

        // Prefix badge
        if self.show_prefix_badge {
            let prefix = self.entity_type.prefix();
            let badge_w = prefix.len() as f32 * 7.5 + 10.0;
            let badge_h = 18.0;
            let badge_y = bounds.origin.y + (bounds.size.height - badge_h) / 2.0;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(x, badge_y, badge_w, badge_h))
                    .with_background(color.with_alpha(0.3))
                    .with_border(color, 1.0),
            );

            let prefix_run = cx.text.layout_mono(
                prefix,
                Point::new(x + 5.0, badge_y + 3.0),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(prefix_run);

            x += badge_w + 8.0;
        }

        // Value (possibly truncated)
        let display = self.display_value();
        let value_color = if self.entity_type.is_secret() && self.hide_secret {
            theme::text::MUTED
        } else {
            theme::text::PRIMARY
        };

        let value_run = cx.text.layout_mono(
            &display,
            Point::new(x, text_y),
            theme::font_size::SM,
            value_color,
        );
        cx.scene.draw_text(value_run);

        // Copy hint on hover
        if self.hovered && !self.entity_type.is_secret() {
            let hint = "Click to copy";
            let hint_x = bounds.origin.x + bounds.size.width - padding - hint.len() as f32 * 5.5;
            let hint_run = cx.text.layout_mono(
                hint,
                Point::new(hint_x, text_y),
                theme::font_size::XS,
                theme::accent::PRIMARY,
            );
            cx.scene.draw_text(hint_run);
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
        (Some(280.0), Some(32.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bech32_type_detection() {
        assert_eq!(
            Bech32Type::from_str_prefix("npub1abc..."),
            Some(Bech32Type::Npub)
        );
        assert_eq!(
            Bech32Type::from_str_prefix("nsec1xyz..."),
            Some(Bech32Type::Nsec)
        );
        assert_eq!(
            Bech32Type::from_str_prefix("note1..."),
            Some(Bech32Type::Note)
        );
        assert_eq!(Bech32Type::from_str_prefix("invalid"), None);
    }

    #[test]
    fn test_secret_detection() {
        assert!(Bech32Type::Nsec.is_secret());
        assert!(!Bech32Type::Npub.is_secret());
    }
}
