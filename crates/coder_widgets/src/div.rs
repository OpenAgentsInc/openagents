//! Div widget - a container for other widgets.
//!
//! The Div widget provides a container that can
//! hold children and style them with backgrounds, borders, etc.

use crate::context::{EventContext, PaintContext};
use crate::widget::{AnyWidget, EventResult, Widget, WidgetId};
use smallvec::SmallVec;
use wgpui::{Bounds, Hsla, InputEvent, Quad};

/// A container widget that can hold children.
pub struct Div {
    /// Unique ID for this widget.
    id: Option<WidgetId>,
    /// Background color.
    background: Option<Hsla>,
    /// Border color.
    border_color: Option<Hsla>,
    /// Border width.
    border_width: f32,
    /// Corner radius.
    corner_radius: f32,
    /// Child widgets.
    children: SmallVec<[AnyWidget; 4]>,
}

impl Div {
    /// Create a new empty Div.
    pub fn new() -> Self {
        Self {
            id: None,
            background: None,
            border_color: None,
            border_width: 0.0,
            corner_radius: 0.0,
            children: SmallVec::new(),
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the background color.
    pub fn background(mut self, color: Hsla) -> Self {
        self.background = Some(color);
        self
    }

    /// Set the border.
    pub fn border(mut self, color: Hsla, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }

    /// Set the corner radius.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Add a child widget.
    pub fn child<W: Widget + 'static>(mut self, widget: W) -> Self {
        self.children.push(AnyWidget::new(widget));
        self
    }

    /// Add multiple children.
    pub fn children<I, W>(mut self, widgets: I) -> Self
    where
        I: IntoIterator<Item = W>,
        W: Widget + 'static,
    {
        for widget in widgets {
            self.children.push(AnyWidget::new(widget));
        }
        self
    }
}

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Div {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw background/border if set
        if self.background.is_some() || self.border_color.is_some() {
            let mut quad = Quad::new(bounds);

            if let Some(bg) = self.background {
                quad = quad.with_background(bg);
            }

            if let Some(border) = self.border_color {
                quad = quad.with_border(border, self.border_width);
            }

            if self.corner_radius > 0.0 {
                quad = quad.with_uniform_radius(self.corner_radius);
            }

            cx.scene.draw_quad(quad);
        }

        // Paint children (they would need their own bounds in a full implementation)
        for child in &mut self.children {
            child.paint(bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Propagate to children (reverse order for z-order)
        for child in self.children.iter_mut().rev() {
            let result = child.event(event, bounds, cx);
            if result.is_handled() {
                return result;
            }
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_div_builder() {
        let div = Div::new()
            .id(1)
            .background(wgpui::theme::bg::SURFACE)
            .corner_radius(8.0);

        assert_eq!(div.id, Some(1));
        assert!(div.background.is_some());
        assert_eq!(div.corner_radius, 8.0);
    }
}
