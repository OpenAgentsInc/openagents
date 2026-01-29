use crate::Size;
use crate::element::{Element, ElementPaintContext, LayoutContext, PrepaintContext};
use crate::layout::LayoutEngine;
use crate::scene::Scene;
use crate::text::TextSystem;

use super::dispatch::DispatchTree;
use super::invalidator::{InvalidationFlags, Invalidator};
use super::window_handle::WindowHandle;

pub struct Window {
    layout: LayoutEngine,
    scene: Scene,
    text: TextSystem,
    dispatch: DispatchTree,
    invalidator: Invalidator,
    size: Size,
    scale_factor: f32,
    focused: Option<u64>,
}

impl Window {
    pub fn new(size: Size, scale_factor: f32) -> Self {
        Self {
            layout: LayoutEngine::new(),
            scene: Scene::new(),
            text: TextSystem::new(scale_factor),
            dispatch: DispatchTree::new(),
            invalidator: Invalidator::new(),
            size,
            scale_factor,
            focused: None,
        }
    }

    pub fn handle(&self) -> WindowHandle {
        self.invalidator.handle()
    }

    pub fn invalidator(&self) -> &Invalidator {
        &self.invalidator
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    pub fn resize(&mut self, size: Size) {
        self.size = size;
        self.invalidator.request_layout();
    }

    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.text.set_scale_factor(scale_factor);
        self.invalidator.request_layout();
    }

    pub fn focus(&self) -> Option<u64> {
        self.focused
    }

    pub fn set_focus(&mut self, id: u64) {
        self.focused = Some(id);
    }

    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub fn text(&self) -> &TextSystem {
        &self.text
    }

    pub fn text_mut(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    pub fn layout(&self) -> &LayoutEngine {
        &self.layout
    }

    pub fn layout_mut(&mut self) -> &mut LayoutEngine {
        &mut self.layout
    }

    pub fn dispatch(&self) -> &DispatchTree {
        &self.dispatch
    }

    pub fn dispatch_mut(&mut self) -> &mut DispatchTree {
        &mut self.dispatch
    }

    pub fn begin_frame(&mut self) -> InvalidationFlags {
        let flags = self.invalidator.take();
        self.scene.clear();
        self.dispatch.clear();
        self.layout.clear();
        flags
    }

    pub fn render_root<E: Element>(&mut self, element: &mut E) {
        let _ = self.begin_frame();

        let mut layout_cx = LayoutContext::new(&mut self.layout);
        let (layout_id, mut request_layout) = element.request_layout(&mut layout_cx);

        self.layout.compute_layout(layout_id, self.size);
        let bounds = self.layout.layout(layout_id);

        let mut prepaint_cx = PrepaintContext::new(&self.layout, &mut self.dispatch);
        let mut prepaint = element.prepaint(bounds, &mut request_layout, &mut prepaint_cx);

        if let Some(id) = element.id() {
            self.dispatch.register(id, bounds, 0);
        }

        let mut paint_cx = ElementPaintContext::new(
            &self.layout,
            &mut self.scene,
            &mut self.text,
            self.scale_factor,
        );
        element.paint(bounds, &mut request_layout, &mut prepaint, &mut paint_cx);
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new(Size::ZERO, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::Element;
    use crate::layout::{LayoutStyle, px};
    use crate::{Bounds, Point};

    struct TestElement {
        id: u64,
        last_bounds: Option<Bounds>,
    }

    impl TestElement {
        fn new(id: u64) -> Self {
            Self {
                id,
                last_bounds: None,
            }
        }
    }

    impl Element for TestElement {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn request_layout(
            &mut self,
            cx: &mut LayoutContext,
        ) -> (crate::layout::LayoutId, Self::RequestLayoutState) {
            let style = LayoutStyle::new().width(px(80.0)).height(px(40.0));
            (cx.request_leaf(&style), ())
        }

        fn prepaint(
            &mut self,
            bounds: Bounds,
            _request_layout: &mut Self::RequestLayoutState,
            cx: &mut PrepaintContext,
        ) -> Self::PrepaintState {
            self.last_bounds = Some(bounds);
            cx.register(99, Bounds::new(10.0, 10.0, 10.0, 10.0), 2);
        }

        fn paint(
            &mut self,
            _bounds: Bounds,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
            _cx: &mut ElementPaintContext,
        ) {
        }

        fn id(&self) -> Option<u64> {
            Some(self.id)
        }
    }

    #[test]
    fn test_window_render_root() {
        let mut window = Window::new(Size::new(200.0, 200.0), 1.0);
        let mut element = TestElement::new(7);

        window.render_root(&mut element);

        let bounds = element.last_bounds.expect("bounds not captured");
        assert!((bounds.size.width - 80.0).abs() < 0.01);
        assert!((bounds.size.height - 40.0).abs() < 0.01);

        let hit = window.dispatch().hit_test(Point::new(12.0, 12.0));
        assert!(!hit.entries.is_empty());
        assert_eq!(hit.entries[0].node_id, crate::hit_test::NodeId(99));
    }

    #[test]
    fn test_window_focus() {
        let mut window = Window::default();
        assert!(window.focus().is_none());
        window.set_focus(42);
        assert_eq!(window.focus(), Some(42));
        window.clear_focus();
        assert!(window.focus().is_none());
    }
}
