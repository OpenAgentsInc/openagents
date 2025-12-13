//! Div element - container for other elements

use crate::element::{AnyElement, Element, LayoutContext, PaintContext, ParentElement};
use crate::layout::{Bounds, LayoutId};
use crate::scene::Quad;
use crate::styled::{Style, Styled};
use smallvec::SmallVec;
use std::rc::Rc;

/// Click handler type
pub type ClickHandler = Rc<dyn Fn()>;

/// Div element - a container that can hold child elements
pub struct Div {
    style: Style,
    children: SmallVec<[AnyElement; 4]>,
    on_click: Option<ClickHandler>,
}

impl Div {
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            children: SmallVec::new(),
            on_click: None,
        }
    }

    /// Set a click handler for this div
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a new div element
pub fn div() -> Div {
    Div::new()
}

/// State for div during layout
#[derive(Default)]
pub struct DivState {
    child_layout_ids: Vec<LayoutId>,
}

impl Element for Div {
    type State = DivState;

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::State) {
        // First, request layout for all children
        let mut child_layout_ids = Vec::with_capacity(self.children.len());
        for child in &mut self.children {
            let layout_id = child.request_layout(cx);
            child_layout_ids.push(layout_id);
        }

        // Then request layout for this div with children
        let layout_id = cx
            .layout_engine
            .request_layout_with_children(&self.style, &child_layout_ids);

        (layout_id, DivState { child_layout_ids })
    }

    fn paint(&mut self, bounds: Bounds, state: &mut Self::State, cx: &mut PaintContext) {
        // Paint background if set
        if self.style.background.a > 0.0 || self.style.border_width > 0.0 {
            let mut quad = Quad::new(bounds);

            if self.style.background.a > 0.0 {
                quad = quad.with_background(self.style.background);
            }

            if self.style.border_width > 0.0 {
                quad = quad.with_border(self.style.border_color, self.style.border_width);
            }

            if self.style.border_radius > 0.0 {
                quad = quad.with_corner_radii(self.style.border_radius);
            }

            cx.scene.add_quad(quad);
        }

        // Register clickable region if we have a handler
        if let Some(handler) = &self.on_click {
            cx.scene.add_clickable_region(bounds, handler.clone());
        }

        // Paint children with their computed layout bounds
        for (child, &layout_id) in self.children.iter_mut().zip(state.child_layout_ids.iter()) {
            // Get child bounds from layout engine (relative to parent)
            let child_layout = cx.layout_engine.layout(layout_id);
            // Offset child bounds by parent origin
            let child_bounds = crate::layout::Bounds::new(
                bounds.origin.x + child_layout.origin.x,
                bounds.origin.y + child_layout.origin.y,
                child_layout.size.width,
                child_layout.size.height,
            );
            child.paint(child_bounds, cx);
        }
    }
}

impl Styled for Div {
    fn style(&mut self) -> &mut Style {
        &mut self.style
    }
}

impl ParentElement for Div {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(children);
    }
}
