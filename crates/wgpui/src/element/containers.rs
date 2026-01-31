use taffy::prelude::{AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent};

use crate::layout::{LayoutId, LayoutStyle, length, px};
use crate::layout_helpers::offset_bounds;
use crate::{Bounds, Size};

use super::{
    AnyElement, Element, ElementPaintContext, IntoElement, LayoutContext, PrepaintContext,
};

pub struct FlexChild {
    element: AnyElement,
    style: Option<LayoutStyle>,
}

impl FlexChild {
    pub fn new<E: IntoElement>(child: E) -> Self {
        Self {
            element: child.into_any_element(),
            style: None,
        }
    }

    pub fn style(mut self, style: LayoutStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn flex_grow(self, value: f32) -> Self {
        self.update_style(|style| style.flex_grow = value)
    }

    pub fn flex_shrink(self, value: f32) -> Self {
        self.update_style(|style| style.flex_shrink = value)
    }

    pub fn width(self, value: Dimension) -> Self {
        self.update_style(|style| style.width = value)
    }

    pub fn height(self, value: Dimension) -> Self {
        self.update_style(|style| style.height = value)
    }

    fn update_style(mut self, apply: impl FnOnce(&mut LayoutStyle)) -> Self {
        let mut style = self.style.unwrap_or_default();
        apply(&mut style);
        self.style = Some(style);
        self
    }
}

pub struct FlexElement {
    style: LayoutStyle,
    children: Vec<FlexChild>,
}

impl FlexElement {
    pub fn new() -> Self {
        Self {
            style: LayoutStyle::new(),
            children: Vec::new(),
        }
    }

    pub fn row() -> Self {
        Self::new().direction(FlexDirection::Row)
    }

    pub fn column() -> Self {
        Self::new().direction(FlexDirection::Column)
    }

    pub fn direction(mut self, value: FlexDirection) -> Self {
        self.style = self.style.flex_direction(value);
        self
    }

    pub fn wrap(mut self, value: FlexWrap) -> Self {
        self.style = self.style.flex_wrap(value);
        self
    }

    pub fn gap(mut self, value: f32) -> Self {
        self.style = self.style.gap(length(value));
        self
    }

    pub fn padding(mut self, value: f32) -> Self {
        self.style = self.style.padding(length(value));
        self
    }

    pub fn align_items(mut self, value: AlignItems) -> Self {
        self.style = self.style.align_items(value);
        self
    }

    pub fn justify_content(mut self, value: JustifyContent) -> Self {
        self.style = self.style.justify_content(value);
        self
    }

    pub fn width(mut self, value: Dimension) -> Self {
        self.style = self.style.width(value);
        self
    }

    pub fn height(mut self, value: Dimension) -> Self {
        self.style = self.style.height(value);
        self
    }

    pub fn flex_grow(mut self, value: f32) -> Self {
        self.style = self.style.flex_grow(value);
        self
    }

    pub fn flex_shrink(mut self, value: f32) -> Self {
        self.style = self.style.flex_shrink(value);
        self
    }

    pub fn child<E: IntoElement>(mut self, child: E) -> Self {
        self.children.push(FlexChild::new(child));
        self
    }

    pub fn child_with_style<E: IntoElement>(mut self, child: E, style: LayoutStyle) -> Self {
        self.children.push(FlexChild::new(child).style(style));
        self
    }

    pub fn push_child<E: IntoElement>(&mut self, child: E) {
        self.children.push(FlexChild::new(child));
    }

    pub fn push_child_with_style<E: IntoElement>(&mut self, child: E, style: LayoutStyle) {
        self.children.push(FlexChild::new(child).style(style));
    }

    pub fn style_mut(&mut self) -> &mut LayoutStyle {
        &mut self.style
    }
}

impl Default for FlexElement {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FlexLayoutState {
    children: Vec<FlexChildLayout>,
}

struct FlexChildLayout {
    wrapper: Option<LayoutId>,
    child: LayoutId,
}

impl Element for FlexElement {
    type RequestLayoutState = FlexLayoutState;
    type PrepaintState = ();

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState) {
        let mut child_nodes = Vec::with_capacity(self.children.len());
        let mut child_layouts = Vec::with_capacity(self.children.len());

        for child in &mut self.children {
            let child_layout = child.element.request_layout(cx);
            let wrapper_layout = child
                .style
                .as_ref()
                .map(|style| cx.request_layout(style, &[child_layout]));
            let layout_id = wrapper_layout.unwrap_or(child_layout);

            child_nodes.push(layout_id);
            child_layouts.push(FlexChildLayout {
                wrapper: wrapper_layout,
                child: child_layout,
            });
        }

        let layout_id = cx.request_layout(&self.style, &child_nodes);
        (
            layout_id,
            FlexLayoutState {
                children: child_layouts,
            },
        )
    }

    fn prepaint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        cx: &mut PrepaintContext,
    ) -> Self::PrepaintState {
        for (child, layout) in self.children.iter_mut().zip(request_layout.children.iter()) {
            let child_bounds = resolve_child_bounds(bounds, |id| cx.layout(id), layout);
            child.element.prepaint(child_bounds, cx);
        }
    }

    fn paint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        cx: &mut ElementPaintContext,
    ) {
        for (child, layout) in self.children.iter_mut().zip(request_layout.children.iter()) {
            let child_bounds = resolve_child_bounds(bounds, |id| cx.layout(id), layout);
            child.element.paint(child_bounds, cx);
        }
    }
}

pub struct StackElement {
    inner: FlexElement,
}

impl StackElement {
    pub fn new() -> Self {
        Self {
            inner: FlexElement::column(),
        }
    }

    pub fn gap(mut self, value: f32) -> Self {
        self.inner = self.inner.gap(value);
        self
    }

    pub fn padding(mut self, value: f32) -> Self {
        self.inner = self.inner.padding(value);
        self
    }

    pub fn align_items(mut self, value: AlignItems) -> Self {
        self.inner = self.inner.align_items(value);
        self
    }

    pub fn justify_content(mut self, value: JustifyContent) -> Self {
        self.inner = self.inner.justify_content(value);
        self
    }

    pub fn child<E: IntoElement>(mut self, child: E) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn child_with_style<E: IntoElement>(mut self, child: E, style: LayoutStyle) -> Self {
        self.inner = self.inner.child_with_style(child, style);
        self
    }

    pub fn push_child<E: IntoElement>(&mut self, child: E) {
        self.inner.push_child(child);
    }

    pub fn push_child_with_style<E: IntoElement>(&mut self, child: E, style: LayoutStyle) {
        self.inner.push_child_with_style(child, style);
    }
}

impl Default for StackElement {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for StackElement {
    type RequestLayoutState = FlexLayoutState;
    type PrepaintState = ();

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(cx)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        cx: &mut PrepaintContext,
    ) -> Self::PrepaintState {
        self.inner.prepaint(bounds, request_layout, cx)
    }

    fn paint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        cx: &mut ElementPaintContext,
    ) {
        self.inner.paint(bounds, request_layout, prepaint, cx)
    }
}

pub struct GridElement {
    inner: FlexElement,
    tile_size: Size,
}

impl GridElement {
    pub fn new(tile_size: Size) -> Self {
        Self {
            inner: FlexElement::row().wrap(FlexWrap::Wrap),
            tile_size,
        }
    }

    pub fn gap(mut self, value: f32) -> Self {
        self.inner = self.inner.gap(value);
        self
    }

    pub fn padding(mut self, value: f32) -> Self {
        self.inner = self.inner.padding(value);
        self
    }

    pub fn align_items(mut self, value: AlignItems) -> Self {
        self.inner = self.inner.align_items(value);
        self
    }

    pub fn justify_content(mut self, value: JustifyContent) -> Self {
        self.inner = self.inner.justify_content(value);
        self
    }

    pub fn tile<E: IntoElement>(mut self, child: E) -> Self {
        let style = self.tile_style();
        self.inner = self.inner.child_with_style(child, style);
        self
    }

    pub fn push_tile<E: IntoElement>(&mut self, child: E) {
        let style = self.tile_style();
        self.inner.push_child_with_style(child, style);
    }

    fn tile_style(&self) -> LayoutStyle {
        LayoutStyle::new()
            .width(px(self.tile_size.width))
            .height(px(self.tile_size.height))
            .flex_shrink(0.0)
    }
}

impl Element for GridElement {
    type RequestLayoutState = FlexLayoutState;
    type PrepaintState = ();

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(cx)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        cx: &mut PrepaintContext,
    ) -> Self::PrepaintState {
        self.inner.prepaint(bounds, request_layout, cx)
    }

    fn paint(
        &mut self,
        bounds: Bounds,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        cx: &mut ElementPaintContext,
    ) {
        self.inner.paint(bounds, request_layout, prepaint, cx)
    }
}

fn resolve_child_bounds(
    bounds: Bounds,
    mut layout: impl FnMut(LayoutId) -> Bounds,
    child: &FlexChildLayout,
) -> Bounds {
    let wrapper_id = child.wrapper.unwrap_or(child.child);
    let wrapper_bounds = layout(wrapper_id);

    if child.wrapper.is_some() {
        let child_bounds = layout(child.child);
        let relative = offset_bounds(child_bounds, wrapper_bounds.origin);
        offset_bounds(relative, bounds.origin)
    } else {
        offset_bounds(wrapper_bounds, bounds.origin)
    }
}
