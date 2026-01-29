use crate::Bounds;
use crate::layout::LayoutId;

use super::drawable::{Drawable, ElementObject};
use super::core::{Element, ElementPaintContext, LayoutContext, PrepaintContext};

pub struct AnyElement {
    inner: Box<dyn ElementObject>,
}

impl AnyElement {
    pub fn new<E>(element: E) -> Self
    where
        E: Element,
        E::RequestLayoutState: 'static,
        E::PrepaintState: 'static,
    {
        Self {
            inner: Box::new(Drawable::new(element)),
        }
    }

    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.inner.as_any_mut().downcast_mut::<T>()
    }

    pub fn request_layout(&mut self, cx: &mut LayoutContext) -> LayoutId {
        self.inner.request_layout(cx)
    }

    pub fn prepaint(&mut self, bounds: Bounds, cx: &mut PrepaintContext) {
        self.inner.prepaint(bounds, cx);
    }

    pub fn paint(&mut self, bounds: Bounds, cx: &mut ElementPaintContext) {
        self.inner.paint(bounds, cx);
    }
}

impl Element for AnyElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.request_layout(cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        bounds: Bounds,
        _request_layout: &mut Self::RequestLayoutState,
        cx: &mut PrepaintContext,
    ) -> Self::PrepaintState {
        self.prepaint(bounds, cx);
    }

    fn paint(
        &mut self,
        bounds: Bounds,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        cx: &mut ElementPaintContext,
    ) {
        self.paint(bounds, cx);
    }
}
