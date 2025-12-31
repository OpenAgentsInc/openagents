use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Bitcoin network types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitcoinNetwork {
    #[default]
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

impl BitcoinNetwork {
    pub fn label(&self) -> &'static str {
        match self {
            BitcoinNetwork::Mainnet => "Mainnet",
            BitcoinNetwork::Testnet => "Testnet",
            BitcoinNetwork::Signet => "Signet",
            BitcoinNetwork::Regtest => "Regtest",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            BitcoinNetwork::Mainnet => "main",
            BitcoinNetwork::Testnet => "test",
            BitcoinNetwork::Signet => "sig",
            BitcoinNetwork::Regtest => "reg",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            BitcoinNetwork::Mainnet => Hsla::new(35.0, 0.9, 0.55, 1.0), // Bitcoin orange
            BitcoinNetwork::Testnet => Hsla::new(200.0, 0.7, 0.5, 1.0), // Blue
            BitcoinNetwork::Signet => Hsla::new(280.0, 0.6, 0.5, 1.0),  // Purple
            BitcoinNetwork::Regtest => theme::text::MUTED,
        }
    }

    pub fn is_production(&self) -> bool {
        matches!(self, BitcoinNetwork::Mainnet)
    }
}

pub struct NetworkBadge {
    id: Option<ComponentId>,
    network: BitcoinNetwork,
    compact: bool,
}

impl NetworkBadge {
    pub fn new(network: BitcoinNetwork) -> Self {
        Self {
            id: None,
            network,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn network(&self) -> BitcoinNetwork {
        self.network
    }

    pub fn set_network(&mut self, network: BitcoinNetwork) {
        self.network = network;
    }
}

impl Default for NetworkBadge {
    fn default() -> Self {
        Self::new(BitcoinNetwork::default())
    }
}

impl Component for NetworkBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.network.color();
        let bg = color.with_alpha(0.15);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let font_size = theme::font_size::XS;
        let text = if self.compact {
            self.network.short_label()
        } else {
            self.network.label()
        };

        let text_x = bounds.origin.x + 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - font_size) / 2.0;

        let text_run = cx
            .text
            .layout(text, Point::new(text_x, text_y), font_size, color);
        cx.scene.draw_text(text_run);
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let width = if self.compact { 36.0 } else { 64.0 };
        (Some(width), Some(20.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_badge() {
        let badge = NetworkBadge::new(BitcoinNetwork::Testnet);
        assert_eq!(badge.network(), BitcoinNetwork::Testnet);
    }

    #[test]
    fn test_network_labels() {
        assert_eq!(BitcoinNetwork::Mainnet.label(), "Mainnet");
        assert_eq!(BitcoinNetwork::Testnet.short_label(), "test");
    }

    #[test]
    fn test_is_production() {
        assert!(BitcoinNetwork::Mainnet.is_production());
        assert!(!BitcoinNetwork::Testnet.is_production());
    }
}
