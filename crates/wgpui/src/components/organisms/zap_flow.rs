//! Zap flow organism for sending Lightning zaps.
//!
//! Provides a multi-step wizard for sending zaps to Nostr users.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Zap flow step
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ZapStep {
    #[default]
    SelectAmount,
    AddMessage,
    Confirm,
    Sending,
    Complete,
}

impl ZapStep {
    pub fn label(&self) -> &'static str {
        match self {
            ZapStep::SelectAmount => "Amount",
            ZapStep::AddMessage => "Message",
            ZapStep::Confirm => "Confirm",
            ZapStep::Sending => "Sending",
            ZapStep::Complete => "Done",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            ZapStep::SelectAmount => 0,
            ZapStep::AddMessage => 1,
            ZapStep::Confirm => 2,
            ZapStep::Sending => 3,
            ZapStep::Complete => 4,
        }
    }
}

/// Preset zap amounts
pub const ZAP_PRESETS: [u64; 6] = [21, 100, 500, 1000, 5000, 21000];

/// Zap flow organism
pub struct ZapFlow {
    id: Option<ComponentId>,
    step: ZapStep,
    recipient_name: String,
    recipient_npub: String,
    amount_sats: u64,
    custom_amount: String,
    message: String,
    preset_hovered: Option<usize>,
    custom_focused: bool,
    next_hovered: bool,
    back_hovered: bool,
    on_send: Option<Box<dyn FnMut(u64, String)>>,
    on_cancel: Option<Box<dyn FnMut()>>,
}

impl ZapFlow {
    pub fn new(recipient_name: impl Into<String>, recipient_npub: impl Into<String>) -> Self {
        Self {
            id: None,
            step: ZapStep::SelectAmount,
            recipient_name: recipient_name.into(),
            recipient_npub: recipient_npub.into(),
            amount_sats: ZAP_PRESETS[2], // Default 500 sats
            custom_amount: String::new(),
            message: String::new(),
            preset_hovered: None,
            custom_focused: false,
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

    pub fn on_send<F>(mut self, f: F) -> Self
    where
        F: FnMut(u64, String) + 'static,
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

    fn format_amount(&self, sats: u64) -> String {
        if sats >= 1_000_000 {
            format!("{:.2}M", sats as f64 / 1_000_000.0)
        } else if sats >= 1_000 {
            format!("{:.1}K", sats as f64 / 1_000.0)
        } else {
            format!("{}", sats)
        }
    }

    fn header_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 60.0)
    }

    fn preset_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let padding = 16.0;
        let cols = 3;
        let gap = 12.0;
        let btn_w = (bounds.size.width - padding * 2.0 - gap * 2.0) / cols as f32;
        let btn_h = 50.0;

        let row = index / cols;
        let col = index % cols;

        let x = bounds.origin.x + padding + col as f32 * (btn_w + gap);
        let y = bounds.origin.y + 100.0 + row as f32 * (btn_h + gap);

        Bounds::new(x, y, btn_w, btn_h)
    }

    fn next_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 16.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 100.0,
            bounds.origin.y + bounds.size.height - padding - 40.0,
            100.0,
            36.0,
        )
    }

    fn back_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 16.0;
        Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + bounds.size.height - padding - 40.0,
            80.0,
            36.0,
        )
    }

    fn short_npub(&self) -> String {
        if self.recipient_npub.len() > 20 {
            format!(
                "{}...{}",
                &self.recipient_npub[..12],
                &self.recipient_npub[self.recipient_npub.len() - 8..]
            )
        } else {
            self.recipient_npub.clone()
        }
    }
}

impl Component for ZapFlow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 16.0;
        let zap_color = Hsla::new(35.0, 0.9, 0.5, 1.0); // Bitcoin orange

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let header = self.header_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(header).with_background(zap_color.with_alpha(0.1)));

        // Lightning icon and title
        let title = format!("\u{26A1} Zap {}", self.recipient_name);
        let title_run = cx.text.layout(
            &title,
            Point::new(bounds.origin.x + padding, bounds.origin.y + 18.0),
            theme::font_size::BASE,
            zap_color,
        );
        cx.scene.draw_text(title_run);

        // Recipient npub
        let npub_run = cx.text.layout(
            &self.short_npub(),
            Point::new(bounds.origin.x + padding, bounds.origin.y + 40.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(npub_run);

        // Step indicator
        let step_y = bounds.origin.y + 70.0;
        let steps = [ZapStep::SelectAmount, ZapStep::AddMessage, ZapStep::Confirm];
        let step_width = (bounds.size.width - padding * 2.0) / steps.len() as f32;

        for (i, step) in steps.iter().enumerate() {
            let step_x = bounds.origin.x + padding + i as f32 * step_width;
            let is_current = *step == self.step;
            let is_complete = step.index() < self.step.index();

            let color = if is_current {
                zap_color
            } else if is_complete {
                Hsla::new(120.0, 0.6, 0.45, 1.0)
            } else {
                theme::text::DISABLED
            };

            let dot_radius = 8.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    step_x + step_width / 2.0 - dot_radius,
                    step_y,
                    dot_radius * 2.0,
                    dot_radius * 2.0,
                ))
                .with_background(color)
                .with_border(color, 1.0),
            );

            let label_run = cx.text.layout(
                step.label(),
                Point::new(step_x + step_width / 2.0 - 20.0, step_y + 20.0),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(label_run);
        }

        // Step content
        match self.step {
            ZapStep::SelectAmount => {
                // Preset amount buttons
                for (i, &amount) in ZAP_PRESETS.iter().enumerate() {
                    let btn = self.preset_bounds(&bounds, i);
                    let is_selected = self.amount_sats == amount;
                    let is_hovered = self.preset_hovered == Some(i);

                    let bg = if is_selected {
                        zap_color.with_alpha(0.3)
                    } else if is_hovered {
                        theme::bg::HOVER
                    } else {
                        theme::bg::MUTED
                    };
                    let border = if is_selected {
                        zap_color
                    } else {
                        theme::border::DEFAULT
                    };

                    cx.scene.draw_quad(
                        Quad::new(btn)
                            .with_background(bg)
                            .with_border(border, if is_selected { 2.0 } else { 1.0 }),
                    );

                    let amount_str = self.format_amount(amount);
                    let amount_run = cx.text.layout(
                        &amount_str,
                        Point::new(
                            btn.origin.x + btn.size.width / 2.0 - 15.0,
                            btn.origin.y + 10.0,
                        ),
                        theme::font_size::BASE,
                        if is_selected {
                            zap_color
                        } else {
                            theme::text::PRIMARY
                        },
                    );
                    cx.scene.draw_text(amount_run);

                    let sats_run = cx.text.layout(
                        "sats",
                        Point::new(
                            btn.origin.x + btn.size.width / 2.0 - 10.0,
                            btn.origin.y + 32.0,
                        ),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(sats_run);
                }

                // Custom amount field
                let custom_y = bounds.origin.y + 240.0;
                let custom_run = cx.text.layout(
                    "Or enter custom amount:",
                    Point::new(bounds.origin.x + padding, custom_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(custom_run);

                let custom_field = Bounds::new(
                    bounds.origin.x + padding,
                    custom_y + 20.0,
                    bounds.size.width - padding * 2.0,
                    36.0,
                );
                let field_border = if self.custom_focused {
                    zap_color
                } else {
                    theme::border::DEFAULT
                };
                cx.scene.draw_quad(
                    Quad::new(custom_field)
                        .with_background(theme::bg::APP)
                        .with_border(field_border, 1.0),
                );

                let placeholder = if self.custom_amount.is_empty() {
                    "Enter sats..."
                } else {
                    &self.custom_amount
                };
                let text_color = if self.custom_amount.is_empty() {
                    theme::text::DISABLED
                } else {
                    theme::text::PRIMARY
                };
                let custom_text_run = cx.text.layout(
                    placeholder,
                    Point::new(custom_field.origin.x + 8.0, custom_field.origin.y + 10.0),
                    theme::font_size::SM,
                    text_color,
                );
                cx.scene.draw_text(custom_text_run);
            }
            ZapStep::AddMessage => {
                let msg_label = cx.text.layout(
                    "Add a message (optional):",
                    Point::new(bounds.origin.x + padding, bounds.origin.y + 110.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(msg_label);

                let msg_field = Bounds::new(
                    bounds.origin.x + padding,
                    bounds.origin.y + 140.0,
                    bounds.size.width - padding * 2.0,
                    80.0,
                );
                cx.scene.draw_quad(
                    Quad::new(msg_field)
                        .with_background(theme::bg::APP)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let placeholder = if self.message.is_empty() {
                    "Write something nice..."
                } else {
                    &self.message
                };
                let text_color = if self.message.is_empty() {
                    theme::text::DISABLED
                } else {
                    theme::text::PRIMARY
                };
                let msg_run = cx.text.layout(
                    placeholder,
                    Point::new(msg_field.origin.x + 8.0, msg_field.origin.y + 8.0),
                    theme::font_size::SM,
                    text_color,
                );
                cx.scene.draw_text(msg_run);
            }
            ZapStep::Confirm => {
                let summary_y = bounds.origin.y + 110.0;

                let amount_label = cx.text.layout(
                    "Amount:",
                    Point::new(bounds.origin.x + padding, summary_y),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(amount_label);

                let amount_str = format!("\u{26A1} {} sats", self.format_amount(self.amount_sats));
                let amount_run = cx.text.layout(
                    &amount_str,
                    Point::new(bounds.origin.x + padding + 80.0, summary_y),
                    theme::font_size::BASE,
                    zap_color,
                );
                cx.scene.draw_text(amount_run);

                if !self.message.is_empty() {
                    let msg_label = cx.text.layout(
                        "Message:",
                        Point::new(bounds.origin.x + padding, summary_y + 30.0),
                        theme::font_size::SM,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(msg_label);

                    let msg_preview = if self.message.len() > 40 {
                        format!("\"{}...\"", &self.message[..37])
                    } else {
                        format!("\"{}\"", &self.message)
                    };
                    let msg_run = cx.text.layout(
                        &msg_preview,
                        Point::new(bounds.origin.x + padding + 80.0, summary_y + 30.0),
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(msg_run);
                }

                let to_label = cx.text.layout(
                    "To:",
                    Point::new(bounds.origin.x + padding, summary_y + 60.0),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(to_label);

                let to_run = cx.text.layout(
                    &self.recipient_name,
                    Point::new(bounds.origin.x + padding + 80.0, summary_y + 60.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(to_run);
            }
            ZapStep::Sending => {
                let center_y = bounds.origin.y + bounds.size.height / 2.0 - 30.0;
                let sending_run = cx.text.layout(
                    "\u{26A1} Sending zap...",
                    Point::new(bounds.origin.x + bounds.size.width / 2.0 - 60.0, center_y),
                    theme::font_size::BASE,
                    zap_color,
                );
                cx.scene.draw_text(sending_run);
            }
            ZapStep::Complete => {
                let center_y = bounds.origin.y + bounds.size.height / 2.0 - 30.0;
                let complete_run = cx.text.layout(
                    "\u{2713} Zap sent!",
                    Point::new(bounds.origin.x + bounds.size.width / 2.0 - 50.0, center_y),
                    theme::font_size::BASE,
                    Hsla::new(120.0, 0.6, 0.45, 1.0),
                );
                cx.scene.draw_text(complete_run);

                let amount_str = format!(
                    "{} sats to {}",
                    self.format_amount(self.amount_sats),
                    self.recipient_name
                );
                let amount_run = cx.text.layout(
                    &amount_str,
                    Point::new(
                        bounds.origin.x + bounds.size.width / 2.0 - 80.0,
                        center_y + 30.0,
                    ),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(amount_run);
            }
        }

        // Navigation buttons
        if self.step != ZapStep::Sending && self.step != ZapStep::Complete {
            // Back button
            if self.step != ZapStep::SelectAmount {
                let back_bounds = self.back_button_bounds(&bounds);
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
                let back_run = cx.text.layout(
                    "\u{2190} Back",
                    Point::new(back_bounds.origin.x + 14.0, back_bounds.origin.y + 10.0),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(back_run);
            }

            // Next/Send button
            let next_bounds = self.next_button_bounds(&bounds);
            let next_label = if self.step == ZapStep::Confirm {
                "Send Zap"
            } else {
                "Next \u{2192}"
            };
            let next_bg = if self.next_hovered {
                zap_color.with_alpha(0.4)
            } else {
                zap_color.with_alpha(0.3)
            };
            cx.scene.draw_quad(
                Quad::new(next_bounds)
                    .with_background(next_bg)
                    .with_border(zap_color, 1.0),
            );
            let next_run = cx.text.layout(
                next_label,
                Point::new(next_bounds.origin.x + 20.0, next_bounds.origin.y + 10.0),
                theme::font_size::SM,
                zap_color,
            );
            cx.scene.draw_text(next_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let next_bounds = self.next_button_bounds(&bounds);
        let back_bounds = self.back_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_next = self.next_hovered;
                let was_back = self.back_hovered;
                let was_preset = self.preset_hovered;

                self.next_hovered = next_bounds.contains(point);
                self.back_hovered = back_bounds.contains(point);

                // Check preset hover
                if self.step == ZapStep::SelectAmount {
                    self.preset_hovered = None;
                    for i in 0..ZAP_PRESETS.len() {
                        if self.preset_bounds(&bounds, i).contains(point) {
                            self.preset_hovered = Some(i);
                            break;
                        }
                    }
                }

                if was_next != self.next_hovered
                    || was_back != self.back_hovered
                    || was_preset != self.preset_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    // Preset selection
                    if self.step == ZapStep::SelectAmount {
                        for (i, &amount) in ZAP_PRESETS.iter().enumerate() {
                            if self.preset_bounds(&bounds, i).contains(point) {
                                self.amount_sats = amount;
                                self.custom_amount.clear();
                                return EventResult::Handled;
                            }
                        }
                    }

                    // Next button
                    if next_bounds.contains(point) {
                        match self.step {
                            ZapStep::SelectAmount => self.step = ZapStep::AddMessage,
                            ZapStep::AddMessage => self.step = ZapStep::Confirm,
                            ZapStep::Confirm => {
                                self.step = ZapStep::Sending;
                                // In real implementation, would trigger actual zap here
                                if let Some(callback) = &mut self.on_send {
                                    callback(self.amount_sats, self.message.clone());
                                }
                                self.step = ZapStep::Complete;
                            }
                            _ => {}
                        }
                        return EventResult::Handled;
                    }

                    // Back button
                    if back_bounds.contains(point) && self.step != ZapStep::SelectAmount {
                        match self.step {
                            ZapStep::AddMessage => self.step = ZapStep::SelectAmount,
                            ZapStep::Confirm => self.step = ZapStep::AddMessage,
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
        (None, Some(380.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zap_flow() {
        let flow = ZapFlow::new("Alice", "npub1abc...");
        assert_eq!(flow.step, ZapStep::SelectAmount);
        assert_eq!(flow.amount_sats, 500);
    }

    #[test]
    fn test_format_amount() {
        let flow = ZapFlow::new("Test", "npub...");
        assert_eq!(flow.format_amount(21), "21");
        assert_eq!(flow.format_amount(1500), "1.5K");
        assert_eq!(flow.format_amount(1500000), "1.50M");
    }
}
