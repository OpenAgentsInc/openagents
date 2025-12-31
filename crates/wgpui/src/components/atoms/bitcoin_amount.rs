use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, theme};

/// Display unit for Bitcoin amounts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitcoinUnit {
    #[default]
    Sats,
    Btc,
    MSats,
}

impl BitcoinUnit {
    pub fn suffix(&self) -> &'static str {
        match self {
            BitcoinUnit::Sats => "sats",
            BitcoinUnit::Btc => "BTC",
            BitcoinUnit::MSats => "msats",
        }
    }
}

/// Direction of payment (for coloring)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmountDirection {
    #[default]
    Neutral,
    Incoming,
    Outgoing,
}

impl AmountDirection {
    pub fn prefix(&self) -> &'static str {
        match self {
            AmountDirection::Neutral => "",
            AmountDirection::Incoming => "+",
            AmountDirection::Outgoing => "-",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            AmountDirection::Neutral => theme::text::PRIMARY,
            AmountDirection::Incoming => theme::status::SUCCESS,
            AmountDirection::Outgoing => theme::status::ERROR,
        }
    }
}

pub struct BitcoinAmount {
    id: Option<ComponentId>,
    amount_sats: u64,
    unit: BitcoinUnit,
    direction: AmountDirection,
    font_size: f32,
    show_unit: bool,
}

impl BitcoinAmount {
    pub fn new(amount_sats: u64) -> Self {
        Self {
            id: None,
            amount_sats,
            unit: BitcoinUnit::Sats,
            direction: AmountDirection::Neutral,
            font_size: theme::font_size::BASE,
            show_unit: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn unit(mut self, unit: BitcoinUnit) -> Self {
        self.unit = unit;
        self
    }

    pub fn direction(mut self, direction: AmountDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn show_unit(mut self, show: bool) -> Self {
        self.show_unit = show;
        self
    }

    pub fn amount_sats(&self) -> u64 {
        self.amount_sats
    }

    pub fn set_amount(&mut self, sats: u64) {
        self.amount_sats = sats;
    }

    fn format_amount(&self) -> String {
        match self.unit {
            BitcoinUnit::Sats => {
                // Format with thousand separators
                let s = self.amount_sats.to_string();
                let mut result = String::new();
                for (i, c) in s.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        result.insert(0, ',');
                    }
                    result.insert(0, c);
                }
                result
            }
            BitcoinUnit::Btc => {
                let btc = self.amount_sats as f64 / 100_000_000.0;
                format!("{:.8}", btc)
            }
            BitcoinUnit::MSats => {
                let msats = self.amount_sats * 1000;
                msats.to_string()
            }
        }
    }
}

impl Default for BitcoinAmount {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Component for BitcoinAmount {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.direction.color();
        let prefix = self.direction.prefix();
        let amount = self.format_amount();
        let suffix = if self.show_unit {
            format!(" {}", self.unit.suffix())
        } else {
            String::new()
        };

        let text = format!("{}{}{}", prefix, amount, suffix);
        let text_y = bounds.origin.y + (bounds.size.height - self.font_size) / 2.0;

        let text_run = cx.text.layout(
            &text,
            Point::new(bounds.origin.x, text_y),
            self.font_size,
            color,
        );
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
        // Estimate width based on typical amount length
        let width = self.font_size * 8.0;
        (Some(width), Some(self.font_size + 4.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitcoin_amount() {
        let amount = BitcoinAmount::new(100000);
        assert_eq!(amount.amount_sats(), 100000);
    }

    #[test]
    fn test_format_sats() {
        let amount = BitcoinAmount::new(1234567);
        assert_eq!(amount.format_amount(), "1,234,567");
    }

    #[test]
    fn test_format_btc() {
        let amount = BitcoinAmount::new(100_000_000).unit(BitcoinUnit::Btc);
        assert_eq!(amount.format_amount(), "1.00000000");
    }

    #[test]
    fn test_direction_colors() {
        assert_eq!(AmountDirection::Incoming.prefix(), "+");
        assert_eq!(AmountDirection::Outgoing.prefix(), "-");
    }
}
