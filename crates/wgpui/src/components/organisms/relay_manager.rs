//! Relay manager organism for managing Nostr relay connections.
//!
//! Provides a panel for adding, removing, and monitoring relay connections.

use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::{RelayInfo, RelayRow};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Relay manager state
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum RelayManagerState {
    #[default]
    Viewing,
    Adding,
    Editing(usize),
}

/// Relay manager organism
pub struct RelayManager {
    id: Option<ComponentId>,
    relays: Vec<RelayInfo>,
    state: RelayManagerState,
    #[allow(dead_code)]
    add_url: String,
    hovered_relay: Option<usize>,
    add_button_hovered: bool,
    on_add: Option<Box<dyn FnMut(String)>>,
    on_remove: Option<Box<dyn FnMut(String)>>,
    on_toggle: Option<Box<dyn FnMut(String, bool)>>,
}

impl RelayManager {
    pub fn new(relays: Vec<RelayInfo>) -> Self {
        Self {
            id: None,
            relays,
            state: RelayManagerState::Viewing,
            add_url: String::new(),
            hovered_relay: None,
            add_button_hovered: false,
            on_add: None,
            on_remove: None,
            on_toggle: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_add<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_add = Some(Box::new(f));
        self
    }

    pub fn on_remove<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_remove = Some(Box::new(f));
        self
    }

    pub fn on_toggle<F>(mut self, f: F) -> Self
    where
        F: FnMut(String, bool) + 'static,
    {
        self.on_toggle = Some(Box::new(f));
        self
    }

    fn header_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 50.0)
    }

    fn add_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let padding = 16.0;
        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - 80.0,
            bounds.origin.y + 12.0,
            70.0,
            26.0,
        )
    }

    fn relay_row_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let header_height = 50.0;
        let row_height = 60.0;
        let y = bounds.origin.y + header_height + index as f32 * row_height;
        Bounds::new(bounds.origin.x, y, bounds.size.width, row_height)
    }

    fn stats_text(&self) -> String {
        let connected = self
            .relays
            .iter()
            .filter(|r| matches!(r.status, crate::components::atoms::RelayStatus::Connected))
            .count();
        format!("{}/{} connected", connected, self.relays.len())
    }
}

impl Component for RelayManager {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 16.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let header = self.header_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(header).with_background(theme::bg::MUTED));

        // Title
        let title_run = cx.text.layout_mono(
            "Relay Manager",
            Point::new(bounds.origin.x + padding, bounds.origin.y + 16.0),
            theme::font_size::BASE,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Connection stats
        let stats = self.stats_text();
        let stats_run = cx.text.layout_mono(
            &stats,
            Point::new(bounds.origin.x + padding + 140.0, bounds.origin.y + 18.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(stats_run);

        // Add button
        let add_bounds = self.add_button_bounds(&bounds);
        let add_bg = if self.add_button_hovered {
            theme::accent::PRIMARY.with_alpha(0.3)
        } else {
            theme::accent::PRIMARY.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(add_bounds)
                .with_background(add_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let add_run = cx.text.layout_mono(
            "+ Add",
            Point::new(add_bounds.origin.x + 14.0, add_bounds.origin.y + 7.0),
            theme::font_size::XS,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(add_run);

        // Relay list
        for (i, relay) in self.relays.iter().enumerate() {
            let row_bounds = self.relay_row_bounds(&bounds, i);
            let mut row = RelayRow::new(relay.clone());
            row.paint(row_bounds, cx);

            // Hover highlight
            if self.hovered_relay == Some(i) {
                cx.scene.draw_quad(
                    Quad::new(row_bounds).with_background(theme::bg::HOVER.with_alpha(0.3)),
                );
            }
        }

        // Empty state
        if self.relays.is_empty() {
            let empty_y = bounds.origin.y + bounds.size.height / 2.0 - 20.0;
            let empty_run = cx.text.layout_mono(
                "No relays configured",
                Point::new(bounds.origin.x + bounds.size.width / 2.0 - 80.0, empty_y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);

            let hint_run = cx.text.layout_mono(
                "Click + Add to connect to a relay",
                Point::new(
                    bounds.origin.x + bounds.size.width / 2.0 - 100.0,
                    empty_y + 24.0,
                ),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(hint_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let add_bounds = self.add_button_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_add_hovered = self.add_button_hovered;
                let was_relay_hovered = self.hovered_relay;

                self.add_button_hovered = add_bounds.contains(point);

                // Check relay hover
                self.hovered_relay = None;
                for i in 0..self.relays.len() {
                    let row_bounds = self.relay_row_bounds(&bounds, i);
                    if row_bounds.contains(point) {
                        self.hovered_relay = Some(i);
                        break;
                    }
                }

                if was_add_hovered != self.add_button_hovered
                    || was_relay_hovered != self.hovered_relay
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if add_bounds.contains(point) {
                        self.state = RelayManagerState::Adding;
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
        let header_height = 50.0;
        let row_height = 60.0;
        let min_rows = 3.0;
        let height = header_height + row_height * self.relays.len().max(min_rows as usize) as f32;
        (None, Some(height.min(400.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::atoms::RelayStatus;

    #[test]
    fn test_relay_manager() {
        let relays = vec![
            RelayInfo::new("wss://relay.damus.io").status(RelayStatus::Connected),
            RelayInfo::new("wss://nos.lol").status(RelayStatus::Connecting),
        ];
        let manager = RelayManager::new(relays);
        assert_eq!(manager.relays.len(), 2);
    }

    #[test]
    fn test_stats_text() {
        let relays = vec![
            RelayInfo::new("wss://relay1.com").status(RelayStatus::Connected),
            RelayInfo::new("wss://relay2.com").status(RelayStatus::Disconnected),
        ];
        let manager = RelayManager::new(relays);
        assert_eq!(manager.stats_text(), "1/2 connected");
    }
}
