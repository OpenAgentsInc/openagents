use crate::components::hud::{DotShape, DotsGrid, Scanlines};
use crate::{Bounds, Component, Hsla, PaintContext, Point, Quad};

use super::theme as viz_theme;

pub const PANEL_RADIUS: f32 = 10.0;
pub const PANEL_TITLE_BAR_HEIGHT: f32 = 20.0;
pub const PANEL_TITLE_FONT_SIZE: f32 = 10.0;

pub fn paint_shell(bounds: Bounds, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(viz_theme::surface::PANEL_BG)
            .with_border(accent.with_alpha(0.28), 1.0)
            .with_corner_radius(PANEL_RADIUS),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 1.0,
            bounds.origin.y + 1.0,
            bounds.size.width - 2.0,
            PANEL_TITLE_BAR_HEIGHT,
        ))
        .with_background(accent.with_alpha(viz_theme::surface::PANEL_TITLE_BG_ALPHA))
        .with_corner_radius(PANEL_RADIUS - 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 10.0,
            bounds.max_y() - 3.0,
            bounds.size.width - 20.0,
            1.0,
        ))
        .with_background(accent.with_alpha(viz_theme::surface::PANEL_RULE_ALPHA)),
    );
}

pub fn paint_title(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    let title_y = bounds.origin.y + 1.0 + (PANEL_TITLE_BAR_HEIGHT - PANEL_TITLE_FONT_SIZE) * 0.5;
    paint.scene.draw_text(paint.text.layout_mono(
        title,
        Point::new(bounds.origin.x + 10.0, title_y),
        PANEL_TITLE_FONT_SIZE,
        accent.with_alpha(0.9),
    ));
}

pub fn paint_texture(bounds: Bounds, accent: Hsla, phase: f32, paint: &mut PaintContext) {
    let inner = body_bounds(bounds);
    let mut dots = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(24.0)
        .size(0.8)
        .color(accent.with_alpha(0.08))
        .animation_progress(1.0);
    dots.paint(inner, paint);

    let mut scanlines = Scanlines::new()
        .spacing(16.0)
        .line_color(accent.with_alpha(0.03))
        .scan_color(accent.with_alpha(0.07))
        .scan_width(18.0)
        .scan_progress(phase)
        .opacity(0.56);
    scanlines.paint(inner, paint);
}

pub fn body_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + 12.0,
        bounds.origin.y + 28.0,
        bounds.size.width - 24.0,
        (bounds.size.height - 40.0).max(18.0),
    )
}
