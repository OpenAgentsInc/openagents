//! Address card molecule for displaying Bitcoin/Lightning addresses.
//!
//! Shows an address with network indicator, copy button, and optional QR code hint.

use crate::components::atoms::BitcoinNetwork;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Address type enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddressType {
    Bitcoin,
    Lightning,
    Nostr,
}

impl AddressType {
    pub fn label(&self) -> &'static str {
        match self {
            AddressType::Bitcoin => "Bitcoin",
            AddressType::Lightning => "Lightning",
            AddressType::Nostr => "Nostr",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            AddressType::Bitcoin => Hsla::new(35.0, 0.9, 0.5, 1.0), // Orange
            AddressType::Lightning => Hsla::new(270.0, 0.7, 0.6, 1.0), // Purple
            AddressType::Nostr => Hsla::new(280.0, 0.6, 0.55, 1.0), // Violet
        }
    }
}

/// Address card component
pub struct AddressCard {
    id: Option<ComponentId>,
    address: String,
    address_type: AddressType,
    network: BitcoinNetwork,
    label: Option<String>,
    copied: bool,
    copy_hovered: bool,
    on_copy: Option<Box<dyn FnMut(String)>>,
}

impl AddressCard {
    pub fn new(address: impl Into<String>, address_type: AddressType) -> Self {
        Self {
            id: None,
            address: address.into(),
            address_type,
            network: BitcoinNetwork::Mainnet,
            label: None,
            copied: false,
            copy_hovered: false,
            on_copy: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn network(mut self, network: BitcoinNetwork) -> Self {
        self.network = network;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn on_copy<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_copy = Some(Box::new(f));
        self
    }

    fn truncated_address(&self) -> String {
        if self.address.len() > 24 {
            format!(
                "{}...{}",
                &self.address[..12],
                &self.address[self.address.len() - 8..]
            )
        } else {
            self.address.clone()
        }
    }

    fn copy_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        let btn_w = 60.0;
        let btn_h = 24.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - btn_w,
            bounds.origin.y + (bounds.size.height - btn_h) / 2.0,
            btn_w,
            btn_h,
        )
    }
}

impl Component for AddressCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        let border_color = self.address_type.color().with_alpha(0.3);
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(border_color, 1.0),
        );

        // Left color bar
        let bar_w = 4.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bar_w,
                bounds.size.height,
            ))
            .with_background(self.address_type.color()),
        );

        let content_x = bounds.origin.x + padding + bar_w;

        // Type badge
        let badge_w = 70.0;
        let badge_h = 20.0;
        let badge_bounds = Bounds::new(content_x, bounds.origin.y + 8.0, badge_w, badge_h);
        cx.scene.draw_quad(
            Quad::new(badge_bounds)
                .with_background(self.address_type.color().with_alpha(0.2))
                .with_border(self.address_type.color(), 1.0),
        );
        let type_label = cx.text.layout(
            self.address_type.label(),
            Point::new(badge_bounds.origin.x + 8.0, badge_bounds.origin.y + 3.0),
            theme::font_size::XS,
            self.address_type.color(),
        );
        cx.scene.draw_text(type_label);

        // Network indicator (for non-mainnet)
        if self.network != BitcoinNetwork::Mainnet {
            let net_x = content_x + badge_w + 8.0;
            let net_label = cx.text.layout(
                self.network.label(),
                Point::new(net_x, bounds.origin.y + 11.0),
                theme::font_size::XS,
                self.network.color(),
            );
            cx.scene.draw_text(net_label);
        }

        // Label (if any)
        if let Some(label) = &self.label {
            let label_run = cx.text.layout(
                label,
                Point::new(content_x, bounds.origin.y + 34.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);
        }

        // Address
        let addr_y = if self.label.is_some() {
            bounds.origin.y + 52.0
        } else {
            bounds.origin.y + 34.0
        };

        let addr_run = cx.text.layout(
            &self.truncated_address(),
            Point::new(content_x, addr_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(addr_run);

        // Copy button
        let copy_bounds = self.copy_button_bounds(&bounds);
        let copy_bg = if self.copy_hovered {
            theme::bg::HOVER
        } else if self.copied {
            Hsla::new(120.0, 0.5, 0.25, 1.0)
        } else {
            theme::bg::MUTED
        };
        cx.scene.draw_quad(
            Quad::new(copy_bounds)
                .with_background(copy_bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let copy_text = if self.copied { "Copied!" } else { "Copy" };
        let copy_color = if self.copied {
            Hsla::new(120.0, 0.7, 0.5, 1.0)
        } else {
            theme::text::PRIMARY
        };
        let copy_label = cx.text.layout(
            copy_text,
            Point::new(copy_bounds.origin.x + 8.0, copy_bounds.origin.y + 4.0),
            theme::font_size::XS,
            copy_color,
        );
        cx.scene.draw_text(copy_label);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let copy_bounds = self.copy_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.copy_hovered;
                self.copy_hovered = copy_bounds.contains(point);
                if was_hovered != self.copy_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    if copy_bounds.contains(point) {
                        self.copied = true;
                        if let Some(callback) = &mut self.on_copy {
                            callback(self.address.clone());
                        }
                        return EventResult::Handled;
                    }
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
        let height = if self.label.is_some() { 80.0 } else { 64.0 };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_card() {
        let card = AddressCard::new(
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            AddressType::Bitcoin,
        );
        assert_eq!(card.address_type, AddressType::Bitcoin);
    }

    #[test]
    fn test_truncated_address() {
        let card = AddressCard::new(
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            AddressType::Bitcoin,
        );
        let truncated = card.truncated_address();
        assert!(truncated.contains("..."));
    }

    #[test]
    fn test_address_types() {
        assert_eq!(AddressType::Bitcoin.label(), "Bitcoin");
        assert_eq!(AddressType::Lightning.label(), "Lightning");
        assert_eq!(AddressType::Nostr.label(), "Nostr");
    }
}
