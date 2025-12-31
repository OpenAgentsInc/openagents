//! Div component - a container for other components.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{AnyComponent, Component, ComponentId, EventResult};
use crate::styled::{StyleRefinement, Styled};
use crate::{Bounds, Hsla, InputEvent, Quad};
use smallvec::SmallVec;

/// A container component that can hold children.
pub struct Div {
    id: Option<ComponentId>,
    pub(crate) style: StyleRefinement,
    children: SmallVec<[AnyComponent; 4]>,
}

impl Div {
    pub fn new() -> Self {
        Self {
            id: None,
            style: StyleRefinement::default(),
            children: SmallVec::new(),
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn background(mut self, color: Hsla) -> Self {
        self.style.background = Some(color);
        self
    }

    pub fn border(mut self, color: Hsla, width: f32) -> Self {
        self.style.border_color = Some(color);
        self.style.border_width = Some(width);
        self
    }

    pub fn border_color(mut self, color: Hsla) -> Self {
        self.style.border_color = Some(color);
        if self.style.border_width.is_none() {
            self.style.border_width = Some(1.0);
        }
        self
    }

    pub fn border_width(mut self, width: f32) -> Self {
        self.style.border_width = Some(width);
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.style.corner_radius = Some(radius);
        self
    }

    pub fn child<C: Component + 'static>(mut self, component: C) -> Self {
        self.children.push(AnyComponent::new(component));
        self
    }

    pub fn children<I, C>(mut self, components: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Component + 'static,
    {
        for component in components {
            self.children.push(AnyComponent::new(component));
        }
        self
    }

    pub fn child_any(mut self, component: AnyComponent) -> Self {
        self.children.push(component);
        self
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Div {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let border_color = self.style.border_color;
        let border_width = self
            .style
            .border_width
            .unwrap_or_else(|| if border_color.is_some() { 1.0 } else { 0.0 });
        let corner_radius = self.style.corner_radius.unwrap_or(0.0);

        if self.style.background.is_some() || border_color.is_some() || corner_radius > 0.0 {
            let mut quad = Quad::new(bounds);

            if let Some(bg) = self.style.background {
                quad = quad.with_background(bg);
            }

            if let Some(border) = border_color {
                quad = quad.with_border(border, border_width);
            }

            if corner_radius > 0.0 {
                quad = quad.with_corner_radius(corner_radius);
            }

            cx.scene.draw_quad(quad);
        }

        for child in &mut self.children {
            child.paint(bounds, cx);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        for child in self.children.iter_mut().rev() {
            let result = child.event(event, bounds, cx);
            if result.is_handled() {
                return result;
            }
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }
}

impl Styled for Div {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;

    #[test]
    fn test_div_new() {
        let div = Div::new();
        assert!(div.id.is_none());
        assert!(div.style.background.is_none());
        assert!(div.style.border_color.is_none());
        assert!(div.style.border_width.is_none());
        assert!(div.children.is_empty());
    }

    #[test]
    fn test_div_builder_id() {
        let div = Div::new().with_id(42);
        assert_eq!(div.id, Some(42));
    }

    #[test]
    fn test_div_builder_background() {
        let div = Div::new().background(theme::bg::SURFACE);
        assert!(div.style.background.is_some());
    }

    #[test]
    fn test_div_builder_border() {
        let div = Div::new().border(theme::border::DEFAULT, 2.0);
        assert!(div.style.border_color.is_some());
        assert_eq!(div.style.border_width, Some(2.0));
    }

    #[test]
    fn test_div_border_color_sets_default_width() {
        let div = Div::new().border_color(theme::border::DEFAULT);
        assert!(div.style.border_color.is_some());
        assert_eq!(div.style.border_width, Some(1.0));
    }

    #[test]
    fn test_div_child_count() {
        let inner = Div::new().background(theme::bg::MUTED);
        let div = Div::new().child(inner);

        assert_eq!(div.child_count(), 1);
        assert!(div.has_children());
    }

    #[test]
    fn test_div_no_children() {
        let div = Div::new();
        assert_eq!(div.child_count(), 0);
        assert!(!div.has_children());
    }

    #[test]
    fn test_div_default() {
        let div = Div::default();
        assert!(div.id.is_none());
        assert!(!div.has_children());
    }

    #[test]
    fn test_div_component_id() {
        let div = Div::new().with_id(123);
        assert_eq!(Component::id(&div), Some(123));

        let div_no_id = Div::new();
        assert_eq!(Component::id(&div_no_id), None);
    }
}
