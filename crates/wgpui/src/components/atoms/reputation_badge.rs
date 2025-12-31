//! Reputation badge for marketplace participants.
//!
//! Shows trust tier and success rate for providers.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Trust tier based on reputation score
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrustTier {
    #[default]
    New,
    Established,
    Trusted,
    Expert,
}

impl TrustTier {
    pub fn label(&self) -> &'static str {
        match self {
            TrustTier::New => "New",
            TrustTier::Established => "Established",
            TrustTier::Trusted => "Trusted",
            TrustTier::Expert => "Expert",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TrustTier::New => "○",
            TrustTier::Established => "◐",
            TrustTier::Trusted => "●",
            TrustTier::Expert => "★",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            TrustTier::New => Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
            TrustTier::Established => Hsla::new(45.0, 0.7, 0.5, 1.0), // Gold
            TrustTier::Trusted => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            TrustTier::Expert => Hsla::new(280.0, 0.7, 0.55, 1.0), // Purple
        }
    }

    pub fn from_score(score: u32) -> Self {
        if score >= 100 {
            TrustTier::Expert
        } else if score >= 50 {
            TrustTier::Trusted
        } else if score >= 10 {
            TrustTier::Established
        } else {
            TrustTier::New
        }
    }
}

/// Badge showing provider reputation
pub struct ReputationBadge {
    id: Option<ComponentId>,
    tier: TrustTier,
    success_rate: Option<f32>, // 0.0-1.0
    jobs_completed: Option<u32>,
    compact: bool,
}

impl ReputationBadge {
    pub fn new(tier: TrustTier) -> Self {
        Self {
            id: None,
            tier,
            success_rate: None,
            jobs_completed: None,
            compact: false,
        }
    }

    pub fn from_score(score: u32) -> Self {
        Self::new(TrustTier::from_score(score))
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn success_rate(mut self, rate: f32) -> Self {
        self.success_rate = Some(rate.clamp(0.0, 1.0));
        self
    }

    pub fn jobs_completed(mut self, count: u32) -> Self {
        self.jobs_completed = Some(count);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Component for ReputationBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.tier.color();
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
        let icon = self.tier.icon();
        let icon_run = cx
            .text
            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
        cx.scene.draw_text(icon_run);

        if !self.compact {
            x += 14.0;
            // Label
            let label = self.tier.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
            x += label.len() as f32 * 6.5 + 8.0;

            // Success rate
            if let Some(rate) = self.success_rate {
                let rate_text = format!("{}%", (rate * 100.0) as u8);
                let rate_color = if rate >= 0.95 {
                    theme::status::SUCCESS
                } else if rate >= 0.8 {
                    theme::status::WARNING
                } else {
                    theme::status::ERROR
                };
                let rate_run = cx.text.layout(
                    &rate_text,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    rate_color,
                );
                cx.scene.draw_text(rate_run);
            }
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
            let mut width = 12.0 + 14.0 + self.tier.label().len() as f32 * 6.5;
            if self.success_rate.is_some() {
                width += 40.0;
            }
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_tier() {
        assert_eq!(TrustTier::from_score(0), TrustTier::New);
        assert_eq!(TrustTier::from_score(10), TrustTier::Established);
        assert_eq!(TrustTier::from_score(50), TrustTier::Trusted);
        assert_eq!(TrustTier::from_score(100), TrustTier::Expert);
    }
}
