//! Transaction row molecule for displaying transaction history.
//!
//! Shows a transaction with amount, type, status, and timestamp.

use crate::components::atoms::PaymentStatus;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Transaction direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionDirection {
    Incoming,
    Outgoing,
}

impl TransactionDirection {
    pub fn label(&self) -> &'static str {
        match self {
            TransactionDirection::Incoming => "Received",
            TransactionDirection::Outgoing => "Sent",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            TransactionDirection::Incoming => "+",
            TransactionDirection::Outgoing => "-",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            TransactionDirection::Incoming => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            TransactionDirection::Outgoing => Hsla::new(0.0, 0.8, 0.5, 1.0),    // Red
        }
    }
}

/// Transaction data
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub id: String,
    pub amount_sats: u64,
    pub direction: TransactionDirection,
    pub status: PaymentStatus,
    pub timestamp: String,
    pub description: Option<String>,
    pub fee_sats: Option<u64>,
}

impl TransactionInfo {
    pub fn new(id: impl Into<String>, amount_sats: u64, direction: TransactionDirection) -> Self {
        Self {
            id: id.into(),
            amount_sats,
            direction,
            status: PaymentStatus::Completed,
            timestamp: "Just now".to_string(),
            description: None,
            fee_sats: None,
        }
    }

    pub fn status(mut self, status: PaymentStatus) -> Self {
        self.status = status;
        self
    }

    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = timestamp.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn fee(mut self, fee_sats: u64) -> Self {
        self.fee_sats = Some(fee_sats);
        self
    }
}

/// Transaction row component
pub struct TransactionRow {
    id: Option<ComponentId>,
    transaction: TransactionInfo,
    hovered: bool,
    on_click: Option<Box<dyn FnMut(String)>>,
}

impl TransactionRow {
    pub fn new(transaction: TransactionInfo) -> Self {
        Self {
            id: None,
            transaction,
            hovered: false,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    fn format_amount(&self) -> String {
        let sats = self.transaction.amount_sats;
        if sats >= 100_000_000 {
            format!("{:.8} BTC", sats as f64 / 100_000_000.0)
        } else if sats >= 1_000_000 {
            format!("{:.2}M sats", sats as f64 / 1_000_000.0)
        } else if sats >= 1_000 {
            format!("{:.1}K sats", sats as f64 / 1_000.0)
        } else {
            format!("{} sats", sats)
        }
    }
}

impl Component for TransactionRow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
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

        // Direction indicator bar
        let bar_w = 3.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bar_w,
                bounds.size.height,
            ))
            .with_background(self.transaction.direction.color()),
        );

        let content_x = bounds.origin.x + padding + bar_w;
        let text_y = bounds.origin.y + 10.0;

        // Direction label
        let dir_label = cx.text.layout(
            self.transaction.direction.label(),
            Point::new(content_x, text_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(dir_label);

        // Description (if any)
        if let Some(desc) = &self.transaction.description {
            let desc_truncated = if desc.len() > 30 {
                format!("{}...", &desc[..27])
            } else {
                desc.clone()
            };
            let desc_run = cx.text.layout(
                &desc_truncated,
                Point::new(content_x + 80.0, text_y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc_run);
        }

        // Timestamp
        let time_run = cx.text.layout(
            &self.transaction.timestamp,
            Point::new(content_x, text_y + 20.0),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(time_run);

        // Status badge
        let status_x = content_x + 100.0;
        let status_color = self.transaction.status.color();
        let status_bounds = Bounds::new(status_x, text_y + 18.0, 60.0, 18.0);
        cx.scene.draw_quad(
            Quad::new(status_bounds)
                .with_background(status_color.with_alpha(0.15))
                .with_border(status_color, 1.0),
        );
        let status_label = cx.text.layout(
            self.transaction.status.label(),
            Point::new(status_bounds.origin.x + 4.0, status_bounds.origin.y + 2.0),
            theme::font_size::XS,
            status_color,
        );
        cx.scene.draw_text(status_label);

        // Amount (right aligned)
        let amount_text = format!(
            "{}{}",
            self.transaction.direction.symbol(),
            self.format_amount()
        );
        let amount_x = bounds.origin.x + bounds.size.width - padding - 120.0;
        let amount_run = cx.text.layout(
            &amount_text,
            Point::new(amount_x, text_y),
            theme::font_size::SM,
            self.transaction.direction.color(),
        );
        cx.scene.draw_text(amount_run);

        // Fee (if outgoing and has fee)
        if self.transaction.direction == TransactionDirection::Outgoing
            && let Some(fee) = self.transaction.fee_sats
        {
            let fee_text = format!("Fee: {} sats", fee);
            let fee_run = cx.text.layout(
                &fee_text,
                Point::new(amount_x, text_y + 20.0),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(fee_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(callback) = &mut self.on_click {
                        callback(self.transaction.id.clone());
                    }
                    return EventResult::Handled;
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
        (None, Some(56.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_info() {
        let tx = TransactionInfo::new("tx-1", 50000, TransactionDirection::Incoming)
            .status(PaymentStatus::Completed)
            .timestamp("5 min ago")
            .description("Payment from Alice");

        assert_eq!(tx.id, "tx-1");
        assert_eq!(tx.amount_sats, 50000);
    }

    #[test]
    fn test_format_amount() {
        let tx = TransactionInfo::new("tx-1", 1500, TransactionDirection::Incoming);
        let row = TransactionRow::new(tx);
        assert_eq!(row.format_amount(), "1.5K sats");
    }

    #[test]
    fn test_direction_colors() {
        assert!(TransactionDirection::Incoming.color().h > 100.0); // Green hue
        assert!(TransactionDirection::Outgoing.color().h < 10.0); // Red hue
    }
}
