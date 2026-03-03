use wgpui::PaintContext;

use crate::app_state::CastControlPaneState;
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    cast_check_button_bounds, cast_inspect_button_bounds, cast_prove_button_bounds,
    cast_loop_once_button_bounds, cast_refresh_button_bounds, cast_sign_button_bounds,
    cast_toggle_broadcast_button_bounds, cast_toggle_loop_button_bounds,
};

pub fn paint_cast_control_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &CastControlPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, &pane_state.source, paint);

    let refresh = cast_refresh_button_bounds(content_bounds);
    let check = cast_check_button_bounds(content_bounds);
    let prove = cast_prove_button_bounds(content_bounds);
    let sign = cast_sign_button_bounds(content_bounds);
    let inspect = cast_inspect_button_bounds(content_bounds);
    let broadcast = cast_toggle_broadcast_button_bounds(content_bounds);
    let loop_once = cast_loop_once_button_bounds(content_bounds);
    let loop_toggle = cast_toggle_loop_button_bounds(content_bounds);

    paint_action_button(refresh, "Refresh Status", paint);
    paint_action_button(check, "Run Check", paint);
    paint_action_button(prove, "Run Prove", paint);
    paint_action_button(sign, "Sign/Broadcast", paint);
    paint_action_button(inspect, "Inspect Spell", paint);
    paint_action_button(
        broadcast,
        if pane_state.broadcast_armed {
            "Broadcast Armed"
        } else {
            "Broadcast Safe"
        },
        paint,
    );
    paint_action_button(loop_once, "Run Loop Once", paint);
    paint_action_button(
        loop_toggle,
        if pane_state.auto_loop_enabled {
            "Stop Auto Loop"
        } else {
            "Start Auto Loop"
        },
        paint,
    );

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        loop_toggle.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Prereqs",
        &pane_state.prereq_status,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Runs",
        &pane_state.run_count.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Auto loop",
        if pane_state.auto_loop_enabled {
            "enabled"
        } else {
            "disabled"
        },
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Loop interval",
        &format!("{}s", pane_state.auto_loop_interval.as_secs()),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Active pid",
        pane_state.active_pid.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Loop config",
        &pane_state.loop_config_path,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last operation",
        pane_state.last_operation.as_deref().unwrap_or("none"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last txid",
        pane_state.last_txid.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last log",
        pane_state.last_log_path.as_deref().unwrap_or("n/a"),
    );
    let _ = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Last receipt",
        pane_state.last_receipt_path.as_deref().unwrap_or("n/a"),
    );
}
