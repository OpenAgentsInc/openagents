//! Market type badge for the unified marketplace.
//!
//! Distinguishes between Compute, Skills, Data, and Trajectory markets.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Market type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketType {
    #[default]
    Compute,
    Skills,
    Data,
    Trajectories,
}

impl MarketType {
    pub fn label(&self) -> &'static str {
        match self {
            MarketType::Compute => "Compute",
            MarketType::Skills => "Skills",
            MarketType::Data => "Data",
            MarketType::Trajectories => "Trajectories",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            MarketType::Compute => "◆",
            MarketType::Skills => "★",
            MarketType::Data => "⬡",
            MarketType::Trajectories => "↝",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            MarketType::Compute => Hsla::new(200.0, 0.8, 0.55, 1.0), // Blue
            MarketType::Skills => Hsla::new(280.0, 0.7, 0.55, 1.0),  // Purple
            MarketType::Data => Hsla::new(45.0, 0.8, 0.5, 1.0),      // Gold
            MarketType::Trajectories => Hsla::new(140.0, 0.7, 0.5, 1.0), // Green
        }
    }
}

/// Badge showing market type
pub struct MarketTypeBadge {
    id: Option<ComponentId>,
    market_type: MarketType,
    compact: bool,
}

impl MarketTypeBadge {
    pub fn new(market_type: MarketType) -> Self {
        Self {
            id: None,
            market_type,
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
}

impl Component for MarketTypeBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.market_type.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Icon
        let icon = self.market_type.icon();
        let icon_run = cx
            .text
            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
        cx.scene.draw_text(icon_run);

        if !self.compact {
            x += 14.0;
            // Label
            let label = self.market_type.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
        }
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
        if self.compact {
            (Some(28.0), Some(22.0))
        } else {
            let width = 12.0 + 14.0 + self.market_type.label().len() as f32 * 6.5;
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_type() {
        assert_eq!(MarketType::Compute.label(), "Compute");
        assert_eq!(MarketType::Skills.label(), "Skills");
    }
}
