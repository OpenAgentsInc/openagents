//! Connection rendering: Bezier curves between pins
//!
//! Renders connections as cubic bezier curves with configurable styling.
//! Supports different states (active, inactive, selected) with visual feedback.

use gpui::{
    Background, Hsla, Path, PathBuilder, Pixels, Point, Window,
    hsla, point, px,
};
use unit::geometry::Point as UnitPoint;

/// Connection visual style
#[derive(Debug, Clone)]
pub struct ConnectionStyle {
    /// Stroke width
    pub stroke_width: Pixels,
    /// Color for active connections
    pub active_color: Hsla,
    /// Color for inactive connections
    pub inactive_color: Hsla,
    /// Color for selected connections
    pub selected_color: Hsla,
    /// Control point distance factor (0-1, how far control points are from endpoints)
    pub curvature: f32,
}

impl Default for ConnectionStyle {
    fn default() -> Self {
        Self {
            stroke_width: px(2.0),
            active_color: hsla(0.0, 0.0, 1.0, 0.8),        // White
            inactive_color: hsla(0.0, 0.0, 0.5, 0.5),      // Gray
            selected_color: hsla(0.58, 0.8, 0.6, 1.0),     // Cyan
            curvature: 0.4,
        }
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Inactive,
    Active,
    Selected,
}

/// A renderable connection between two points
#[derive(Debug, Clone)]
pub struct Connection {
    /// Starting point (output pin)
    pub from: Point<Pixels>,
    /// Ending point (input pin)
    pub to: Point<Pixels>,
    /// Connection state
    pub state: ConnectionState,
}

impl Connection {
    /// Create a new connection between two points
    pub fn new(from: Point<Pixels>, to: Point<Pixels>) -> Self {
        Self {
            from,
            to,
            state: ConnectionState::default(),
        }
    }

    /// Create from unit geometry points
    pub fn from_unit_points(from: &UnitPoint, to: &UnitPoint) -> Self {
        Self::new(
            point(px(from.x as f32), px(from.y as f32)),
            point(px(to.x as f32), px(to.y as f32)),
        )
    }

    /// Set connection state
    pub fn with_state(mut self, state: ConnectionState) -> Self {
        self.state = state;
        self
    }

    /// Build the bezier path for this connection
    pub fn build_path(&self, style: &ConnectionStyle) -> Option<Path<Pixels>> {
        let dx = (self.to.x - self.from.x).0.abs();
        let control_offset = px(dx * style.curvature);

        // Control points for horizontal-biased bezier
        let cp1 = point(self.from.x + control_offset, self.from.y);
        let cp2 = point(self.to.x - control_offset, self.to.y);

        let mut builder = PathBuilder::stroke(style.stroke_width);
        builder.move_to(self.from);
        builder.curve_to(cp1, cp2, self.to);

        builder.build().ok()
    }

    /// Get the color for this connection based on state
    pub fn color(&self, style: &ConnectionStyle) -> Hsla {
        match self.state {
            ConnectionState::Inactive => style.inactive_color,
            ConnectionState::Active => style.active_color,
            ConnectionState::Selected => style.selected_color,
        }
    }

    /// Paint this connection to the window
    pub fn paint(&self, style: &ConnectionStyle, window: &mut Window) {
        if let Some(path) = self.build_path(style) {
            let color = self.color(style);
            window.paint_path(path, Background::from(color));
        }
    }
}

/// Collection of connections to render together
pub struct ConnectionLayer {
    /// All connections
    pub connections: Vec<Connection>,
    /// Shared style
    pub style: ConnectionStyle,
}

impl ConnectionLayer {
    /// Create a new empty connection layer
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            style: ConnectionStyle::default(),
        }
    }

    /// Add a connection
    pub fn add(&mut self, connection: Connection) {
        self.connections.push(connection);
    }

    /// Clear all connections
    pub fn clear(&mut self) {
        self.connections.clear();
    }

    /// Paint all connections
    pub fn paint(&self, window: &mut Window) {
        for conn in &self.connections {
            conn.paint(&self.style, window);
        }
    }
}

impl Default for ConnectionLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a temporary "dragging" connection from a pin to cursor
pub fn drag_connection(
    from: Point<Pixels>,
    to: Point<Pixels>,
    style: &ConnectionStyle,
) -> Option<Path<Pixels>> {
    Connection::new(from, to)
        .with_state(ConnectionState::Active)
        .build_path(style)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_creation() {
        let conn = Connection::new(
            point(px(0.0), px(0.0)),
            point(px(100.0), px(50.0)),
        );
        assert_eq!(conn.state, ConnectionState::Inactive);
    }

    #[test]
    fn test_connection_from_unit_points() {
        let from = UnitPoint::new(10.0, 20.0);
        let to = UnitPoint::new(100.0, 80.0);
        let conn = Connection::from_unit_points(&from, &to);

        assert_eq!(conn.from.x.0, 10.0);
        assert_eq!(conn.from.y.0, 20.0);
        assert_eq!(conn.to.x.0, 100.0);
        assert_eq!(conn.to.y.0, 80.0);
    }

    #[test]
    fn test_connection_layer() {
        let mut layer = ConnectionLayer::new();
        layer.add(Connection::new(
            point(px(0.0), px(0.0)),
            point(px(100.0), px(100.0)),
        ));
        assert_eq!(layer.connections.len(), 1);

        layer.clear();
        assert_eq!(layer.connections.len(), 0);
    }
}
