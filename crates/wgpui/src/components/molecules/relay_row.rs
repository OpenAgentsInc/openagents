//! Relay row component for displaying Nostr relay information.

use crate::components::atoms::{RelayStatus, RelayStatusDot};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

/// Information about a Nostr relay
#[derive(Debug, Clone)]
pub struct RelayInfo {
    pub url: String,
    pub status: RelayStatus,
    pub read: bool,
    pub write: bool,
    pub events_received: u64,
    pub events_sent: u64,
    pub latency_ms: Option<u32>,
}

impl RelayInfo {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            status: RelayStatus::Disconnected,
            read: true,
            write: true,
            events_received: 0,
            events_sent: 0,
            latency_ms: None,
        }
    }

    pub fn status(mut self, status: RelayStatus) -> Self {
        self.status = status;
        self
    }

    pub fn read(mut self, read: bool) -> Self {
        self.read = read;
        self
    }

    pub fn write(mut self, write: bool) -> Self {
        self.write = write;
        self
    }

    pub fn events(mut self, received: u64, sent: u64) -> Self {
        self.events_received = received;
        self.events_sent = sent;
        self
    }

    pub fn latency(mut self, ms: u32) -> Self {
        self.latency_ms = Some(ms);
        self
    }

    /// Extract display name from URL
    pub fn display_name(&self) -> String {
        self.url
            .trim_start_matches("wss://")
            .trim_start_matches("ws://")
            .trim_end_matches('/')
            .to_string()
    }
}

/// A row displaying relay information
pub struct RelayRow {
    id: Option<ComponentId>,
    relay: RelayInfo,
    hovered: bool,
    compact: bool,
}

impl RelayRow {
    pub fn new(relay: RelayInfo) -> Self {
        Self {
            id: None,
            relay,
            hovered: false,
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

    pub fn relay(&self) -> &RelayInfo {
        &self.relay
    }
}

impl Component for RelayRow {
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
        let status_size = 10.0;

        // Status dot
        let mut status_dot = RelayStatusDot::new(self.relay.status).size(status_size);
        status_dot.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + (bounds.size.height - status_size) / 2.0,
                status_size,
                status_size,
            ),
            cx,
        );

        // Relay name
        let name_x = bounds.origin.x + padding + status_size + 10.0;
        let name_y = bounds.origin.y + (bounds.size.height - theme::font_size::SM) / 2.0;

        let name = self.relay.display_name();
        let name_run = cx.text.layout(
            &name,
            Point::new(name_x, name_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        if !self.compact {
            // Read/Write indicators
            let rw_x = bounds.origin.x + 200.0;
            let rw_text = format!(
                "{}{}",
                if self.relay.read { "R" } else { "-" },
                if self.relay.write { "W" } else { "-" }
            );
            let rw_color = if self.relay.read && self.relay.write {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            };
            let rw_run = cx.text.layout(
                &rw_text,
                Point::new(rw_x, name_y),
                theme::font_size::XS,
                rw_color,
            );
            cx.scene.draw_text(rw_run);

            // Events count
            let events_x = bounds.origin.x + 250.0;
            let events_text = format!(
                "↓{} ↑{}",
                self.relay.events_received, self.relay.events_sent
            );
            let events_run = cx.text.layout(
                &events_text,
                Point::new(events_x, name_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(events_run);

            // Latency
            if let Some(latency) = self.relay.latency_ms {
                let latency_x = bounds.origin.x + bounds.size.width - padding - 50.0;
                let latency_text = format!("{}ms", latency);
                let latency_color = if latency < 100 {
                    theme::status::SUCCESS
                } else if latency < 300 {
                    theme::status::WARNING
                } else {
                    theme::status::ERROR
                };
                let latency_run = cx.text.layout(
                    &latency_text,
                    Point::new(latency_x, name_y),
                    theme::font_size::XS,
                    latency_color,
                );
                cx.scene.draw_text(latency_run);
            }
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
        let height = if self.compact { 36.0 } else { 44.0 };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_info() {
        let relay = RelayInfo::new("wss://relay.damus.io")
            .status(RelayStatus::Connected)
            .events(100, 50);

        assert_eq!(relay.display_name(), "relay.damus.io");
        assert_eq!(relay.events_received, 100);
    }
}
