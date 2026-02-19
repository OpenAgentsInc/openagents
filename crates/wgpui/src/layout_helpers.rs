use crate::geometry::{Bounds, Point, Size};
use crate::layout::{LayoutEngine, LayoutStyle, length, px};
use taffy::prelude::FlexWrap;

#[derive(Clone, Copy, Debug)]
pub enum RowItem {
    Fixed(f32),
    Flex(f32),
}

impl RowItem {
    pub fn fixed(width: f32) -> Self {
        Self::Fixed(width)
    }

    pub fn flex(grow: f32) -> Self {
        Self::Flex(grow)
    }
}

pub struct HeaderNavContentLayout {
    pub header: Bounds,
    pub nav: Bounds,
    pub content: Bounds,
}

pub struct PanelLayout {
    pub header: Bounds,
    pub body: Bounds,
}

pub fn offset_bounds(bounds: Bounds, origin: Point) -> Bounds {
    Bounds::new(
        bounds.origin.x + origin.x,
        bounds.origin.y + origin.y,
        bounds.size.width,
        bounds.size.height,
    )
}

pub fn stack_bounds(bounds: Bounds, heights: &[f32], gap: f32) -> Vec<Bounds> {
    if heights.is_empty() {
        return Vec::new();
    }

    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let mut nodes = Vec::with_capacity(heights.len());

    for height in heights {
        let style = LayoutStyle::new().height(px(*height)).flex_shrink(0.0);
        nodes.push(engine.request_leaf(&style));
    }

    let stack_style = LayoutStyle::new()
        .flex_col()
        .gap(gap)
        .width(px(bounds.size.width))
        .height(px(bounds.size.height));
    let stack = engine.request_layout(&stack_style, &nodes);

    engine.compute_layout(stack, Size::new(bounds.size.width, bounds.size.height));

    nodes
        .into_iter()
        .map(|node| offset_bounds(engine.layout(node), bounds.origin))
        .collect()
}

pub fn row_bounds(bounds: Bounds, height: f32, items: &[RowItem], gap: f32) -> Vec<Bounds> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let mut nodes = Vec::with_capacity(items.len());

    for item in items {
        let style = match item {
            RowItem::Fixed(width) => LayoutStyle::new()
                .width(px(*width))
                .height(px(height))
                .flex_shrink(0.0),
            RowItem::Flex(grow) => LayoutStyle::new().height(px(height)).flex_grow(*grow),
        };
        nodes.push(engine.request_leaf(&style));
    }

    let row_style = LayoutStyle::new()
        .flex_row()
        .gap(gap)
        .width(px(bounds.size.width))
        .height(px(bounds.size.height));
    let row = engine.request_layout(&row_style, &nodes);

    engine.compute_layout(row, Size::new(bounds.size.width, bounds.size.height));

    nodes
        .into_iter()
        .map(|node| offset_bounds(engine.layout(node), bounds.origin))
        .collect()
}

pub fn grid_bounds(bounds: Bounds, item_size: Size, count: usize, gap: f32) -> Vec<Bounds> {
    if count == 0 {
        return Vec::new();
    }

    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let mut nodes = Vec::with_capacity(count);

    for _ in 0..count {
        let style = LayoutStyle::new()
            .width(px(item_size.width))
            .height(px(item_size.height))
            .flex_shrink(0.0);
        nodes.push(engine.request_leaf(&style));
    }

    let grid_style = LayoutStyle::new()
        .flex_row()
        .flex_wrap(FlexWrap::Wrap)
        .align_content(taffy::prelude::AlignContent::FlexStart)
        .gap(gap)
        .width(px(bounds.size.width))
        .height(px(bounds.size.height));
    let grid = engine.request_layout(&grid_style, &nodes);

    engine.compute_layout(grid, Size::new(bounds.size.width, bounds.size.height));

    nodes
        .into_iter()
        .map(|node| offset_bounds(engine.layout(node), bounds.origin))
        .collect()
}

pub fn layout_header_nav_content(
    bounds: Bounds,
    header_height: f32,
    nav_width: f32,
    gap: f32,
    margin: f32,
) -> HeaderNavContentLayout {
    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let padding = length(margin);

    let header = engine.request_leaf(&LayoutStyle::new().height(px(header_height)));
    let nav = engine.request_layout(
        &LayoutStyle::new().width(px(nav_width)).flex_shrink(0.0),
        &[],
    );
    let content = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);

    let row = engine.request_layout(&LayoutStyle::new().flex_row().gap(gap), &[nav, content]);

    let root = engine.request_layout(
        &LayoutStyle::new()
            .flex_col()
            .gap(gap)
            .padding(padding)
            .width(px(bounds.size.width))
            .height(px(bounds.size.height)),
        &[header, row],
    );

    engine.compute_layout(root, Size::new(bounds.size.width, bounds.size.height));

    HeaderNavContentLayout {
        header: offset_bounds(engine.layout(header), bounds.origin),
        nav: offset_bounds(engine.layout(nav), bounds.origin),
        content: offset_bounds(engine.layout(content), bounds.origin),
    }
}

pub fn layout_panel(bounds: Bounds, header_height: f32, gap: f32, padding: f32) -> PanelLayout {
    let mut engine = LayoutEngine::new();
    let gap = length(gap);
    let padding = length(padding);

    let header = engine.request_leaf(&LayoutStyle::new().height(px(header_height)));
    let body = engine.request_layout(&LayoutStyle::new().flex_grow(1.0), &[]);

    let panel = engine.request_layout(
        &LayoutStyle::new()
            .flex_col()
            .gap(gap)
            .padding(padding)
            .width(px(bounds.size.width))
            .height(px(bounds.size.height)),
        &[header, body],
    );

    engine.compute_layout(panel, Size::new(bounds.size.width, bounds.size.height));

    PanelLayout {
        header: offset_bounds(engine.layout(header), bounds.origin),
        body: offset_bounds(engine.layout(body), bounds.origin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) {
        assert!(
            (a - b).abs() < 0.5,
            "expected {a} ~= {b} (diff {})",
            (a - b).abs()
        );
    }

    #[test]
    fn test_stack_bounds_positions() {
        let bounds = Bounds::new(5.0, 7.0, 200.0, 200.0);
        let items = stack_bounds(bounds, &[20.0, 30.0], 10.0);

        assert_eq!(items.len(), 2);
        approx_eq(items[0].origin.x, 5.0);
        approx_eq(items[0].origin.y, 7.0);
        approx_eq(items[0].size.height, 20.0);
        approx_eq(items[1].origin.y, 7.0 + 20.0 + 10.0);
        approx_eq(items[1].size.height, 30.0);
    }

    #[test]
    fn test_row_bounds_with_flex() {
        let bounds = Bounds::new(10.0, 20.0, 300.0, 40.0);
        let items = [
            RowItem::fixed(100.0),
            RowItem::flex(1.0),
            RowItem::flex(1.0),
        ];
        let rows = row_bounds(bounds, 24.0, &items, 10.0);

        assert_eq!(rows.len(), 3);
        approx_eq(rows[0].origin.x, 10.0);
        approx_eq(rows[0].size.width, 100.0);
        approx_eq(rows[0].size.height, 24.0);

        let remaining = 300.0 - 100.0 - 10.0 * 2.0;
        let flex_width = remaining / 2.0;
        approx_eq(rows[1].size.width, flex_width);
        approx_eq(rows[1].origin.x, 10.0 + 100.0 + 10.0);
        approx_eq(rows[2].origin.x, 10.0 + 100.0 + 10.0 + flex_width + 10.0);
    }

    #[test]
    fn test_grid_bounds_wraps_rows() {
        let bounds = Bounds::new(0.0, 0.0, 220.0, 100.0);
        let cells = grid_bounds(bounds, Size::new(50.0, 20.0), 4, 10.0);

        assert_eq!(cells.len(), 4);
        approx_eq(cells[0].origin.x, 0.0);
        approx_eq(cells[0].origin.y, 0.0);
        approx_eq(cells[1].origin.x, 60.0);
        approx_eq(cells[2].origin.x, 120.0);
        approx_eq(cells[3].origin.y, 30.0);
    }
}
