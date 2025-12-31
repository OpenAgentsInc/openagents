use crate::components::{Component, PaintContext as ComponentPaintContext};
use crate::layout::{LayoutEngine, LayoutId, LayoutStyle, px};
use crate::window::DispatchTree;
use crate::{Bounds, Point, Scene, Size, TextSystem};

use super::AnyElement;

pub type ElementId = u64;

pub struct LayoutContext<'a> {
    layout: &'a mut LayoutEngine,
}

impl<'a> LayoutContext<'a> {
    pub fn new(layout: &'a mut LayoutEngine) -> Self {
        Self { layout }
    }

    pub fn request_layout(&mut self, style: &LayoutStyle, children: &[LayoutId]) -> LayoutId {
        self.layout.request_layout(style, children)
    }

    pub fn request_leaf(&mut self, style: &LayoutStyle) -> LayoutId {
        self.layout.request_leaf(style)
    }

    pub fn request_measured<F>(&mut self, style: &LayoutStyle, measure: F) -> LayoutId
    where
        F: Fn(
                taffy::Size<Option<f32>>,
                taffy::Size<taffy::AvailableSpace>,
                taffy::NodeId,
                Option<&mut ()>,
                &taffy::Style,
            ) -> taffy::Size<f32>
            + Send
            + Sync
            + 'static,
    {
        self.layout.request_measured(style, measure)
    }
}

pub struct PrepaintContext<'a> {
    layout: &'a LayoutEngine,
    dispatch: &'a mut DispatchTree,
}

impl<'a> PrepaintContext<'a> {
    pub fn new(layout: &'a LayoutEngine, dispatch: &'a mut DispatchTree) -> Self {
        Self { layout, dispatch }
    }

    pub fn layout(&self, id: LayoutId) -> Bounds {
        self.layout.layout(id)
    }

    pub fn size(&self, id: LayoutId) -> Size {
        self.layout.size(id)
    }

    pub fn register(&mut self, id: ElementId, bounds: Bounds, depth: u32) {
        self.dispatch.register(id, bounds, depth);
    }
}

pub struct ElementPaintContext<'a> {
    layout: &'a LayoutEngine,
    pub scene: &'a mut Scene,
    pub text: &'a mut TextSystem,
    pub scale_factor: f32,
    pub scroll_offset: Point,
}

impl<'a> ElementPaintContext<'a> {
    pub fn new(
        layout: &'a LayoutEngine,
        scene: &'a mut Scene,
        text: &'a mut TextSystem,
        scale_factor: f32,
    ) -> Self {
        Self {
            layout,
            scene,
            text,
            scale_factor,
            scroll_offset: Point::ZERO,
        }
    }

    pub fn with_scroll_offset(
        layout: &'a LayoutEngine,
        scene: &'a mut Scene,
        text: &'a mut TextSystem,
        scale_factor: f32,
        scroll_offset: Point,
    ) -> Self {
        Self {
            layout,
            scene,
            text,
            scale_factor,
            scroll_offset,
        }
    }

    pub fn layout(&self, id: LayoutId) -> Bounds {
        self.layout.layout(id)
    }

    pub fn size(&self, id: LayoutId) -> Size {
        self.layout.size(id)
    }

    pub fn component_context(&mut self) -> ComponentPaintContext<'_> {
        ComponentPaintContext {
            scene: &mut *self.scene,
            text: &mut *self.text,
            scale_factor: self.scale_factor,
            scroll_offset: self.scroll_offset,
        }
    }
}

pub trait Element: 'static {
    type RequestLayoutState: 'static;
    type PrepaintState: 'static;

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState);

    fn prepaint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        cx: &mut PrepaintContext,
    ) -> Self::PrepaintState;

    fn paint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        cx: &mut ElementPaintContext,
    );

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn into_any(self) -> AnyElement
    where
        Self: Sized,
    {
        AnyElement::new(self)
    }
}

pub struct ComponentElement<C: Component> {
    component: C,
}

impl<C: Component> ComponentElement<C> {
    pub fn new(component: C) -> Self {
        Self { component }
    }

    pub fn into_inner(self) -> C {
        self.component
    }
}

impl<C: Component + 'static> Element for ComponentElement<C> {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState) {
        let (width, height) = self.component.size_hint();
        let mut style = LayoutStyle::new();
        if let Some(width) = width {
            style = style.width(px(width));
        }
        if let Some(height) = height {
            style = style.height(px(height));
        }

        let layout_id = cx.request_leaf(&style);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds,
        _request_layout: &mut Self::RequestLayoutState,
        _cx: &mut PrepaintContext,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        bounds: Bounds,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        cx: &mut ElementPaintContext,
    ) {
        let mut paint_cx = cx.component_context();
        self.component.paint(bounds, &mut paint_cx);
    }

    fn id(&self) -> Option<ElementId> {
        self.component.id()
    }
}

impl<C: Component> From<C> for ComponentElement<C> {
    fn from(component: C) -> Self {
        Self::new(component)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutEngine;

    struct FixedComponent;

    impl Component for FixedComponent {
        fn paint(&mut self, _bounds: Bounds, _cx: &mut ComponentPaintContext) {}

        fn size_hint(&self) -> (Option<f32>, Option<f32>) {
            (Some(120.0), Some(24.0))
        }
    }

    #[test]
    fn test_component_element_size_hint() {
        let mut layout = LayoutEngine::new();
        let mut cx = LayoutContext::new(&mut layout);
        let mut element = ComponentElement::new(FixedComponent);

        let (layout_id, _state) = element.request_layout(&mut cx);
        layout.compute_layout(layout_id, Size::new(800.0, 600.0));

        let bounds = layout.layout(layout_id);
        assert!((bounds.size.width - 120.0).abs() < 0.01);
        assert!((bounds.size.height - 24.0).abs() < 0.01);
    }
}
