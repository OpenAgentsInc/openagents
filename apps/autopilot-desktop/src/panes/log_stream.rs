use wgpui::{Component, PaintContext, Point, theme};

use crate::app_state::LogStreamPaneState;
use crate::pane_renderer::{paint_action_button, paint_source_badge};
use crate::pane_system::{log_stream_copy_button_bounds, log_stream_terminal_bounds};

pub fn paint(
    content_bounds: wgpui::Bounds,
    pane_state: &mut LogStreamPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "log", paint);
    paint_action_button(
        log_stream_copy_button_bounds(content_bounds),
        "Copy all",
        paint,
    );
    paint.scene.draw_text(paint.text.layout(
        "Replay-safe runtime logs for provider, buyer, wallet, and mirrored trace output.",
        Point::new(content_bounds.origin.x + 12.0, content_bounds.origin.y + 12.0),
        10.0,
        theme::text::MUTED,
    ));
    pane_state.terminal.set_title("");
    pane_state
        .terminal
        .paint(log_stream_terminal_bounds(content_bounds), paint);
}
