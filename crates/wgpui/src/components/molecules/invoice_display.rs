use crate::components::atoms::{BitcoinAmount, PaymentStatus, PaymentStatusBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

/// Invoice type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InvoiceType {
    #[default]
    Bolt11,
    SparkAddress,
    OnChainAddress,
}

impl InvoiceType {
    pub fn label(&self) -> &'static str {
        match self {
            InvoiceType::Bolt11 => "Lightning Invoice",
            InvoiceType::SparkAddress => "Spark Address",
            InvoiceType::OnChainAddress => "Bitcoin Address",
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            InvoiceType::Bolt11 => "lnbc",
            InvoiceType::SparkAddress => "sp1",
            InvoiceType::OnChainAddress => "bc1",
        }
    }
}

/// Invoice data
#[derive(Debug, Clone)]
pub struct InvoiceInfo {
    pub invoice_type: InvoiceType,
    pub payment_request: String,
    pub amount_sats: Option<u64>,
    pub description: Option<String>,
    pub expiry: Option<String>,
    pub status: PaymentStatus,
}

impl InvoiceInfo {
    pub fn new(invoice_type: InvoiceType, payment_request: impl Into<String>) -> Self {
        Self {
            invoice_type,
            payment_request: payment_request.into(),
            amount_sats: None,
            description: None,
            expiry: None,
            status: PaymentStatus::Pending,
        }
    }

    pub fn amount(mut self, sats: u64) -> Self {
        self.amount_sats = Some(sats);
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn expiry(mut self, exp: impl Into<String>) -> Self {
        self.expiry = Some(exp.into());
        self
    }

    pub fn status(mut self, status: PaymentStatus) -> Self {
        self.status = status;
        self
    }

    /// Truncate the payment request for display
    pub fn truncated(&self) -> String {
        let pr = &self.payment_request;
        if pr.len() > 32 {
            format!("{}...{}", &pr[..16], &pr[pr.len() - 12..])
        } else {
            pr.clone()
        }
    }
}

pub struct InvoiceDisplay {
    id: Option<ComponentId>,
    invoice: InvoiceInfo,
    show_qr_placeholder: bool,
    compact: bool,
}

impl InvoiceDisplay {
    pub fn new(invoice: InvoiceInfo) -> Self {
        Self {
            id: None,
            invoice,
            show_qr_placeholder: true,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_qr(mut self, show: bool) -> Self {
        self.show_qr_placeholder = show;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn invoice(&self) -> &InvoiceInfo {
        &self.invoice
    }
}

impl Component for InvoiceDisplay {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 12.0;
        let mut y = bounds.origin.y + padding;

        // Header: Invoice type + status
        let header_run = cx.text.layout(
            self.invoice.invoice_type.label(),
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(header_run);

        // Status badge
        let badge_w = 72.0;
        let badge_h = 20.0;
        let badge_x = bounds.origin.x + bounds.size.width - padding - badge_w;
        let mut status_badge = PaymentStatusBadge::new(self.invoice.status);
        status_badge.paint(Bounds::new(badge_x, y - 2.0, badge_w, badge_h), cx);

        y += 24.0;

        // QR placeholder (if enabled and not compact)
        if self.show_qr_placeholder && !self.compact {
            let qr_size = 120.0;
            let qr_x = bounds.origin.x + (bounds.size.width - qr_size) / 2.0;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(qr_x, y, qr_size, qr_size))
                    .with_background(theme::bg::MUTED)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            // QR placeholder text
            let qr_text = cx.text.layout(
                "QR",
                Point::new(qr_x + qr_size / 2.0 - 10.0, y + qr_size / 2.0 - 8.0),
                theme::font_size::LG,
                theme::text::MUTED,
            );
            cx.scene.draw_text(qr_text);

            y += qr_size + 16.0;
        }

        // Amount (if present)
        if let Some(amount) = self.invoice.amount_sats {
            let amount_label = cx.text.layout(
                "Amount:",
                Point::new(bounds.origin.x + padding, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(amount_label);

            let mut amount_display = BitcoinAmount::new(amount).font_size(theme::font_size::LG);
            amount_display.paint(
                Bounds::new(bounds.origin.x + padding + 60.0, y - 4.0, 150.0, 24.0),
                cx,
            );
            y += 28.0;
        }

        // Description
        if let Some(ref desc) = self.invoice.description {
            let desc_label = cx.text.layout(
                desc,
                Point::new(bounds.origin.x + padding, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc_label);
            y += 20.0;
        }

        // Expiry
        if let Some(ref exp) = self.invoice.expiry {
            let exp_text = format!("Expires: {}", exp);
            let exp_run = cx.text.layout(
                &exp_text,
                Point::new(bounds.origin.x + padding, y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(exp_run);
            y += 20.0;
        }

        // Invoice string (truncated)
        let invoice_bg_bounds = Bounds::new(
            bounds.origin.x + padding,
            y,
            bounds.size.width - padding * 2.0,
            28.0,
        );
        cx.scene
            .draw_quad(Quad::new(invoice_bg_bounds).with_background(theme::bg::MUTED));

        let invoice_text = self.invoice.truncated();
        let inv_run = cx.text.layout(
            &invoice_text,
            Point::new(bounds.origin.x + padding + 8.0, y + 6.0),
            theme::font_size::XS,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(inv_run);

        // Copy hint
        let copy_hint = cx.text.layout(
            "Click to copy",
            Point::new(
                bounds.origin.x + bounds.size.width - padding - 70.0,
                y + 8.0,
            ),
            theme::font_size::XS,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(copy_hint);
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
        let height = if self.compact {
            120.0
        } else if self.show_qr_placeholder {
            280.0
        } else {
            160.0
        };
        (Some(320.0), Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoice_info() {
        let invoice = InvoiceInfo::new(InvoiceType::Bolt11, "lnbc1000n1...")
            .amount(1000)
            .description("Test payment");

        assert_eq!(invoice.amount_sats, Some(1000));
    }

    #[test]
    fn test_invoice_truncation() {
        let invoice = InvoiceInfo::new(
            InvoiceType::Bolt11,
            "lnbc100n1pn9xnxhpp5e5wfyknkdxqmz1234567890abcdefghijklmnop",
        );
        let truncated = invoice.truncated();
        assert!(truncated.contains("..."));
    }
}
