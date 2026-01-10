//! Signing request card molecule for threshold signature approvals.
//!
//! Displays pending signing requests for FROSTR threshold signatures.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Signing request urgency
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SigningUrgency {
    Normal,
    Urgent,
    Expired,
}

impl SigningUrgency {
    pub fn label(&self) -> &'static str {
        match self {
            SigningUrgency::Normal => "Normal",
            SigningUrgency::Urgent => "Urgent",
            SigningUrgency::Expired => "Expired",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            SigningUrgency::Normal => theme::text::MUTED,
            SigningUrgency::Urgent => Hsla::new(30.0, 0.8, 0.5, 1.0), // Orange
            SigningUrgency::Expired => Hsla::new(0.0, 0.7, 0.5, 1.0), // Red
        }
    }
}

/// Signing request type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SigningType {
    Transaction,
    Message,
    Event,
    KeyRotation,
}

impl SigningType {
    pub fn label(&self) -> &'static str {
        match self {
            SigningType::Transaction => "Transaction",
            SigningType::Message => "Message",
            SigningType::Event => "Nostr Event",
            SigningType::KeyRotation => "Key Rotation",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SigningType::Transaction => "\u{20BF}",  // Bitcoin
            SigningType::Message => "\u{2709}",      // Envelope
            SigningType::Event => "\u{26A1}",        // Lightning
            SigningType::KeyRotation => "\u{1F511}", // Key
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            SigningType::Transaction => Hsla::new(35.0, 0.9, 0.5, 1.0), // Bitcoin orange
            SigningType::Message => Hsla::new(200.0, 0.6, 0.5, 1.0),    // Blue
            SigningType::Event => Hsla::new(280.0, 0.6, 0.55, 1.0),     // Purple
            SigningType::KeyRotation => Hsla::new(50.0, 0.7, 0.5, 1.0), // Gold
        }
    }
}

/// Signing request information
#[derive(Debug, Clone)]
pub struct SigningRequestInfo {
    pub id: String,
    pub signing_type: SigningType,
    pub description: String,
    pub requester: String,
    pub urgency: SigningUrgency,
    pub threshold: (u8, u8), // (current, required)
    pub expires_in: Option<String>,
    pub created_at: String,
}

impl SigningRequestInfo {
    pub fn new(
        id: impl Into<String>,
        signing_type: SigningType,
        description: impl Into<String>,
        requester: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            signing_type,
            description: description.into(),
            requester: requester.into(),
            urgency: SigningUrgency::Normal,
            threshold: (0, 2),
            expires_in: None,
            created_at: "Just now".to_string(),
        }
    }

    pub fn urgency(mut self, urgency: SigningUrgency) -> Self {
        self.urgency = urgency;
        self
    }

    pub fn threshold(mut self, current: u8, required: u8) -> Self {
        self.threshold = (current, required);
        self
    }

    pub fn expires_in(mut self, expires: impl Into<String>) -> Self {
        self.expires_in = Some(expires.into());
        self
    }

    pub fn created_at(mut self, ts: impl Into<String>) -> Self {
        self.created_at = ts.into();
        self
    }
}

/// Signing request card component
pub struct SigningRequestCard {
    id: Option<ComponentId>,
    request: SigningRequestInfo,
    hovered: bool,
    approve_hovered: bool,
    reject_hovered: bool,
    on_approve: Option<Box<dyn FnMut(String)>>,
    on_reject: Option<Box<dyn FnMut(String)>>,
    on_view: Option<Box<dyn FnMut(String)>>,
}

impl SigningRequestCard {
    pub fn new(request: SigningRequestInfo) -> Self {
        Self {
            id: None,
            request,
            hovered: false,
            approve_hovered: false,
            reject_hovered: false,
            on_approve: None,
            on_reject: None,
            on_view: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_approve<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_approve = Some(Box::new(f));
        self
    }

    pub fn on_reject<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_reject = Some(Box::new(f));
        self
    }

    pub fn on_view<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_view = Some(Box::new(f));
        self
    }

    fn approve_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 150.0,
            bounds.origin.y + bounds.size.height - padding - 26.0,
            65.0,
            24.0,
        )
    }

    fn reject_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 12.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 75.0,
            bounds.origin.y + bounds.size.height - padding - 26.0,
            65.0,
            24.0,
        )
    }
}

impl Component for SigningRequestCard {
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

        // Type indicator stripe
        let stripe_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, 4.0, bounds.size.height);
        cx.scene
            .draw_quad(Quad::new(stripe_bounds).with_background(self.request.signing_type.color()));

        let mut y = bounds.origin.y + padding;

        // Type icon and label
        let icon = self.request.signing_type.icon();
        let icon_run = cx.text.layout_mono(
            icon,
            Point::new(bounds.origin.x + padding + 6.0, y),
            theme::font_size::SM,
            self.request.signing_type.color(),
        );
        cx.scene.draw_text(icon_run);

        let type_label = self.request.signing_type.label();
        let label_run = cx.text.layout_mono(
            type_label,
            Point::new(bounds.origin.x + padding + 26.0, y),
            theme::font_size::SM,
            self.request.signing_type.color(),
        );
        cx.scene.draw_text(label_run);

        // Urgency badge
        if self.request.urgency != SigningUrgency::Normal {
            let urgency_w = (self.request.urgency.label().len() as f32 * 6.0) + 10.0;
            let urgency_x =
                bounds.origin.x + padding + 26.0 + (type_label.len() as f32 * 7.0) + 8.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(urgency_x, y - 1.0, urgency_w, 16.0))
                    .with_background(self.request.urgency.color().with_alpha(0.2))
                    .with_border(self.request.urgency.color(), 1.0),
            );
            let urgency_run = cx.text.layout_mono(
                self.request.urgency.label(),
                Point::new(urgency_x + 4.0, y),
                theme::font_size::XS,
                self.request.urgency.color(),
            );
            cx.scene.draw_text(urgency_run);
        }

        // Threshold progress
        let threshold_text = format!(
            "{}/{} signatures",
            self.request.threshold.0, self.request.threshold.1
        );
        let threshold_x = bounds.origin.x + bounds.size.width - padding - 90.0;
        let threshold_run = cx.text.layout_mono(
            &threshold_text,
            Point::new(threshold_x, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(threshold_run);

        y += 20.0;

        // Description
        let desc_truncated = if self.request.description.len() > 60 {
            format!("{}...", &self.request.description[..57])
        } else {
            self.request.description.clone()
        };
        let desc_run = cx.text.layout_mono(
            &desc_truncated,
            Point::new(bounds.origin.x + padding + 6.0, y),
            theme::font_size::XS,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(desc_run);

        y += 18.0;

        // Requester and time
        let requester_text = format!("from {}", self.request.requester);
        let requester_run = cx.text.layout_mono(
            &requester_text,
            Point::new(bounds.origin.x + padding + 6.0, y),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(requester_run);

        // Expires info
        if let Some(expires) = &self.request.expires_in {
            let expires_text = format!("expires {}", expires);
            let expires_run = cx.text.layout_mono(
                &expires_text,
                Point::new(bounds.origin.x + padding + 150.0, y),
                theme::font_size::XS,
                self.request.urgency.color(),
            );
            cx.scene.draw_text(expires_run);
        }

        // Approve button
        let approve_bounds = self.approve_button_bounds(&bounds);
        let approve_color = Hsla::new(120.0, 0.6, 0.45, 1.0);
        let approve_bg = if self.approve_hovered {
            approve_color.with_alpha(0.3)
        } else {
            approve_color.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(approve_bounds)
                .with_background(approve_bg)
                .with_border(approve_color, 1.0),
        );
        let approve_run = cx.text.layout_mono(
            "Approve",
            Point::new(approve_bounds.origin.x + 8.0, approve_bounds.origin.y + 6.0),
            theme::font_size::XS,
            approve_color,
        );
        cx.scene.draw_text(approve_run);

        // Reject button
        let reject_bounds = self.reject_button_bounds(&bounds);
        let reject_color = Hsla::new(0.0, 0.6, 0.5, 1.0);
        let reject_bg = if self.reject_hovered {
            reject_color.with_alpha(0.3)
        } else {
            reject_color.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(reject_bounds)
                .with_background(reject_bg)
                .with_border(reject_color, 1.0),
        );
        let reject_run = cx.text.layout_mono(
            "Reject",
            Point::new(reject_bounds.origin.x + 12.0, reject_bounds.origin.y + 6.0),
            theme::font_size::XS,
            reject_color,
        );
        cx.scene.draw_text(reject_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let approve_bounds = self.approve_button_bounds(&bounds);
        let reject_bounds = self.reject_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered;
                let was_approve = self.approve_hovered;
                let was_reject = self.reject_hovered;

                self.hovered = bounds.contains(point);
                self.approve_hovered = approve_bounds.contains(point);
                self.reject_hovered = reject_bounds.contains(point);

                if was_hovered != self.hovered
                    || was_approve != self.approve_hovered
                    || was_reject != self.reject_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if approve_bounds.contains(point) {
                        if let Some(callback) = &mut self.on_approve {
                            callback(self.request.id.clone());
                        }
                        return EventResult::Handled;
                    }

                    if reject_bounds.contains(point) {
                        if let Some(callback) = &mut self.on_reject {
                            callback(self.request.id.clone());
                        }
                        return EventResult::Handled;
                    }

                    if bounds.contains(point) {
                        if let Some(callback) = &mut self.on_view {
                            callback(self.request.id.clone());
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
        (None, Some(95.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_request_info() {
        let request = SigningRequestInfo::new(
            "sr1",
            SigningType::Transaction,
            "Send 0.05 BTC to bc1q...",
            "Agent-1",
        )
        .urgency(SigningUrgency::Urgent)
        .threshold(1, 2)
        .expires_in("5 minutes");

        assert_eq!(request.signing_type, SigningType::Transaction);
        assert_eq!(request.threshold, (1, 2));
    }

    #[test]
    fn test_signing_types() {
        assert_eq!(SigningType::Transaction.label(), "Transaction");
        assert_eq!(SigningType::Event.label(), "Nostr Event");
    }
}
