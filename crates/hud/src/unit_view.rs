//! UnitView: GPUI visualization of a Unit
//!
//! Renders a unit as a box with input pins on the left and output pins on the right.
//! Shows the unit's name, lifecycle state, and any errors.

use gpui::{
    App, Context, Entity, EventEmitter, Hsla, Pixels, Point, Render, Window,
    div, hsla, point, prelude::*, px,
};
use unit::{Lifecycle, Unit};

use crate::pin_view::{PinDirection, PinSnapshot, PinView};

/// Unit visual style configuration
#[derive(Debug, Clone)]
pub struct UnitStyle {
    /// Background color when playing
    pub playing_bg: Hsla,
    /// Background color when paused
    pub paused_bg: Hsla,
    /// Background color when has error
    pub error_bg: Hsla,
    /// Border color
    pub border_color: Hsla,
    /// Border radius
    pub border_radius: Pixels,
    /// Minimum width
    pub min_width: Pixels,
    /// Minimum height
    pub min_height: Pixels,
    /// Padding
    pub padding: Pixels,
    /// Pin vertical spacing
    pub pin_spacing: Pixels,
    /// Header height
    pub header_height: Pixels,
    /// Text color
    pub text_color: Hsla,
}

impl Default for UnitStyle {
    fn default() -> Self {
        Self {
            playing_bg: hsla(0.0, 0.0, 0.15, 0.95),       // Dark gray
            paused_bg: hsla(0.0, 0.0, 0.1, 0.95),         // Darker gray
            error_bg: hsla(0.0, 0.5, 0.15, 0.95),         // Dark red
            border_color: hsla(0.0, 0.0, 0.3, 1.0),       // Gray border
            border_radius: px(8.0),
            min_width: px(120.0),
            min_height: px(60.0),
            padding: px(12.0),
            pin_spacing: px(20.0),
            header_height: px(24.0),
            text_color: hsla(0.0, 0.0, 0.9, 1.0),         // Light gray
        }
    }
}

/// Events emitted by UnitView
#[derive(Debug, Clone)]
pub enum UnitEvent {
    /// Unit was clicked
    Clicked,
    /// Unit was double-clicked (to open/expand)
    DoubleClicked,
    /// Drag started on this unit
    DragStarted { offset: Point<Pixels> },
    /// Unit was selected
    Selected,
    /// Unit was deselected
    Deselected,
    /// A pin on this unit was clicked
    PinClicked { pin_name: String, direction: PinDirection },
    /// Play button clicked
    PlayClicked,
    /// Pause button clicked
    PauseClicked,
}

/// Snapshot of unit state for rendering
#[derive(Debug, Clone)]
pub struct UnitSnapshot {
    /// Unit ID
    pub id: String,
    /// Lifecycle state
    pub lifecycle: Lifecycle,
    /// Input pin snapshots
    pub inputs: Vec<PinSnapshot>,
    /// Output pin snapshots
    pub outputs: Vec<PinSnapshot>,
    /// Current error message
    pub error: Option<String>,
    /// Position in graph coordinates
    pub position: Point<Pixels>,
}

impl UnitSnapshot {
    /// Create from a Unit reference
    pub fn from_unit(unit: &dyn Unit, position: Point<Pixels>) -> Self {
        let inputs = unit.input_names()
            .iter()
            .filter_map(|name| {
                unit.input(name).map(|pin| {
                    PinSnapshot::from_any_pin(name, pin, PinDirection::Input)
                })
            })
            .collect();

        let outputs = unit.output_names()
            .iter()
            .filter_map(|name| {
                unit.output(name).map(|pin| {
                    PinSnapshot::from_any_pin(name, pin, PinDirection::Output)
                })
            })
            .collect();

        Self {
            id: unit.id().to_string(),
            lifecycle: unit.lifecycle(),
            inputs,
            outputs,
            error: unit.error().map(|s| s.to_string()),
            position,
        }
    }

    /// Calculate the unit's size based on pins
    pub fn calculate_size(&self, style: &UnitStyle) -> gpui::Size<Pixels> {
        let max_pins = self.inputs.len().max(self.outputs.len());
        let pin_spacing_f: f32 = style.pin_spacing.into();
        let header_f: f32 = style.header_height.into();
        let padding_f: f32 = style.padding.into();
        let min_height_f: f32 = style.min_height.into();

        let pin_height = (max_pins as f32) * pin_spacing_f;
        let height = (header_f + pin_height + padding_f * 2.0).max(min_height_f);

        gpui::Size {
            width: style.min_width,
            height: px(height),
        }
    }
}

/// GPUI Entity for rendering a unit
pub struct UnitView {
    /// Unit snapshot for rendering
    snapshot: UnitSnapshot,
    /// Visual style
    style: UnitStyle,
    /// Whether currently selected
    selected: bool,
    /// Whether currently hovered
    hovered: bool,
    /// Input pin view entities
    input_views: Vec<Entity<PinView>>,
    /// Output pin view entities
    output_views: Vec<Entity<PinView>>,
}

impl UnitView {
    /// Create a new UnitView
    pub fn new(snapshot: UnitSnapshot, cx: &mut Context<Self>) -> Self {
        let input_views = snapshot.inputs.iter()
            .map(|pin| cx.new(|_| PinView::new(pin.clone())))
            .collect();

        let output_views = snapshot.outputs.iter()
            .map(|pin| cx.new(|_| PinView::new(pin.clone())))
            .collect();

        Self {
            snapshot,
            style: UnitStyle::default(),
            selected: false,
            hovered: false,
            input_views,
            output_views,
        }
    }

    /// Update the unit snapshot
    pub fn update_snapshot(&mut self, snapshot: UnitSnapshot, cx: &mut Context<Self>) {
        // Update pin views if counts changed
        if snapshot.inputs.len() != self.input_views.len() {
            self.input_views = snapshot.inputs.iter()
                .map(|pin| cx.new(|_| PinView::new(pin.clone())))
                .collect();
        } else {
            // Update existing pins
            for (view, pin) in self.input_views.iter().zip(&snapshot.inputs) {
                view.update(cx, |v, _| v.update_snapshot(pin.clone()));
            }
        }

        if snapshot.outputs.len() != self.output_views.len() {
            self.output_views = snapshot.outputs.iter()
                .map(|pin| cx.new(|_| PinView::new(pin.clone())))
                .collect();
        } else {
            for (view, pin) in self.output_views.iter().zip(&snapshot.outputs) {
                view.update(cx, |v, _| v.update_snapshot(pin.clone()));
            }
        }

        self.snapshot = snapshot;
        cx.notify();
    }

    /// Get the unit ID
    pub fn id(&self) -> &str {
        &self.snapshot.id
    }

    /// Get current position
    pub fn position(&self) -> Point<Pixels> {
        self.snapshot.position
    }

    /// Set position
    pub fn set_position(&mut self, position: Point<Pixels>) {
        self.snapshot.position = position;
    }

    /// Get the size of this unit
    pub fn size(&self) -> gpui::Size<Pixels> {
        self.snapshot.calculate_size(&self.style)
    }

    /// Check if selected
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    /// Set selected state
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// Get the background color based on state
    fn background_color(&self) -> Hsla {
        if self.snapshot.error.is_some() {
            self.style.error_bg
        } else {
            match self.snapshot.lifecycle {
                Lifecycle::Playing => self.style.playing_bg,
                Lifecycle::Paused => self.style.paused_bg,
            }
        }
    }

    /// Get border color based on selection/hover state
    fn border_color(&self) -> Hsla {
        if self.selected {
            hsla(0.58, 0.8, 0.6, 1.0) // Cyan for selected
        } else if self.hovered {
            hsla(0.0, 0.0, 0.5, 1.0) // Lighter gray for hover
        } else {
            self.style.border_color
        }
    }

    /// Get the connection point for a specific pin
    pub fn pin_connection_point(&self, pin_name: &str, direction: PinDirection) -> Option<Point<Pixels>> {
        let size = self.size();
        let pos = self.position();

        let pins = match direction {
            PinDirection::Input => &self.snapshot.inputs,
            PinDirection::Output => &self.snapshot.outputs,
        };

        let index = pins.iter().position(|p| p.name == pin_name)?;
        let header_f: f32 = self.style.header_height.into();
        let padding_f: f32 = self.style.padding.into();
        let spacing_f: f32 = self.style.pin_spacing.into();
        let y_offset = header_f + padding_f + (index as f32 + 0.5) * spacing_f;

        let x = match direction {
            PinDirection::Input => pos.x,
            PinDirection::Output => pos.x + size.width,
        };

        Some(point(x, pos.y + px(y_offset)))
    }
}

impl EventEmitter<UnitEvent> for UnitView {}

impl Render for UnitView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = self.size();
        let bg_color = self.background_color();
        let border_color = self.border_color();

        div()
            .w(size.width)
            .h(size.height)
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .rounded(self.style.border_radius)
            .cursor_pointer()
            .relative()
            .child(
                // Header with unit name
                div()
                    .w_full()
                    .h(self.style.header_height)
                    .px(self.style.padding)
                    .flex()
                    .items_center()
                    .text_color(self.style.text_color)
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(self.snapshot.id.clone())
            )
            .child(
                // Content area with pins
                div()
                    .w_full()
                    .flex_1()
                    .flex()
                    .justify_between()
                    .child(
                        // Left side: input pins
                        div()
                            .flex()
                            .flex_col()
                            .gap(self.style.pin_spacing - px(12.0))
                            .children(self.input_views.iter().cloned())
                    )
                    .child(
                        // Right side: output pins
                        div()
                            .flex()
                            .flex_col()
                            .gap(self.style.pin_spacing - px(12.0))
                            .children(self.output_views.iter().cloned())
                    )
            )
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                let event_x: f32 = event.position.x.into();
                let event_y: f32 = event.position.y.into();
                let pos_x: f32 = this.snapshot.position.x.into();
                let pos_y: f32 = this.snapshot.position.y.into();
                let offset = point(px(event_x - pos_x), px(event_y - pos_y));
                cx.emit(UnitEvent::Clicked);
                cx.emit(UnitEvent::DragStarted { offset });
            }))
    }
}

/// Create a UnitView entity
pub fn unit_view(snapshot: UnitSnapshot, cx: &mut App) -> Entity<UnitView> {
    cx.new(|cx| UnitView::new(snapshot, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_snapshot_size() {
        let snapshot = UnitSnapshot {
            id: "test".to_string(),
            lifecycle: Lifecycle::Paused,
            inputs: vec![
                PinSnapshot {
                    name: "a".to_string(),
                    state: unit::PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "i32".to_string(),
                },
                PinSnapshot {
                    name: "b".to_string(),
                    state: unit::PinState::Empty,
                    is_constant: false,
                    is_ignored: false,
                    direction: PinDirection::Input,
                    type_name: "i32".to_string(),
                },
            ],
            outputs: vec![],
            error: None,
            position: point(px(0.0), px(0.0)),
        };

        let style = UnitStyle::default();
        let size = snapshot.calculate_size(&style);

        let height: f32 = size.height.into();
        let width: f32 = size.width.into();
        let min_height: f32 = style.min_height.into();
        let min_width: f32 = style.min_width.into();
        assert!(height >= min_height);
        assert!(width >= min_width);
    }
}
