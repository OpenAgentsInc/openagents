//! Bounty badge for GitAfter (NIP-34 kind:1636).
//!
//! Displays bounty amount with optional expiry.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Bounty status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BountyStatus {
    #[default]
    Active,
    Claimed,
    Paid,
    Expired,
}

impl BountyStatus {
    pub fn label(&self) -> &'static str {
        match self {
            BountyStatus::Active => "Active",
            BountyStatus::Claimed => "Claimed",
            BountyStatus::Paid => "Paid",
            BountyStatus::Expired => "Expired",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            BountyStatus::Active => Hsla::new(45.0, 0.95, 0.55, 1.0), // Gold
            BountyStatus::Claimed => Hsla::new(200.0, 0.7, 0.5, 1.0), // Blue
            BountyStatus::Paid => Hsla::new(120.0, 0.7, 0.45, 1.0),   // Green
            BountyStatus::Expired => Hsla::new(0.0, 0.0, 0.5, 1.0),   // Gray
        }
    }
}

/// Badge displaying bounty amount
pub struct BountyBadge {
    id: Option<ComponentId>,
    amount_sats: u64,
    status: BountyStatus,
    show_icon: bool,
}

impl BountyBadge {
    pub fn new(amount_sats: u64) -> Self {
        Self {
            id: None,
            amount_sats,
            status: BountyStatus::Active,
            show_icon: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn status(mut self, status: BountyStatus) -> Self {
        self.status = status;
        self
    }

    pub fn show_icon(mut self, show: bool) -> Self {
        self.show_icon = show;
        self
    }

    /// Format sats with K/M suffix
    fn format_amount(&self) -> String {
        if self.amount_sats >= 1_000_000 {
            format!("{:.1}M", self.amount_sats as f64 / 1_000_000.0)
        } else if self.amount_sats >= 1_000 {
            format!("{}K", self.amount_sats / 1_000)
        } else {
            format!("{}", self.amount_sats)
        }
    }
}

impl Component for BountyBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.3, 0.1, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let mut x = bounds.origin.x + padding;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;

        // Lightning icon
        if self.show_icon {
            let icon_run = cx.text.layout_mono(
                "⚡",
                Point::new(x, text_y - 1.0),
                theme::font_size::SM,
                color,
            );
            cx.scene.draw_text(icon_run);
            x += 14.0;
        }

        // Amount
        let amount_text = self.format_amount();
        let amount_run = cx.text.layout_mono(
            &amount_text,
            Point::new(x, text_y),
            theme::font_size::XS,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(amount_run);
        x += amount_text.len() as f32 * 6.5 + 4.0;

        // Sats label
        let sats_run = cx.text.layout_mono(
            "sats",
            Point::new(x, text_y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(sats_run);
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
        let amount_len = self.format_amount().len();
        let width = if self.show_icon { 14.0 } else { 0.0 } + amount_len as f32 * 7.0 + 40.0;
        (Some(width), Some(24.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounty_format() {
        let badge = BountyBadge::new(50000);
        assert_eq!(badge.format_amount(), "50K");

        let badge = BountyBadge::new(1500000);
        assert_eq!(badge.format_amount(), "1.5M");

        let badge = BountyBadge::new(500);
        assert_eq!(badge.format_amount(), "500");
    }
}
