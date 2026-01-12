use wgpui::components::hud::{CornerConfig, Frame, FrameStyle};
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad,
    Scene, Text, TextSystem, theme,
};

use crate::constants::PANEL_PADDING;

pub(crate) fn draw_panel(
    title: &str,
    bounds: Bounds,
    cx: &mut PaintContext,
    paint: impl FnOnce(Bounds, &mut PaintContext),
) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let title_bounds = Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING,
        bounds.size.width - PANEL_PADDING * 2.0,
        24.0,
    );
    let mut title_text = Text::new(title)
        .font_size(theme::font_size::BASE)
        .color(theme::text::PRIMARY);
    title_text.paint(title_bounds, cx);

    let inner = Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING + 28.0,
        (bounds.size.width - PANEL_PADDING * 2.0).max(0.0),
        (bounds.size.height - PANEL_PADDING * 2.0 - 28.0).max(0.0),
    );
    paint(inner, cx);
}

pub(crate) fn panel_inner(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING + 28.0,
        (bounds.size.width - PANEL_PADDING * 2.0).max(0.0),
        (bounds.size.height - PANEL_PADDING * 2.0 - 28.0).max(0.0),
    )
}

pub(crate) fn draw_tile(
    bounds: Bounds,
    label: &str,
    cx: &mut PaintContext,
    paint: impl FnOnce(Bounds, &mut PaintContext),
) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut text = Text::new(label)
        .font_size(theme::font_size::SM)
        .color(theme::text::MUTED);
    text.paint(
        Bounds::new(
            bounds.origin.x + 10.0,
            bounds.origin.y + 8.0,
            bounds.size.width - 20.0,
            20.0,
        ),
        cx,
    );

    let inner = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 32.0,
        (bounds.size.width - 20.0).max(0.0),
        (bounds.size.height - 42.0).max(0.0),
    );
    paint(inner, cx);
}

#[allow(dead_code)]
pub(crate) fn paint_centered(
    component: &mut impl Component,
    bounds: Bounds,
    cx: &mut PaintContext,
) {
    let (w, h) = component.size_hint();
    let width = w.unwrap_or(bounds.size.width).min(bounds.size.width);
    let height = h.unwrap_or(bounds.size.height).min(bounds.size.height);
    let x = bounds.origin.x + (bounds.size.width - width) / 2.0;
    let y = bounds.origin.y + (bounds.size.height - height) / 2.0;
    component.paint(Bounds::new(x, y, width, height), cx);
}

pub(crate) fn center_bounds(bounds: Bounds, width: f32, height: f32) -> Bounds {
    Bounds::new(
        bounds.origin.x + (bounds.size.width - width) / 2.0,
        bounds.origin.y + (bounds.size.height - height) / 2.0,
        width.min(bounds.size.width),
        height.min(bounds.size.height),
    )
}

pub(crate) fn component_event(
    component: &mut impl Component,
    event: &InputEvent,
    bounds: Bounds,
    cx: &mut EventContext,
) -> bool {
    matches!(component.event(event, bounds, cx), EventResult::Handled)
}

pub(crate) struct GridMetrics {
    pub(crate) cols: usize,
    pub(crate) height: f32,
}

pub(crate) fn grid_metrics(
    width: f32,
    items: usize,
    tile_w: f32,
    tile_h: f32,
    gap: f32,
) -> GridMetrics {
    let cols = (((width + gap) / (tile_w + gap)).floor() as usize).max(1);
    let rows = if items == 0 {
        0
    } else {
        (items + cols - 1) / cols
    };
    let height = if rows == 0 {
        0.0
    } else {
        rows as f32 * tile_h + (rows as f32 - 1.0) * gap
    };
    GridMetrics { cols, height }
}

pub(crate) fn panel_height(inner_height: f32) -> f32 {
    inner_height + PANEL_PADDING * 2.0 + 28.0
}

pub(crate) fn stacked_height(panels: &[f32]) -> f32 {
    let mut height = 0.0;
    for (idx, panel) in panels.iter().enumerate() {
        if idx > 0 {
            height += crate::constants::SECTION_GAP;
        }
        height += *panel;
    }
    height
}

pub(crate) fn inset_bounds(bounds: Bounds, inset: f32) -> Bounds {
    Bounds::new(
        bounds.origin.x + inset,
        bounds.origin.y + inset,
        (bounds.size.width - inset * 2.0).max(0.0),
        (bounds.size.height - inset * 2.0).max(0.0),
    )
}

pub(crate) fn draw_bitcoin_symbol(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    y: f32,
    font_size: f32,
    color: Hsla,
) {
    let bar_h = font_size * 0.18;
    let bar_w = 2.0;
    let bar_x1 = x + font_size * 0.15;
    let bar_x2 = x + font_size * 0.38;

    scene.draw_quad(
        Quad::new(Bounds::new(bar_x1, y - bar_h + 2.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x2, y - bar_h + 2.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x1, y + font_size - 4.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x2, y + font_size - 4.0, bar_w, bar_h)).with_background(color),
    );

    let b = text_system.layout("B", Point::new(x, y), font_size, color);
    scene.draw_text(b);
}

pub(crate) fn demo_frame(style: FrameStyle) -> Frame {
    match style {
        FrameStyle::Corners => Frame::corners().corner_length(18.0),
        FrameStyle::Lines => Frame::lines(),
        FrameStyle::Octagon => Frame::octagon().corner_length(14.0),
        FrameStyle::Underline => Frame::underline().square_size(12.0),
        FrameStyle::Nefrex => Frame::nefrex()
            .corner_config(CornerConfig::diagonal())
            .square_size(10.0)
            .small_line_length(10.0)
            .large_line_length(35.0),
        FrameStyle::Kranox => Frame::kranox()
            .square_size(10.0)
            .small_line_length(10.0)
            .large_line_length(35.0),
        FrameStyle::Nero => Frame::nero().corner_length(20.0),
        FrameStyle::Header => Frame::header().corner_length(12.0).header_bottom(true),
        FrameStyle::Circle => Frame::circle().circle_segments(48),
    }
}
