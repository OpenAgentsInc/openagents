//! Receive flow organism for invoice generation wizard.
//!
//! Guides users through creating Lightning invoices or displaying receive addresses.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Receive type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReceiveType {
    Bitcoin,
    Lightning,
}

impl ReceiveType {
    pub fn label(&self) -> &'static str {
        match self {
            ReceiveType::Bitcoin => "Bitcoin",
            ReceiveType::Lightning => "Lightning",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            ReceiveType::Bitcoin => Hsla::new(35.0, 0.9, 0.5, 1.0), // Orange
            ReceiveType::Lightning => Hsla::new(270.0, 0.7, 0.6, 1.0), // Purple
        }
    }
}

/// Receive flow step
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReceiveStep {
    SelectType,
    SetAmount,
    ShowInvoice,
    Received,
}

impl ReceiveStep {
    pub fn label(&self) -> &'static str {
        match self {
            ReceiveStep::SelectType => "Select Type",
            ReceiveStep::SetAmount => "Set Amount",
            ReceiveStep::ShowInvoice => "Invoice",
            ReceiveStep::Received => "Complete",
        }
    }
}

/// Invoice state
#[derive(Debug, Clone)]
pub struct InvoiceState {
    pub receive_type: ReceiveType,
    pub amount_sats: Option<u64>,
    pub memo: String,
    pub invoice: String,
    pub address: String,
    pub expires_in: Option<u64>,
}

impl Default for InvoiceState {
    fn default() -> Self {
        Self {
            receive_type: ReceiveType::Lightning,
            amount_sats: None,
            memo: String::new(),
            invoice: String::new(),
            address: String::new(),
            expires_in: None,
        }
    }
}

/// Receive flow wizard organism
pub struct ReceiveFlow {
    id: Option<ComponentId>,
    step: ReceiveStep,
    state: InvoiceState,
    copied: bool,
    type_bitcoin_hovered: bool,
    type_lightning_hovered: bool,
    copy_hovered: bool,
    next_hovered: bool,
    on_create_invoice: Option<Box<dyn FnMut(InvoiceState)>>,
    on_done: Option<Box<dyn FnMut()>>,
}

impl ReceiveFlow {
    pub fn new() -> Self {
        Self {
            id: None,
            step: ReceiveStep::SelectType,
            state: InvoiceState::default(),
            copied: false,
            type_bitcoin_hovered: false,
            type_lightning_hovered: false,
            copy_hovered: false,
            next_hovered: false,
            on_create_invoice: None,
            on_done: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn step(mut self, step: ReceiveStep) -> Self {
        self.step = step;
        self
    }

    pub fn receive_type(mut self, receive_type: ReceiveType) -> Self {
        self.state.receive_type = receive_type;
        self
    }

    pub fn amount(mut self, amount_sats: u64) -> Self {
        self.state.amount_sats = Some(amount_sats);
        self
    }

    pub fn memo(mut self, memo: impl Into<String>) -> Self {
        self.state.memo = memo.into();
        self
    }

    pub fn invoice(mut self, invoice: impl Into<String>) -> Self {
        self.state.invoice = invoice.into();
        self
    }

    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.state.address = address.into();
        self
    }

    pub fn expires_in(mut self, seconds: u64) -> Self {
        self.state.expires_in = Some(seconds);
        self
    }

    pub fn on_create_invoice<F>(mut self, f: F) -> Self
    where
        F: FnMut(InvoiceState) + 'static,
    {
        self.on_create_invoice = Some(Box::new(f));
        self
    }

    pub fn on_done<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_done = Some(Box::new(f));
        self
    }

    fn format_amount(&self) -> String {
        match self.state.amount_sats {
            Some(sats) if sats >= 100_000_000 => {
                format!("{:.8} BTC", sats as f64 / 100_000_000.0)
            }
            Some(sats) => format!("{} sats", sats),
            None => "Any amount".to_string(),
        }
    }

    fn truncate_invoice(&self) -> String {
        let inv = if self.state.receive_type == ReceiveType::Lightning {
            &self.state.invoice
        } else {
            &self.state.address
        };

        if inv.len() > 48 {
            format!("{}...{}", &inv[..24], &inv[inv.len() - 16..])
        } else {
            inv.clone()
        }
    }

    fn type_button_bounds(&self, bounds: &Bounds) -> (Bounds, Bounds) {
        let padding = 16.0;
        let btn_h = 80.0;
        let btn_w = (bounds.size.width - padding * 3.0) / 2.0;
        let y = bounds.origin.y + 80.0;

        let bitcoin_btn = Bounds::new(bounds.origin.x + padding, y, btn_w, btn_h);
        let lightning_btn = Bounds::new(bounds.origin.x + padding * 2.0 + btn_w, y, btn_w, btn_h);

        (bitcoin_btn, lightning_btn)
    }

    fn copy_button_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(
            bounds.origin.x + bounds.size.width / 2.0 - 40.0,
            bounds.origin.y + bounds.size.height - 120.0,
            80.0,
            32.0,
        )
    }

    fn next_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 16.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 100.0,
            bounds.origin.y + bounds.size.height - padding - 36.0,
            100.0,
            36.0,
        )
    }
}

impl Default for ReceiveFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ReceiveFlow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 16.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let title = "Receive Payment";
        let title_run = cx.text.layout(
            title,
            Point::new(bounds.origin.x + padding, bounds.origin.y + 16.0),
            theme::font_size::LG,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Step indicator
        let step_text = format!("Step: {}", self.step.label());
        let step_run = cx.text.layout(
            &step_text,
            Point::new(bounds.origin.x + padding, bounds.origin.y + 44.0),
            theme::font_size::SM,
            theme::text::MUTED,
        );
        cx.scene.draw_text(step_run);

        match self.step {
            ReceiveStep::SelectType => {
                let prompt = cx.text.layout(
                    "Choose receive method:",
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 70.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(prompt);

                let (bitcoin_bounds, lightning_bounds) = self.type_button_bounds(&bounds);

                // Bitcoin button
                let btc_selected = self.state.receive_type == ReceiveType::Bitcoin;
                let btc_bg = if self.type_bitcoin_hovered || btc_selected {
                    ReceiveType::Bitcoin.color().with_alpha(0.2)
                } else {
                    theme::bg::MUTED
                };
                cx.scene.draw_quad(
                    Quad::new(bitcoin_bounds)
                        .with_background(btc_bg)
                        .with_border(
                            if btc_selected {
                                ReceiveType::Bitcoin.color()
                            } else {
                                theme::border::DEFAULT
                            },
                            if btc_selected { 2.0 } else { 1.0 },
                        ),
                );

                let btc_label = cx.text.layout(
                    "Bitcoin",
                    Point::new(
                        bitcoin_bounds.origin.x + 12.0,
                        bitcoin_bounds.origin.y + 12.0,
                    ),
                    theme::font_size::LG,
                    ReceiveType::Bitcoin.color(),
                );
                cx.scene.draw_text(btc_label);

                let btc_desc = cx.text.layout(
                    "On-chain transaction",
                    Point::new(
                        bitcoin_bounds.origin.x + 12.0,
                        bitcoin_bounds.origin.y + 40.0,
                    ),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(btc_desc);

                let btc_time = cx.text.layout(
                    "~10 min confirmation",
                    Point::new(
                        bitcoin_bounds.origin.x + 12.0,
                        bitcoin_bounds.origin.y + 56.0,
                    ),
                    theme::font_size::XS,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(btc_time);

                // Lightning button
                let ln_selected = self.state.receive_type == ReceiveType::Lightning;
                let ln_bg = if self.type_lightning_hovered || ln_selected {
                    ReceiveType::Lightning.color().with_alpha(0.2)
                } else {
                    theme::bg::MUTED
                };
                cx.scene.draw_quad(
                    Quad::new(lightning_bounds)
                        .with_background(ln_bg)
                        .with_border(
                            if ln_selected {
                                ReceiveType::Lightning.color()
                            } else {
                                theme::border::DEFAULT
                            },
                            if ln_selected { 2.0 } else { 1.0 },
                        ),
                );

                let ln_label = cx.text.layout(
                    "Lightning",
                    Point::new(
                        lightning_bounds.origin.x + 12.0,
                        lightning_bounds.origin.y + 12.0,
                    ),
                    theme::font_size::LG,
                    ReceiveType::Lightning.color(),
                );
                cx.scene.draw_text(ln_label);

                let ln_desc = cx.text.layout(
                    "Instant payment",
                    Point::new(
                        lightning_bounds.origin.x + 12.0,
                        lightning_bounds.origin.y + 40.0,
                    ),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(ln_desc);

                let ln_time = cx.text.layout(
                    "~1 second confirmation",
                    Point::new(
                        lightning_bounds.origin.x + 12.0,
                        lightning_bounds.origin.y + 56.0,
                    ),
                    theme::font_size::XS,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(ln_time);
            }
            ReceiveStep::SetAmount => {
                let label = cx.text.layout(
                    "Amount (optional)",
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 70.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(label);

                // Current amount
                let amount_text = self.format_amount();
                let amount_run = cx.text.layout(
                    &amount_text,
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 96.0),
                    24.0,
                    self.state.receive_type.color(),
                );
                cx.scene.draw_text(amount_run);

                // Memo field
                let memo_label = cx.text.layout(
                    "Memo (optional)",
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 140.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(memo_label);

                let memo_bounds = Bounds::new(
                    bounds.origin.x + padding,
                    bounds.origin.y + 164.0,
                    bounds.size.width - padding * 2.0,
                    36.0,
                );
                cx.scene.draw_quad(
                    Quad::new(memo_bounds)
                        .with_background(theme::bg::MUTED)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let memo_text = if self.state.memo.is_empty() {
                    "Add a note..."
                } else {
                    &self.state.memo
                };
                let memo_run = cx.text.layout(
                    memo_text,
                    Point::new(memo_bounds.origin.x + 12.0, memo_bounds.origin.y + 10.0),
                    theme::font_size::SM,
                    if self.state.memo.is_empty() {
                        theme::text::DISABLED
                    } else {
                        theme::text::PRIMARY
                    },
                );
                cx.scene.draw_text(memo_run);
            }
            ReceiveStep::ShowInvoice => {
                // Invoice type indicator
                let type_color = self.state.receive_type.color();
                let type_label = cx.text.layout(
                    self.state.receive_type.label(),
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 70.0),
                    theme::font_size::SM,
                    type_color,
                );
                cx.scene.draw_text(type_label);

                // Amount
                let amount_text = self.format_amount();
                let amount_run = cx.text.layout(
                    &amount_text,
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 94.0),
                    theme::font_size::LG,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(amount_run);

                // QR Code placeholder
                let qr_size = 120.0;
                let qr_x = bounds.origin.x + (bounds.size.width - qr_size) / 2.0;
                let qr_y = bounds.origin.y + 130.0;
                let qr_bounds = Bounds::new(qr_x, qr_y, qr_size, qr_size);
                cx.scene.draw_quad(
                    Quad::new(qr_bounds)
                        .with_background(Hsla::new(0.0, 0.0, 1.0, 1.0))
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // QR placeholder text
                let qr_text = cx.text.layout(
                    "[QR Code]",
                    Point::new(qr_x + 30.0, qr_y + 50.0),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(qr_text);

                // Invoice/Address text
                let invoice_text = self.truncate_invoice();
                let invoice_run = cx.text.layout(
                    &invoice_text,
                    Point::new(bounds.origin.x + padding, qr_y + qr_size + 16.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(invoice_run);

                // Copy button
                let copy_bounds = self.copy_button_bounds(&bounds);
                let copy_bg = if self.copy_hovered {
                    theme::bg::HOVER
                } else if self.copied {
                    Hsla::new(120.0, 0.5, 0.25, 1.0)
                } else {
                    theme::bg::MUTED
                };
                cx.scene.draw_quad(
                    Quad::new(copy_bounds)
                        .with_background(copy_bg)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let copy_text = if self.copied { "Copied!" } else { "Copy" };
                let copy_label = cx.text.layout(
                    copy_text,
                    Point::new(copy_bounds.origin.x + 20.0, copy_bounds.origin.y + 8.0),
                    theme::font_size::SM,
                    if self.copied {
                        Hsla::new(120.0, 0.7, 0.5, 1.0)
                    } else {
                        theme::text::PRIMARY
                    },
                );
                cx.scene.draw_text(copy_label);

                // Expiry (for Lightning)
                if let Some(expires) = self.state.expires_in {
                    let mins = expires / 60;
                    let expiry_text = format!("Expires in {} min", mins);
                    let expiry_run = cx.text.layout(
                        &expiry_text,
                        Point::new(bounds.origin.x + padding, copy_bounds.origin.y + 44.0),
                        theme::font_size::XS,
                        Hsla::new(45.0, 0.7, 0.5, 1.0), // Yellow warning
                    );
                    cx.scene.draw_text(expiry_run);
                }
            }
            ReceiveStep::Received => {
                let success = cx.text.layout(
                    "Payment Received!",
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 80.0),
                    theme::font_size::LG,
                    Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
                );
                cx.scene.draw_text(success);

                let amount_text = self.format_amount();
                let amount_run = cx.text.layout(
                    &format!("Received {}", amount_text),
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 110.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(amount_run);

                // Checkmark symbol
                let check_bounds = Bounds::new(
                    bounds.origin.x + bounds.size.width / 2.0 - 30.0,
                    bounds.origin.y + 150.0,
                    60.0,
                    60.0,
                );
                cx.scene.draw_quad(
                    Quad::new(check_bounds)
                        .with_background(Hsla::new(120.0, 0.5, 0.25, 1.0))
                        .with_border(Hsla::new(120.0, 0.7, 0.45, 1.0), 2.0),
                );
                let check = cx.text.layout(
                    "\u{2713}",
                    Point::new(check_bounds.origin.x + 18.0, check_bounds.origin.y + 14.0),
                    28.0,
                    Hsla::new(120.0, 0.7, 0.65, 1.0),
                );
                cx.scene.draw_text(check);
            }
        }

        // Next button
        let next_bounds = self.next_button_bounds(&bounds);
        let next_text = match self.step {
            ReceiveStep::SelectType => "Next",
            ReceiveStep::SetAmount => "Create Invoice",
            ReceiveStep::ShowInvoice => "Done",
            ReceiveStep::Received => "Close",
        };
        let next_bg = if self.next_hovered {
            theme::accent::PRIMARY.with_alpha(0.8)
        } else {
            theme::accent::PRIMARY
        };
        cx.scene.draw_quad(
            Quad::new(next_bounds)
                .with_background(next_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let next_label = cx.text.layout(
            next_text,
            Point::new(
                next_bounds.origin.x + (next_bounds.size.width - 60.0) / 2.0,
                next_bounds.origin.y + 10.0,
            ),
            theme::font_size::SM,
            Hsla::new(0.0, 0.0, 1.0, 1.0),
        );
        cx.scene.draw_text(next_label);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let (bitcoin_bounds, lightning_bounds) = self.type_button_bounds(&bounds);
        let copy_bounds = self.copy_button_bounds(&bounds);
        let next_bounds = self.next_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_btc = self.type_bitcoin_hovered;
                let was_ln = self.type_lightning_hovered;
                let was_copy = self.copy_hovered;
                let was_next = self.next_hovered;

                self.type_bitcoin_hovered = bitcoin_bounds.contains(point);
                self.type_lightning_hovered = lightning_bounds.contains(point);
                self.copy_hovered = copy_bounds.contains(point);
                self.next_hovered = next_bounds.contains(point);

                if was_btc != self.type_bitcoin_hovered
                    || was_ln != self.type_lightning_hovered
                    || was_copy != self.copy_hovered
                    || was_next != self.next_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if self.step == ReceiveStep::SelectType {
                        if bitcoin_bounds.contains(point) {
                            self.state.receive_type = ReceiveType::Bitcoin;
                            return EventResult::Handled;
                        }
                        if lightning_bounds.contains(point) {
                            self.state.receive_type = ReceiveType::Lightning;
                            return EventResult::Handled;
                        }
                    }

                    if self.step == ReceiveStep::ShowInvoice && copy_bounds.contains(point) {
                        self.copied = true;
                        return EventResult::Handled;
                    }

                    if next_bounds.contains(point) {
                        match self.step {
                            ReceiveStep::SelectType => self.step = ReceiveStep::SetAmount,
                            ReceiveStep::SetAmount => {
                                if let Some(callback) = &mut self.on_create_invoice {
                                    callback(self.state.clone());
                                }
                                self.step = ReceiveStep::ShowInvoice;
                            }
                            ReceiveStep::ShowInvoice | ReceiveStep::Received => {
                                if let Some(callback) = &mut self.on_done {
                                    callback();
                                }
                            }
                        }
                        return EventResult::Handled;
                    }
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
        (None, Some(380.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receive_flow() {
        let flow = ReceiveFlow::new()
            .receive_type(ReceiveType::Lightning)
            .amount(10000);

        assert_eq!(flow.step, ReceiveStep::SelectType);
        assert_eq!(flow.state.amount_sats, Some(10000));
    }

    #[test]
    fn test_receive_types() {
        assert_eq!(ReceiveType::Bitcoin.label(), "Bitcoin");
        assert_eq!(ReceiveType::Lightning.label(), "Lightning");
    }

    #[test]
    fn test_format_amount() {
        let flow = ReceiveFlow::new().amount(1_500_000);
        assert!(flow.format_amount().contains("1500000"));
    }
}
