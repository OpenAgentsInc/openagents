//! Relay connection status indicator for Nostr relays.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Relay connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RelayStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
    Authenticating,
}

impl RelayStatus {
    pub fn label(&self) -> &'static str {
        match self {
            RelayStatus::Disconnected => "Disconnected",
            RelayStatus::Connecting => "Connecting",
            RelayStatus::Connected => "Connected",
            RelayStatus::Error => "Error",
            RelayStatus::Authenticating => "Auth",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            RelayStatus::Disconnected => "OFF",
            RelayStatus::Connecting => "...",
            RelayStatus::Connected => "ON",
            RelayStatus::Error => "ERR",
            RelayStatus::Authenticating => "AUTH",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            RelayStatus::Disconnected => Hsla::new(0.0, 0.0, 0.5, 1.0),
            RelayStatus::Connecting => Hsla::new(45.0, 0.9, 0.5, 1.0),
            RelayStatus::Connected => Hsla::new(120.0, 0.7, 0.45, 1.0),
            RelayStatus::Error => Hsla::new(0.0, 0.8, 0.5, 1.0),
            RelayStatus::Authenticating => Hsla::new(200.0, 0.7, 0.5, 1.0),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            RelayStatus::Disconnected => "○",
            RelayStatus::Connecting => "◐",
            RelayStatus::Connected => "●",
            RelayStatus::Error => "✕",
            RelayStatus::Authenticating => "◑",
        }
    }
}

/// A dot indicator showing relay status
pub struct RelayStatusDot {
    id: Option<ComponentId>,
    status: RelayStatus,
    size: f32,
    show_label: bool,
}

impl RelayStatusDot {
    pub fn new(status: RelayStatus) -> Self {
        Self {
            id: None,
            status,
            size: 10.0,
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

    pub fn status(&self) -> RelayStatus {
        self.status
    }
}

impl Component for RelayStatusDot {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();

        // Draw the dot
        let dot_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + (bounds.size.height - self.size) / 2.0,
            self.size,
            self.size,
        );
        cx.scene
            .draw_quad(Quad::new(dot_bounds).with_background(color));

        // Draw label if enabled
        if self.show_label {
            let label = self.status.short_label();
            let text_x = bounds.origin.x + self.size + 6.0;
            let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
            let run = cx.text.layout(
                label,
                Point::new(text_x, text_y),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(run);
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
            self.size + 40.0
        } else {
            self.size
        };
        (Some(width), Some(self.size))
    }
}

/// A badge showing relay status with label
pub struct RelayStatusBadge {
    id: Option<ComponentId>,
    status: RelayStatus,
}

impl RelayStatusBadge {
    pub fn new(status: RelayStatus) -> Self {
        Self { id: None, status }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Component for RelayStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.3, 0.15, 0.9);

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        // Label
        let label = self.status.label();
        let text_w = label.len() as f32 * 7.0;
        let text_x = bounds.origin.x + (bounds.size.width - text_w) / 2.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let run = cx.text.layout(
            label,
            Point::new(text_x, text_y),
            theme::font_size::XS,
            color,
        );
        cx.scene.draw_text(run);
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
        (Some(80.0), Some(22.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_status() {
        assert_eq!(RelayStatus::Connected.label(), "Connected");
        assert_eq!(RelayStatus::Error.short_label(), "ERR");
    }
}
