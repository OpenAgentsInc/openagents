use wgpui::{Component, PaintContext, Point, theme};

use crate::app_state::{LogStreamLevelFilter, LogStreamPaneState};
use crate::pane_renderer::paint_action_button;
use crate::pane_system::{
    log_stream_copy_button_bounds, log_stream_filter_button_bounds, log_stream_terminal_bounds,
};

pub fn paint(
    content_bounds: wgpui::Bounds,
    pane_state: &mut LogStreamPaneState,
    paint: &mut PaintContext,
) {
    paint_action_button(
        log_stream_copy_button_bounds(content_bounds),
        "Copy all",
        paint,
    );
    let filter_label = match pane_state
        .active_level_filter()
        .unwrap_or(LogStreamLevelFilter::Info)
    {
        LogStreamLevelFilter::Debug => "DBG",
        LogStreamLevelFilter::Info => "INF",
        LogStreamLevelFilter::Warn => "WRN",
        LogStreamLevelFilter::Error => "ERR",
    };
    paint_action_button(
        log_stream_filter_button_bounds(content_bounds),
        filter_label,
        paint,
    );
    paint.scene.draw_text(paint.text.layout(
        "Replay-safe runtime logs for provider, buyer, wallet, and mirrored trace output.",
        Point::new(
            content_bounds.origin.x + 12.0,
            content_bounds.origin.y + 12.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
    pane_state.terminal.set_title("");
    pane_state
        .terminal
        .paint(log_stream_terminal_bounds(content_bounds), paint);
}
