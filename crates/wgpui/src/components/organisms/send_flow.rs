//! Send flow organism for multi-step send wizard.
//!
//! Guides users through the process of sending Bitcoin or Lightning payments.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Send flow step
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SendStep {
    EnterAddress,
    EnterAmount,
    Review,
    Confirm,
    Complete,
}

impl SendStep {
    pub fn index(&self) -> usize {
        match self {
            SendStep::EnterAddress => 0,
            SendStep::EnterAmount => 1,
            SendStep::Review => 2,
            SendStep::Confirm => 3,
            SendStep::Complete => 4,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SendStep::EnterAddress => "Address",
            SendStep::EnterAmount => "Amount",
            SendStep::Review => "Review",
            SendStep::Confirm => "Confirm",
            SendStep::Complete => "Complete",
        }
    }
}

/// Send flow state
#[derive(Debug, Clone, Default)]
pub struct SendFlowState {
    pub address: String,
    pub amount_sats: u64,
    pub fee_sats: u64,
    pub memo: String,
    pub is_lightning: bool,
}

/// Send flow wizard organism
pub struct SendFlow {
    id: Option<ComponentId>,
    step: SendStep,
    state: SendFlowState,
    error: Option<String>,
    next_hovered: bool,
    back_hovered: bool,
    on_send: Option<Box<dyn FnMut(SendFlowState)>>,
    on_cancel: Option<Box<dyn FnMut()>>,
}

impl SendFlow {
    pub fn new() -> Self {
        Self {
            id: None,
            step: SendStep::EnterAddress,
            state: SendFlowState::default(),
            error: None,
            next_hovered: false,
            back_hovered: false,
            on_send: None,
            on_cancel: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn step(mut self, step: SendStep) -> Self {
        self.step = step;
        self
    }

    pub fn address(mut self, address: impl Into<String>) -> Self {
        self.state.address = address.into();
        self
    }

    pub fn amount(mut self, amount_sats: u64) -> Self {
        self.state.amount_sats = amount_sats;
        self
    }

    pub fn fee(mut self, fee_sats: u64) -> Self {
        self.state.fee_sats = fee_sats;
        self
    }

    pub fn memo(mut self, memo: impl Into<String>) -> Self {
        self.state.memo = memo.into();
        self
    }

    pub fn lightning(mut self, is_lightning: bool) -> Self {
        self.state.is_lightning = is_lightning;
        self
    }

    pub fn error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub fn on_send<F>(mut self, f: F) -> Self
    where
        F: FnMut(SendFlowState) + 'static,
    {
        self.on_send = Some(Box::new(f));
        self
    }

    pub fn on_cancel<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_cancel = Some(Box::new(f));
        self
    }

    fn button_bounds(&self, bounds: &Bounds) -> (Bounds, Bounds) {
        let padding = 16.0;
        let btn_w = 100.0;
        let btn_h = 36.0;
        let footer_y = bounds.origin.y + bounds.size.height - padding - btn_h;

        let back_btn = Bounds::new(bounds.origin.x + padding, footer_y, btn_w, btn_h);
        let next_btn = Bounds::new(
            bounds.origin.x + bounds.size.width - padding - btn_w,
            footer_y,
            btn_w,
            btn_h,
        );

        (back_btn, next_btn)
    }

    fn format_amount(&self) -> String {
        let sats = self.state.amount_sats;
        if sats >= 100_000_000 {
            format!("{:.8} BTC", sats as f64 / 100_000_000.0)
        } else {
            format!("{} sats", sats)
        }
    }

    fn truncate_address(&self) -> String {
        if self.state.address.len() > 32 {
            format!(
                "{}...{}",
                &self.state.address[..16],
                &self.state.address[self.state.address.len() - 12..]
            )
        } else {
            self.state.address.clone()
        }
    }
}

impl Default for SendFlow {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SendFlow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 16.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let header_height = 48.0;
        let title = if self.state.is_lightning {
            "Send Lightning Payment"
        } else {
            "Send Bitcoin"
        };
        let title_run = cx.text.layout(
            title,
            Point::new(bounds.origin.x + padding, bounds.origin.y + 16.0),
            theme::font_size::LG,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Step indicator
        let steps = [
            SendStep::EnterAddress,
            SendStep::EnterAmount,
            SendStep::Review,
            SendStep::Confirm,
        ];
        let step_y = bounds.origin.y + header_height;
        let step_w = (bounds.size.width - padding * 2.0) / steps.len() as f32;

        for (i, step) in steps.iter().enumerate() {
            let x = bounds.origin.x + padding + i as f32 * step_w;
            let is_current = *step == self.step;
            let is_completed = step.index() < self.step.index();

            // Step dot
            let dot_radius = 10.0;
            let dot_x = x + step_w / 2.0 - dot_radius;
            let dot_bounds = Bounds::new(dot_x, step_y, dot_radius * 2.0, dot_radius * 2.0);

            let dot_color = if is_current {
                theme::accent::PRIMARY
            } else if is_completed {
                Hsla::new(120.0, 0.7, 0.45, 1.0) // Green
            } else {
                theme::bg::MUTED
            };
            cx.scene.draw_quad(
                Quad::new(dot_bounds)
                    .with_background(dot_color)
                    .with_border(dot_color, 1.0),
            );

            // Step number
            let num = format!("{}", i + 1);
            let num_run = cx.text.layout(
                &num,
                Point::new(dot_x + 6.0, step_y + 2.0),
                theme::font_size::XS,
                if is_current || is_completed {
                    Hsla::new(0.0, 0.0, 1.0, 1.0)
                } else {
                    theme::text::MUTED
                },
            );
            cx.scene.draw_text(num_run);

            // Step label
            let label_run = cx.text.layout(
                step.label(),
                Point::new(x + step_w / 2.0 - 20.0, step_y + 24.0),
                theme::font_size::XS,
                if is_current {
                    theme::text::PRIMARY
                } else {
                    theme::text::MUTED
                },
            );
            cx.scene.draw_text(label_run);

            // Connector line
            if i < steps.len() - 1 {
                let line_bounds = Bounds::new(
                    x + step_w / 2.0 + dot_radius + 4.0,
                    step_y + dot_radius - 1.0,
                    step_w - dot_radius * 2.0 - 8.0,
                    2.0,
                );
                let line_color = if is_completed {
                    Hsla::new(120.0, 0.7, 0.45, 1.0)
                } else {
                    theme::bg::MUTED
                };
                cx.scene
                    .draw_quad(Quad::new(line_bounds).with_background(line_color));
            }
        }

        // Content area
        let content_y = step_y + 56.0;
        let _content_height = bounds.size.height - header_height - 56.0 - 60.0; // Reserve footer space

        match self.step {
            SendStep::EnterAddress => {
                let label = cx.text.layout(
                    "Recipient Address",
                    Point::new(bounds.origin.x + padding, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(label);

                // Input field (simulated)
                let input_bounds = Bounds::new(
                    bounds.origin.x + padding,
                    content_y + 24.0,
                    bounds.size.width - padding * 2.0,
                    40.0,
                );
                cx.scene.draw_quad(
                    Quad::new(input_bounds)
                        .with_background(theme::bg::MUTED)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let placeholder = if self.state.address.is_empty() {
                    "Enter Bitcoin or Lightning address..."
                } else {
                    &self.state.address
                };
                let placeholder_run = cx.text.layout(
                    placeholder,
                    Point::new(input_bounds.origin.x + 12.0, input_bounds.origin.y + 12.0),
                    theme::font_size::SM,
                    if self.state.address.is_empty() {
                        theme::text::DISABLED
                    } else {
                        theme::text::PRIMARY
                    },
                );
                cx.scene.draw_text(placeholder_run);

                // Hint
                let hint = cx.text.layout(
                    "Supports Bitcoin addresses (bc1...) and Lightning invoices (lnbc...)",
                    Point::new(bounds.origin.x + padding, content_y + 72.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(hint);
            }
            SendStep::EnterAmount => {
                let label = cx.text.layout(
                    "Amount",
                    Point::new(bounds.origin.x + padding, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(label);

                // Amount display
                let amount_text = self.format_amount();
                let amount_run = cx.text.layout(
                    &amount_text,
                    Point::new(bounds.origin.x + padding, content_y + 24.0),
                    24.0,
                    theme::accent::PRIMARY,
                );
                cx.scene.draw_text(amount_run);

                // Quick amounts
                let quick_amounts = ["1K sats", "10K sats", "100K sats", "Max"];
                let quick_y = content_y + 70.0;
                for (i, amt) in quick_amounts.iter().enumerate() {
                    let btn_w = 80.0;
                    let x = bounds.origin.x + padding + i as f32 * (btn_w + 8.0);
                    let btn_bounds = Bounds::new(x, quick_y, btn_w, 28.0);
                    cx.scene.draw_quad(
                        Quad::new(btn_bounds)
                            .with_background(theme::bg::MUTED)
                            .with_border(theme::border::DEFAULT, 1.0),
                    );
                    let amt_run = cx.text.layout(
                        amt,
                        Point::new(x + 12.0, quick_y + 6.0),
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(amt_run);
                }
            }
            SendStep::Review => {
                // Summary
                let items = [
                    ("To:", &self.truncate_address()),
                    ("Amount:", &self.format_amount()),
                    ("Fee:", &format!("{} sats", self.state.fee_sats)),
                    (
                        "Total:",
                        &format!("{} sats", self.state.amount_sats + self.state.fee_sats),
                    ),
                ];

                for (i, (label, value)) in items.iter().enumerate() {
                    let y = content_y + i as f32 * 32.0;
                    let label_run = cx.text.layout(
                        label,
                        Point::new(bounds.origin.x + padding, y),
                        theme::font_size::SM,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    let value_run = cx.text.layout(
                        value,
                        Point::new(bounds.origin.x + padding + 80.0, y),
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(value_run);
                }
            }
            SendStep::Confirm => {
                let confirm_text = cx.text.layout(
                    "Confirm Transaction",
                    Point::new(bounds.origin.x + padding, content_y),
                    theme::font_size::LG,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(confirm_text);

                let warning = cx.text.layout(
                    "This action cannot be undone. Please verify all details.",
                    Point::new(bounds.origin.x + padding, content_y + 28.0),
                    theme::font_size::SM,
                    Hsla::new(45.0, 0.8, 0.5, 1.0), // Warning yellow
                );
                cx.scene.draw_text(warning);
            }
            SendStep::Complete => {
                let success_text = cx.text.layout(
                    "Transaction Sent!",
                    Point::new(bounds.origin.x + padding, content_y),
                    theme::font_size::LG,
                    Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
                );
                cx.scene.draw_text(success_text);

                let details = cx.text.layout(
                    &format!(
                        "Sent {} to {}",
                        self.format_amount(),
                        self.truncate_address()
                    ),
                    Point::new(bounds.origin.x + padding, content_y + 28.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(details);
            }
        }

        // Error message
        if let Some(error) = &self.error {
            let error_y = bounds.origin.y + bounds.size.height - 100.0;
            let error_run = cx.text.layout(
                error,
                Point::new(bounds.origin.x + padding, error_y),
                theme::font_size::SM,
                Hsla::new(0.0, 0.8, 0.5, 1.0), // Red
            );
            cx.scene.draw_text(error_run);
        }

        // Footer buttons
        let (back_bounds, next_bounds) = self.button_bounds(&bounds);

        // Back button (except on first and last steps)
        if self.step != SendStep::EnterAddress && self.step != SendStep::Complete {
            let back_bg = if self.back_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::MUTED
            };
            cx.scene.draw_quad(
                Quad::new(back_bounds)
                    .with_background(back_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );
            let back_label = cx.text.layout(
                "Back",
                Point::new(back_bounds.origin.x + 32.0, back_bounds.origin.y + 10.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(back_label);
        }

        // Next/Confirm/Done button
        let next_text = match self.step {
            SendStep::Confirm => "Send",
            SendStep::Complete => "Done",
            _ => "Next",
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
            Point::new(next_bounds.origin.x + 32.0, next_bounds.origin.y + 10.0),
            theme::font_size::SM,
            Hsla::new(0.0, 0.0, 1.0, 1.0), // White
        );
        cx.scene.draw_text(next_label);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let (back_bounds, next_bounds) = self.button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_back = self.back_hovered;
                let was_next = self.next_hovered;

                self.back_hovered = back_bounds.contains(point);
                self.next_hovered = next_bounds.contains(point);

                if was_back != self.back_hovered || was_next != self.next_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if next_bounds.contains(point) {
                        match self.step {
                            SendStep::EnterAddress => self.step = SendStep::EnterAmount,
                            SendStep::EnterAmount => self.step = SendStep::Review,
                            SendStep::Review => self.step = SendStep::Confirm,
                            SendStep::Confirm => {
                                if let Some(callback) = &mut self.on_send {
                                    callback(self.state.clone());
                                }
                                self.step = SendStep::Complete;
                            }
                            SendStep::Complete => {
                                if let Some(callback) = &mut self.on_cancel {
                                    callback();
                                }
                            }
                        }
                        return EventResult::Handled;
                    }

                    if back_bounds.contains(point) && self.step != SendStep::EnterAddress {
                        match self.step {
                            SendStep::EnterAmount => self.step = SendStep::EnterAddress,
                            SendStep::Review => self.step = SendStep::EnterAmount,
                            SendStep::Confirm => self.step = SendStep::Review,
                            _ => {}
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
        (None, Some(320.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_flow() {
        let flow = SendFlow::new()
            .address("bc1qtest...")
            .amount(50000)
            .fee(500);

        assert_eq!(flow.step, SendStep::EnterAddress);
        assert_eq!(flow.state.amount_sats, 50000);
    }

    #[test]
    fn test_step_progression() {
        assert_eq!(SendStep::EnterAddress.index(), 0);
        assert_eq!(SendStep::EnterAmount.index(), 1);
        assert_eq!(SendStep::Review.index(), 2);
    }

    #[test]
    fn test_format_amount() {
        let flow = SendFlow::new().amount(1_500_000);
        assert!(flow.format_amount().contains("1500000"));
    }
}
