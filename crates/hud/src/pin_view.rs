//! PinView: GPUI visualization of a Pin
//!
//! Renders a pin as a small circle indicator with state-based coloring.
//! Can be used standalone or as part of a UnitView.

use gpui::{
    App, Context, Entity, EventEmitter, Hsla, Pixels, Point, Render, Window,
    div, hsla, point, prelude::*, px,
};
use unit::{AnyPin, PinState, IO};

/// Pin visual style configuration
#[derive(Debug, Clone)]
pub struct PinStyle {
    /// Pin radius
    pub radius: Pixels,
    /// Color when pin is empty
    pub empty_color: Hsla,
    /// Color when pin has valid data
    pub valid_color: Hsla,
    /// Color when pin is invalid
    pub invalid_color: Hsla,
    /// Color when pin is constant
    pub constant_color: Hsla,
    /// Border color
    pub border_color: Hsla,
    /// Border width
    pub border_width: Pixels,
}

impl Default for PinStyle {
    fn default() -> Self {
        Self {
            radius: px(6.0),
            empty_color: hsla(0.0, 0.0, 0.3, 1.0),        // Dark gray
            valid_color: hsla(0.33, 0.8, 0.5, 1.0),       // Green
            invalid_color: hsla(0.0, 0.8, 0.5, 1.0),      // Red
            constant_color: hsla(0.58, 0.8, 0.6, 1.0),    // Cyan
            border_color: hsla(0.0, 0.0, 0.8, 1.0),       // Light gray
            border_width: px(1.0),
        }
    }
}

/// Events emitted by PinView
#[derive(Debug, Clone)]
pub enum PinEvent {
    /// Pin was clicked
    Clicked,
    /// Drag started from this pin
    DragStarted,
    /// Something was dropped on this pin
    DropReceived,
    /// Mouse entered pin area
    Hovered,
    /// Mouse left pin area
    Unhovered,
}

/// Pin direction for positioning
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinDirection {
    /// Input pin (typically on left side)
    Input,
    /// Output pin (typically on right side)
    Output,
}

impl From<IO> for PinDirection {
    fn from(io: IO) -> Self {
        match io {
            IO::Input => PinDirection::Input,
            IO::Output => PinDirection::Output,
        }
    }
}

/// Snapshot of pin state for rendering
#[derive(Debug, Clone)]
pub struct PinSnapshot {
    /// Pin name
    pub name: String,
    /// Pin state
    pub state: PinState,
    /// Whether pin is constant
    pub is_constant: bool,
    /// Whether pin is ignored
    pub is_ignored: bool,
    /// Pin direction
    pub direction: PinDirection,
    /// Type name for tooltip
    pub type_name: String,
}

impl PinSnapshot {
    /// Create from an AnyPin reference
    pub fn from_any_pin(name: &str, pin: &dyn AnyPin, direction: PinDirection) -> Self {
        let state = if pin.is_active() {
            PinState::Valid
        } else if pin.is_idle() {
            PinState::Empty
        } else {
            PinState::Invalid
        };

        Self {
            name: name.to_string(),
            state,
            is_constant: pin.is_constant(),
            is_ignored: pin.is_ignored(),
            direction,
            type_name: pin.type_name().to_string(),
        }
    }
}

/// GPUI Entity for rendering a pin
pub struct PinView {
    /// Pin snapshot for rendering
    snapshot: PinSnapshot,
    /// Visual style
    style: PinStyle,
    /// Whether currently hovered (reserved for future use)
    _hovered: bool,
}

impl PinView {
    /// Create a new PinView
    pub fn new(snapshot: PinSnapshot) -> Self {
        Self {
            snapshot,
            style: PinStyle::default(),
            _hovered: false,
        }
    }

    /// Update the pin snapshot
    pub fn update_snapshot(&mut self, snapshot: PinSnapshot) {
        self.snapshot = snapshot;
    }

    /// Get the pin name
    pub fn name(&self) -> &str {
        &self.snapshot.name
    }

    /// Get the color for the current state
    fn state_color(&self) -> Hsla {
        if self.snapshot.is_constant {
            self.style.constant_color
        } else {
            match self.snapshot.state {
                PinState::Empty => self.style.empty_color,
                PinState::Valid => self.style.valid_color,
                PinState::Invalid => self.style.invalid_color,
            }
        }
    }

    /// Get connection point in local coordinates
    pub fn connection_point(&self, center: Point<Pixels>) -> Point<Pixels> {
        match self.snapshot.direction {
            PinDirection::Input => point(center.x - self.style.radius, center.y),
            PinDirection::Output => point(center.x + self.style.radius, center.y),
        }
    }
}

impl EventEmitter<PinEvent> for PinView {}

impl Render for PinView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let color = self.state_color();
        let radius = self.style.radius;
        let border_color = self.style.border_color;
        let diameter = radius * 2.0;

        div()
            .size(diameter)
            .rounded_full()
            .bg(color)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|_this, _, _, cx| {
                cx.emit(PinEvent::Clicked);
                cx.emit(PinEvent::DragStarted);
            }))
    }
}

/// Create a PinView entity
pub fn pin_view(snapshot: PinSnapshot, cx: &mut App) -> Entity<PinView> {
    cx.new(|_| PinView::new(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_snapshot() {
        let snapshot = PinSnapshot {
            name: "input".to_string(),
            state: PinState::Valid,
            is_constant: false,
            is_ignored: false,
            direction: PinDirection::Input,
            type_name: "i32".to_string(),
        };

        assert_eq!(snapshot.name, "input");
        assert_eq!(snapshot.state, PinState::Valid);
        assert_eq!(snapshot.direction, PinDirection::Input);
    }

    #[test]
    fn test_pin_direction_from_io() {
        assert_eq!(PinDirection::from(IO::Input), PinDirection::Input);
        assert_eq!(PinDirection::from(IO::Output), PinDirection::Output);
    }
}
