//! Element trait and core types for building UI trees

use crate::layout::{Bounds, LayoutId};
use crate::scene::Scene;
use crate::styled::Style;

/// Context passed during layout phase
pub struct LayoutContext<'a> {
    pub layout_engine: &'a mut crate::layout::LayoutEngine,
    pub text_system: &'a mut crate::text::TextSystem,
}

/// Context passed during paint phase
pub struct PaintContext<'a> {
    pub scene: &'a mut Scene,
    pub text_system: &'a mut crate::text::TextSystem,
}

/// Core trait for UI elements
///
/// Elements have a two-phase lifecycle:
/// 1. `request_layout` - Request space from the layout engine
/// 2. `paint` - Draw to the scene after layout is computed
pub trait Element: 'static {
    /// State computed during layout, passed to paint
    type State: 'static + Default;

    /// Request layout from the layout engine
    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::State);

    /// Paint the element to the scene
    fn paint(&mut self, bounds: Bounds, state: &mut Self::State, cx: &mut PaintContext);
}

/// Trait for converting types into elements
pub trait IntoElement {
    type Element: Element;

    fn into_element(self) -> Self::Element;
}

// Any Element is IntoElement for itself
impl<E: Element> IntoElement for E {
    type Element = E;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Component trait for reusable UI pieces (like GPUI's RenderOnce)
pub trait RenderOnce: 'static + Sized {
    fn render(self) -> impl IntoElement;
}

/// Type-erased element for storing heterogeneous children
pub struct AnyElement {
    element: Box<dyn AnyElementImpl>,
    layout_id: Option<LayoutId>,
    state: Box<dyn std::any::Any>,
}

trait AnyElementImpl {
    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Box<dyn std::any::Any>);
    fn paint(&mut self, bounds: Bounds, state: &mut Box<dyn std::any::Any>, cx: &mut PaintContext);
}

struct ElementWrapper<E: Element> {
    element: E,
}

impl<E: Element> AnyElementImpl for ElementWrapper<E> {
    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Box<dyn std::any::Any>) {
        let (layout_id, state) = self.element.request_layout(cx);
        (layout_id, Box::new(state))
    }

    fn paint(&mut self, bounds: Bounds, state: &mut Box<dyn std::any::Any>, cx: &mut PaintContext) {
        let state = state.downcast_mut::<E::State>().unwrap();
        self.element.paint(bounds, state, cx);
    }
}

impl AnyElement {
    pub fn new<E: Element>(element: E) -> Self {
        Self {
            element: Box::new(ElementWrapper { element }),
            layout_id: None,
            state: Box::new(E::State::default()),
        }
    }

    pub fn request_layout(&mut self, cx: &mut LayoutContext) -> LayoutId {
        let (layout_id, state) = self.element.request_layout(cx);
        self.layout_id = Some(layout_id);
        self.state = state;
        layout_id
    }

    pub fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.element.paint(bounds, &mut self.state, cx);
    }

    pub fn layout_id(&self) -> Option<LayoutId> {
        self.layout_id
    }
}

/// Helper to convert into AnyElement
pub fn into_any<E: IntoElement>(element: E) -> AnyElement {
    AnyElement::new(element.into_element())
}

/// Parent element trait for elements that can contain children
pub trait ParentElement: Sized {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>);

    fn child(mut self, child: impl IntoElement) -> Self {
        self.extend(std::iter::once(AnyElement::new(child.into_element())));
        self
    }

    fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.extend(children.into_iter().map(|c| AnyElement::new(c.into_element())));
        self
    }
}

/// Empty element that renders nothing
pub struct Empty;

impl Element for Empty {
    type State = ();

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::State) {
        let style = Style::default();
        let layout_id = cx.layout_engine.request_layout(&style);
        (layout_id, ())
    }

    fn paint(&mut self, _bounds: Bounds, _state: &mut Self::State, _cx: &mut PaintContext) {
        // Nothing to paint
    }
}
