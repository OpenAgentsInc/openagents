use std::any::Any;

use crate::Bounds;
use crate::layout::LayoutId;

use super::core::{Element, ElementPaintContext, LayoutContext, PrepaintContext};

pub(crate) trait ElementObject {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn request_layout(&mut self, cx: &mut LayoutContext) -> LayoutId;
    fn prepaint(&mut self, bounds: Bounds, cx: &mut PrepaintContext);
    fn paint(&mut self, bounds: Bounds, cx: &mut ElementPaintContext);
}

pub struct Drawable<E: Element> {
    element: E,
    layout_id: Option<LayoutId>,
    request_layout: Option<E::RequestLayoutState>,
    prepaint: Option<E::PrepaintState>,
}

impl<E: Element> Drawable<E> {
    pub fn new(element: E) -> Self {
        Self {
            element,
            layout_id: None,
            request_layout: None,
            prepaint: None,
        }
    }

    pub fn element_mut(&mut self) -> &mut E {
        &mut self.element
    }

    pub fn request_layout(&mut self, cx: &mut LayoutContext) -> LayoutId {
        let (layout_id, state) = self.element.request_layout(cx);
        self.layout_id = Some(layout_id);
        self.request_layout = Some(state);
        self.prepaint = None;
        layout_id
    }

    pub fn prepaint(&mut self, bounds: Bounds, cx: &mut PrepaintContext) {
        let Some(request_layout) = self.request_layout.as_mut() else {
            panic!("prepaint called before request_layout");
        };
        let prepaint = self.element.prepaint(bounds, request_layout, cx);
        self.prepaint = Some(prepaint);
    }

    pub fn paint(&mut self, bounds: Bounds, cx: &mut ElementPaintContext) {
        let Some(request_layout) = self.request_layout.as_mut() else {
            panic!("paint called before request_layout");
        };
        let Some(prepaint) = self.prepaint.as_mut() else {
            panic!("paint called before prepaint");
        };
        self.element.paint(bounds, request_layout, prepaint, cx);
    }
}

impl<E: Element> ElementObject for Drawable<E> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.element
    }

    fn request_layout(&mut self, cx: &mut LayoutContext) -> LayoutId {
        Drawable::request_layout(self, cx)
    }

    fn prepaint(&mut self, bounds: Bounds, cx: &mut PrepaintContext) {
        Drawable::prepaint(self, bounds, cx);
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut ElementPaintContext) {
        Drawable::paint(self, bounds, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutEngine;
    use crate::{Point, Scene, Size, TextSystem};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Tracker {
        calls: Rc<RefCell<Vec<&'static str>>>,
    }

    impl Element for Tracker {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn request_layout(
            &mut self,
            cx: &mut LayoutContext,
        ) -> (LayoutId, Self::RequestLayoutState) {
            self.calls.borrow_mut().push("request_layout");
            let layout_id = cx.request_leaf(&Default::default());
            (layout_id, ())
        }

        fn prepaint(
            &mut self,
            _bounds: Bounds,
            _request_layout: &mut Self::RequestLayoutState,
            _cx: &mut PrepaintContext,
        ) -> Self::PrepaintState {
            self.calls.borrow_mut().push("prepaint");
        }

        fn paint(
            &mut self,
            _bounds: Bounds,
            _request_layout: &mut Self::RequestLayoutState,
            _prepaint: &mut Self::PrepaintState,
            _cx: &mut ElementPaintContext,
        ) {
            self.calls.borrow_mut().push("paint");
        }
    }

    #[test]
    fn test_drawable_lifecycle() {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let tracker = Tracker {
            calls: calls.clone(),
        };

        let mut drawable = Drawable::new(tracker);
        let mut layout = LayoutEngine::new();
        let mut layout_cx = LayoutContext::new(&mut layout);

        let layout_id = drawable.request_layout(&mut layout_cx);
        layout.compute_layout(layout_id, Size::new(100.0, 100.0));

        let bounds = layout.layout(layout_id);
        let mut dispatch = crate::window::DispatchTree::new();
        let mut prepaint_cx = PrepaintContext::new(&layout, &mut dispatch);
        drawable.prepaint(bounds, &mut prepaint_cx);

        let mut scene = Scene::new();
        let mut text = TextSystem::new(1.0);
        let mut paint_cx = ElementPaintContext::with_scroll_offset(
            &layout,
            &mut scene,
            &mut text,
            1.0,
            Point::ZERO,
        );

        drawable.paint(bounds, &mut paint_cx);

        let entries = calls.borrow();
        assert_eq!(entries.as_slice(), ["request_layout", "prepaint", "paint"]);
    }
}
