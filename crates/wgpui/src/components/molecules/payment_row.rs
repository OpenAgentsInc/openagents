use crate::components::atoms::{
    AmountDirection, BitcoinAmount, PaymentMethod, PaymentMethodIcon, PaymentStatus,
    PaymentStatusBadge,
};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

/// Payment direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaymentDirection {
    #[default]
    Send,
    Receive,
}

impl PaymentDirection {
    pub fn label(&self) -> &'static str {
        match self {
            PaymentDirection::Send => "Sent",
            PaymentDirection::Receive => "Received",
        }
    }

    pub fn amount_direction(&self) -> AmountDirection {
        match self {
            PaymentDirection::Send => AmountDirection::Outgoing,
            PaymentDirection::Receive => AmountDirection::Incoming,
        }
    }
}

/// A single payment transaction
#[derive(Debug, Clone)]
pub struct PaymentInfo {
    pub id: String,
    pub amount_sats: u64,
    pub fee_sats: u64,
    pub direction: PaymentDirection,
    pub method: PaymentMethod,
    pub status: PaymentStatus,
    pub timestamp: String,
    pub description: Option<String>,
}

impl PaymentInfo {
    pub fn new(id: impl Into<String>, amount_sats: u64, direction: PaymentDirection) -> Self {
        Self {
            id: id.into(),
            amount_sats,
            fee_sats: 0,
            direction,
            method: PaymentMethod::Lightning,
            status: PaymentStatus::Completed,
            timestamp: String::new(),
            description: None,
        }
    }

    pub fn fee(mut self, sats: u64) -> Self {
        self.fee_sats = sats;
        self
    }

    pub fn method(mut self, method: PaymentMethod) -> Self {
        self.method = method;
        self
    }

    pub fn status(mut self, status: PaymentStatus) -> Self {
        self.status = status;
        self
    }

    pub fn timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = ts.into();
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

pub struct PaymentRow {
    id: Option<ComponentId>,
    payment: PaymentInfo,
    hovered: bool,
    show_fee: bool,
}

impl PaymentRow {
    pub fn new(payment: PaymentInfo) -> Self {
        Self {
            id: None,
            payment,
            hovered: false,
            show_fee: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_fee(mut self, show: bool) -> Self {
        self.show_fee = show;
        self
    }

    pub fn payment(&self) -> &PaymentInfo {
        &self.payment
    }
}

impl Component for PaymentRow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
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

        let padding = 12.0;
        let icon_size = 20.0;

        // Payment method icon
        let mut method_icon = PaymentMethodIcon::new(self.payment.method).size(icon_size);
        method_icon.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + (bounds.size.height - icon_size) / 2.0,
                icon_size,
                icon_size,
            ),
            cx,
        );

        // Direction and description
        let text_x = bounds.origin.x + padding + icon_size + 12.0;
        let direction_text = self.payment.direction.label();
        let dir_run = cx.text.layout(
            direction_text,
            Point::new(text_x, bounds.origin.y + 10.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(dir_run);

        // Description or method label
        let desc = self
            .payment
            .description
            .as_deref()
            .unwrap_or(self.payment.method.label());
        let desc_run = cx.text.layout(
            desc,
            Point::new(text_x, bounds.origin.y + 28.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(desc_run);

        // Amount on the right
        let amount_w = 100.0;
        let amount_x = bounds.origin.x + bounds.size.width - padding - amount_w - 80.0;
        let mut amount = BitcoinAmount::new(self.payment.amount_sats)
            .direction(self.payment.direction.amount_direction())
            .font_size(theme::font_size::BASE);
        amount.paint(
            Bounds::new(amount_x, bounds.origin.y + 10.0, amount_w, 24.0),
            cx,
        );

        // Fee if showing
        if self.show_fee && self.payment.fee_sats > 0 {
            let fee_text = format!("fee: {} sats", self.payment.fee_sats);
            let fee_run = cx.text.layout(
                &fee_text,
                Point::new(amount_x, bounds.origin.y + 32.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(fee_run);
        }

        // Status badge
        let badge_w = 72.0;
        let badge_h = 20.0;
        let badge_x = bounds.origin.x + bounds.size.width - padding - badge_w;
        let mut status_badge = PaymentStatusBadge::new(self.payment.status);
        status_badge.paint(
            Bounds::new(
                badge_x,
                bounds.origin.y + (bounds.size.height - badge_h) / 2.0,
                badge_w,
                badge_h,
            ),
            cx,
        );

        // Timestamp
        if !self.payment.timestamp.is_empty() {
            let ts_run = cx.text.layout(
                &self.payment.timestamp,
                Point::new(badge_x, bounds.origin.y + bounds.size.height - 18.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(ts_run);
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
        (None, Some(56.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_info() {
        let payment = PaymentInfo::new("tx123", 50000, PaymentDirection::Send)
            .fee(100)
            .method(PaymentMethod::Lightning)
            .status(PaymentStatus::Completed);

        assert_eq!(payment.amount_sats, 50000);
        assert_eq!(payment.fee_sats, 100);
    }

    #[test]
    fn test_payment_row() {
        let payment = PaymentInfo::new("tx456", 100000, PaymentDirection::Receive);
        let row = PaymentRow::new(payment);
        assert_eq!(row.payment().amount_sats, 100000);
    }
}
