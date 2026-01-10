use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, theme};

/// Payment method types for Bitcoin/Lightning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaymentMethod {
    #[default]
    Lightning,
    Spark,
    OnChain,
    Token,
    Deposit,
    Withdraw,
}

impl PaymentMethod {
    pub fn icon(&self) -> &'static str {
        match self {
            PaymentMethod::Lightning => "⚡",
            PaymentMethod::Spark => "✦",
            PaymentMethod::OnChain => "₿",
            PaymentMethod::Token => "◈",
            PaymentMethod::Deposit => "↓",
            PaymentMethod::Withdraw => "↑",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PaymentMethod::Lightning => "Lightning",
            PaymentMethod::Spark => "Spark",
            PaymentMethod::OnChain => "On-chain",
            PaymentMethod::Token => "Token",
            PaymentMethod::Deposit => "Deposit",
            PaymentMethod::Withdraw => "Withdraw",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            PaymentMethod::Lightning => Hsla::new(45.0, 1.0, 0.5, 1.0), // Yellow/gold
            PaymentMethod::Spark => Hsla::new(280.0, 0.8, 0.6, 1.0),    // Purple
            PaymentMethod::OnChain => Hsla::new(35.0, 0.9, 0.55, 1.0),  // Orange (Bitcoin)
            PaymentMethod::Token => Hsla::new(180.0, 0.7, 0.5, 1.0),    // Cyan
            PaymentMethod::Deposit => theme::status::SUCCESS,
            PaymentMethod::Withdraw => theme::status::WARNING,
        }
    }
}

pub struct PaymentMethodIcon {
    id: Option<ComponentId>,
    method: PaymentMethod,
    size: f32,
    show_label: bool,
}

impl PaymentMethodIcon {
    pub fn new(method: PaymentMethod) -> Self {
        Self {
            id: None,
            method,
            size: 16.0,
            show_label: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    pub fn method(&self) -> PaymentMethod {
        self.method
    }

    pub fn set_method(&mut self, method: PaymentMethod) {
        self.method = method;
    }
}

impl Default for PaymentMethodIcon {
    fn default() -> Self {
        Self::new(PaymentMethod::default())
    }
}

impl Component for PaymentMethodIcon {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let icon = self.method.icon();
        let color = self.method.color();

        let text_run = cx.text.layout_mono(
            icon,
            Point::new(bounds.origin.x, bounds.origin.y),
            self.size,
            color,
        );
        cx.scene.draw_text(text_run);

        if self.show_label {
            let label = self.method.label();
            let label_x = bounds.origin.x + self.size + 4.0;
            let label_run = cx.text.layout_mono(
                label,
                Point::new(label_x, bounds.origin.y + 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);
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
        let width = if self.show_label {
            self.size + 60.0
        } else {
            self.size
        };
        (Some(width), Some(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payment_method_icon() {
        let icon = PaymentMethodIcon::new(PaymentMethod::Lightning);
        assert_eq!(icon.method(), PaymentMethod::Lightning);
    }

    #[test]
    fn test_payment_method_labels() {
        assert_eq!(PaymentMethod::Lightning.label(), "Lightning");
        assert_eq!(PaymentMethod::Spark.label(), "Spark");
        assert_eq!(PaymentMethod::OnChain.label(), "On-chain");
    }

    #[test]
    fn test_payment_method_icons() {
        assert_eq!(PaymentMethod::Lightning.icon(), "⚡");
        assert_eq!(PaymentMethod::Spark.icon(), "✦");
        assert_eq!(PaymentMethod::OnChain.icon(), "₿");
    }
}
