//! Dock container for side panels

use wgpui::{Bounds, EventContext, EventResult, Hsla, InputEvent, PaintContext};

use super::Panel;

/// Position where a dock can be placed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockPosition {
    #[default]
    Left,
    Right,
    Bottom,
}

/// State tracking for a dock
#[derive(Debug, Clone)]
pub struct DockState {
    pub position: DockPosition,
    pub open: bool,
    pub size: f32,
    pub min_size: f32,
    pub max_size: f32,
}

impl DockState {
    pub fn left(size: f32) -> Self {
        Self {
            position: DockPosition::Left,
            open: true,
            size,
            min_size: 200.0,
            max_size: 500.0,
        }
    }

    pub fn right(size: f32) -> Self {
        Self {
            position: DockPosition::Right,
            open: true,
            size,
            min_size: 200.0,
            max_size: 500.0,
        }
    }

    pub fn bottom(size: f32) -> Self {
        Self {
            position: DockPosition::Bottom,
            open: false, // Bottom starts closed
            size,
            min_size: 100.0,
            max_size: 400.0,
        }
    }

    pub fn effective_size(&self) -> f32 {
        if self.open { self.size } else { 0.0 }
    }
}

/// A dock that can hold panels
pub struct Dock {
    state: DockState,
    panels: Vec<Box<dyn Panel>>,
    active_panel: usize,
    border_color: Hsla,
}

impl Dock {
    pub fn new(position: DockPosition, size: f32) -> Self {
        let state = match position {
            DockPosition::Left => DockState::left(size),
            DockPosition::Right => DockState::right(size),
            DockPosition::Bottom => DockState::bottom(size),
        };

        Self {
            state,
            panels: Vec::new(),
            active_panel: 0,
            border_color: Hsla::new(0.0, 0.0, 0.3, 0.5), // Subtle gray line
        }
    }

    pub fn border_color(mut self, color: Hsla) -> Self {
        self.border_color = color;
        self
    }

    pub fn add_panel(&mut self, panel: Box<dyn Panel>) {
        self.panels.push(panel);
    }

    pub fn toggle(&mut self) {
        self.state.open = !self.state.open;

        // Notify panels
        if let Some(panel) = self.panels.get_mut(self.active_panel) {
            if self.state.open {
                panel.on_show();
            } else {
                panel.on_hide();
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.state.open
    }

    pub fn effective_size(&self) -> f32 {
        self.state.effective_size()
    }

    pub fn position(&self) -> DockPosition {
        self.state.position
    }

    pub fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if !self.state.open {
            return;
        }

        // Paint active panel - panels handle their own HUD frame styling
        if let Some(panel) = self.panels.get_mut(self.active_panel) {
            let margin = 8.0;
            let content_bounds = Bounds::new(
                bounds.origin.x + margin,
                bounds.origin.y + margin,
                bounds.size.width - margin * 2.0,
                bounds.size.height - margin * 2.0,
            );
            panel.paint(content_bounds, cx);
        }
    }

    pub fn event(
        &mut self,
        event: &InputEvent,
        bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        if !self.state.open {
            return EventResult::Ignored;
        }

        // Delegate to active panel
        if let Some(panel) = self.panels.get_mut(self.active_panel) {
            return panel.event(event, bounds, cx);
        }

        EventResult::Ignored
    }
}
