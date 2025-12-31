use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Payment status for Bitcoin transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaymentStatus {
    #[default]
    Pending,
    Completed,
    Failed,
    Expired,
}

impl PaymentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            PaymentStatus::Pending => "Pending",
            PaymentStatus::Completed => "Completed",
            PaymentStatus::Failed => "Failed",
            PaymentStatus::Expired => "Expired",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            PaymentStatus::Pending => "◌",
            PaymentStatus::Completed => "✓",
            PaymentStatus::Failed => "✗",
            PaymentStatus::Expired => "⊘",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            PaymentStatus::Pending => theme::status::WARNING,
            PaymentStatus::Completed => theme::status::SUCCESS,
            PaymentStatus::Failed => theme::status::ERROR,
            PaymentStatus::Expired => theme::text::MUTED,
        }
    }

    pub fn background(&self) -> Hsla {
        self.color().with_alpha(0.15)
    }
}

pub struct PaymentStatusBadge {
    id: Option<ComponentId>,
    status: PaymentStatus,
    compact: bool,
}

impl PaymentStatusBadge {
    pub fn new(status: PaymentStatus) -> Self {
        Self {
            id: None,
            status,
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

    pub fn status(&self) -> PaymentStatus {
        self.status
    }

    pub fn set_status(&mut self, status: PaymentStatus) {
        self.status = status;
    }
}

impl Default for PaymentStatusBadge {
    fn default() -> Self {
        Self::new(PaymentStatus::default())
    }
}

impl Component for PaymentStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = self.status.background();

        // Draw background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let font_size = theme::font_size::XS;
        let text = if self.compact {
            self.status.icon()
        } else {
            self.status.label()
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
        let width = if self.compact { 24.0 } else { 72.0 };
        (Some(width), Some(20.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_status_badge() {
        let badge = PaymentStatusBadge::new(PaymentStatus::Completed);
        assert_eq!(badge.status(), PaymentStatus::Completed);
    }

    #[test]
    fn test_payment_status_labels() {
        assert_eq!(PaymentStatus::Pending.label(), "Pending");
        assert_eq!(PaymentStatus::Completed.label(), "Completed");
        assert_eq!(PaymentStatus::Failed.label(), "Failed");
    }

    #[test]
    fn test_compact_mode() {
        let badge = PaymentStatusBadge::new(PaymentStatus::Pending).compact(true);
        let (w, _) = badge.size_hint();
        assert_eq!(w, Some(24.0));
    }
}
