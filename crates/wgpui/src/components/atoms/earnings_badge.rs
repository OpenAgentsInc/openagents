//! Earnings badge for marketplace revenue display.
//!
//! Shows earnings from compute, skills, data, and trajectory contributions.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Earnings type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EarningsType {
    #[default]
    Total,
    Compute,
    Skills,
    Data,
    Trajectories,
}

impl EarningsType {
    pub fn label(&self) -> &'static str {
        match self {
            EarningsType::Total => "Total",
            EarningsType::Compute => "Compute",
            EarningsType::Skills => "Skills",
            EarningsType::Data => "Data",
            EarningsType::Trajectories => "Trajectories",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            EarningsType::Total => "⚡",
            EarningsType::Compute => "◆",
            EarningsType::Skills => "★",
            EarningsType::Data => "⬡",
            EarningsType::Trajectories => "↝",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            EarningsType::Total => Hsla::new(45.0, 0.9, 0.55, 1.0), // Gold
            EarningsType::Compute => Hsla::new(200.0, 0.8, 0.55, 1.0), // Blue
            EarningsType::Skills => Hsla::new(280.0, 0.7, 0.55, 1.0), // Purple
            EarningsType::Data => Hsla::new(45.0, 0.7, 0.5, 1.0),   // Orange
            EarningsType::Trajectories => Hsla::new(140.0, 0.7, 0.5, 1.0), // Green
        }
    }
}

/// Format sats with K/M suffix
fn format_sats(sats: u64) -> String {
    if sats >= 1_000_000 {
        format!("{:.1}M", sats as f64 / 1_000_000.0)
    } else if sats >= 1_000 {
        format!("{:.1}K", sats as f64 / 1_000.0)
    } else {
        format!("{}", sats)
    }
}

/// Badge showing earnings
pub struct EarningsBadge {
    id: Option<ComponentId>,
    earnings_type: EarningsType,
    amount_sats: u64,
    show_lightning: bool,
    compact: bool,
}

impl EarningsBadge {
    pub fn new(earnings_type: EarningsType, amount_sats: u64) -> Self {
        Self {
            id: None,
            earnings_type,
            amount_sats,
            show_lightning: true,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_lightning(mut self, show: bool) -> Self {
        self.show_lightning = show;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for EarningsBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.earnings_type.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        if self.compact {
            // Just lightning and amount
            if self.show_lightning {
                let bolt = cx
                    .text
                    .layout("⚡", Point::new(x, text_y), theme::font_size::SM, color);
                cx.scene.draw_text(bolt);
                x += 14.0;
            }
            let amount = format_sats(self.amount_sats);
            let amount_run =
                cx.text
                    .layout(&amount, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(amount_run);
        } else {
            // Type icon
            let icon = self.earnings_type.icon();
            let icon_run = cx
                .text
                .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
            cx.scene.draw_text(icon_run);
            x += 14.0;

            // Type label
            let label = self.earnings_type.label();
            let label_run = cx.text.layout_mono(
                label,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);
            x += label.len() as f32 * 6.5 + 8.0;

            // Amount
            let amount = format_sats(self.amount_sats);
            let amount_run = cx.text.layout_mono(
                &amount,
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(amount_run);
            x += amount.len() as f32 * 6.5 + 4.0;

            // Sats label
            let sats_run = cx.text.layout_mono(
                "sats",
                Point::new(x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(sats_run);
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
            let amount_len = format_sats(self.amount_sats).len();
            let width = 12.0 + 14.0 + amount_len as f32 * 6.5;
            (Some(width), Some(22.0))
        } else {
            let amount_len = format_sats(self.amount_sats).len();
            let width = 12.0
                + 14.0
                + self.earnings_type.label().len() as f32 * 6.5
                + 8.0
                + amount_len as f32 * 6.5
                + 4.0
                + 4.0 * 6.5;
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sats() {
        assert_eq!(format_sats(500), "500");
        assert_eq!(format_sats(5000), "5.0K");
        assert_eq!(format_sats(5_000_000), "5.0M");
    }
}
