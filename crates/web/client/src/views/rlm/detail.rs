//! RLM dashboard run detail view.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::PaintContext;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::AppState;
use super::{
    bg_dark, bg_panel, border_color, format_duration_ms, format_time_ago, state_active,
    state_complete, state_error, state_pending, text_muted, text_primary, truncate_text,
    wrap_text, FONT_BODY, FONT_HEADER, FONT_SMALL, FONT_TITLE,
};

pub(crate) fn build_rlm_detail_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(bg_dark()));

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.08))
        .distance(40.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    let padding = 20.0;
    let content_width = (width - padding * 2.0).min(1200.0);
    let content_x = (width - content_width) / 2.0;
    let content_y = padding;
    let content_height = height - padding * 2.0;
    state.rlm_detail.content_bounds = Bounds::new(content_x, content_y, content_width, content_height);

    if !state.rlm_detail.frame_started {
        state.rlm_detail.frame_started = true;
    }
    let frame_progress = state.rlm_detail.frame_animator.update(AnimatorState::Entering);

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    Frame::corners()
        .line_color(border_color())
        .bg_color(bg_panel())
        .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.05))
        .border_color(border_color())
        .stroke_width(1.0)
        .corner_length(20.0)
        .animation_progress(frame_progress)
        .paint(Bounds::new(content_x, content_y, content_width, content_height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    let inner_padding = 16.0;
    let inner_x = content_x + inner_padding;
    let inner_width = content_width - inner_padding * 2.0;
    let mut y = content_y + inner_padding - state.rlm_detail.scroll_offset;

    // Header
    let title = "RLM RUN";
    let title_run = text_system.layout(title, Point::new(inner_x, y), FONT_TITLE, text_primary());
    scene.draw_text(title_run);

    let back_text = "Back to runs";
    let back_width = text_system.measure(back_text, FONT_SMALL) + 16.0;
    let back_bounds = Bounds::new(inner_x + inner_width - back_width, y + 2.0, back_width, 22.0);
    state.rlm_detail.back_button_bounds = back_bounds;
    let back_color = if state.rlm_detail.back_button_hovered {
        text_primary()
    } else {
        text_muted()
    };
    scene.draw_quad(
        Quad::new(back_bounds)
            .with_background(bg_panel().with_alpha(0.3))
            .with_border(back_color, 1.0),
    );
    let back_run =
        text_system.layout(back_text, Point::new(back_bounds.x() + 8.0, back_bounds.y() + 4.0), FONT_SMALL, back_color);
    scene.draw_text(back_run);

    y += 34.0;

    // Summary panel
    let summary_padding = 12.0;
    let mut summary_height = 96.0;
    if let Some(run) = state.rlm_detail.run.as_ref() {
        let query_lines = wrap_text(text_system, &run.query, inner_width - summary_padding * 2.0, FONT_BODY);
        summary_height += query_lines.len() as f32 * (FONT_BODY + 4.0) + 24.0;
    }

    let summary_bounds = Bounds::new(inner_x, y, inner_width, summary_height);
    scene.draw_quad(
        Quad::new(summary_bounds)
            .with_background(bg_panel().with_alpha(0.85))
            .with_border(border_color(), 1.0),
    );

    if state.rlm_detail.loading {
        let loading = "Loading run...";
        let loading_run = text_system.layout(
            loading,
            Point::new(summary_bounds.x() + summary_padding, summary_bounds.y() + summary_padding),
            FONT_BODY,
            text_muted(),
        );
        scene.draw_text(loading_run);
    } else if let Some(error) = state.rlm_detail.error.as_ref() {
        let message = format!("Error loading run: {}", error);
        let message_run = text_system.layout(
            &message,
            Point::new(summary_bounds.x() + summary_padding, summary_bounds.y() + summary_padding),
            FONT_BODY,
            state_error(),
        );
        scene.draw_text(message_run);
    } else if let Some(run) = state.rlm_detail.run.as_ref() {
        let status = run.status.to_uppercase();
        let status_color = status_color(&run.status);
        let status_width = text_system.measure(&status, FONT_SMALL) + 12.0;
        let status_bounds = Bounds::new(
            summary_bounds.x() + summary_padding,
            summary_bounds.y() + summary_padding,
            status_width,
            18.0,
        );
        scene.draw_quad(
            Quad::new(status_bounds)
                .with_background(status_color.with_alpha(0.15))
                .with_border(status_color, 1.0),
        );
        let status_run = text_system.layout(
            &status,
            Point::new(status_bounds.x() + 6.0, status_bounds.y() + 2.0),
            FONT_SMALL,
            status_color,
        );
        scene.draw_text(status_run);

        let id_label = format!("Run {}", truncate_text(&run.id, 18));
        let id_run = text_system.layout(
            &id_label,
            Point::new(status_bounds.x() + status_bounds.width() + 12.0, status_bounds.y() + 2.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(id_run);

        let mut query_y = status_bounds.y() + 26.0;
        let query_lines = wrap_text(text_system, &run.query, summary_bounds.width() - summary_padding * 2.0, FONT_BODY);
        for line in query_lines {
            let line_run = text_system.layout(
                &line,
                Point::new(summary_bounds.x() + summary_padding, query_y),
                FONT_BODY,
                text_primary(),
            );
            scene.draw_text(line_run);
            query_y += FONT_BODY + 4.0;
        }

        query_y += 6.0;
        let stats_line = format!(
            "Fragments: {} | Budget: {} sats | Cost: {} sats",
            run.fragment_count,
            run.budget_sats,
            run.total_cost_sats,
        );
        let stats_run = text_system.layout(
            &stats_line,
            Point::new(summary_bounds.x() + summary_padding, query_y),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(stats_run);

        query_y += FONT_SMALL + 4.0;
        let completed = run
            .completed_at
            .map(format_time_ago)
            .unwrap_or_else(|| "in progress".to_string());
        let timing_line = format!(
            "Duration: {} | Created: {} | Completed: {}",
            format_duration_ms(run.total_duration_ms),
            format_time_ago(run.created_at),
            completed,
        );
        let timing_run = text_system.layout(
            &timing_line,
            Point::new(summary_bounds.x() + summary_padding, query_y),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(timing_run);
    } else {
        let empty = "Run not found.";
        let empty_run = text_system.layout(
            empty,
            Point::new(summary_bounds.x() + summary_padding, summary_bounds.y() + summary_padding),
            FONT_BODY,
            text_muted(),
        );
        scene.draw_text(empty_run);
    }

    y += summary_height + 16.0;

    // Output panel
    let output_title;
    let output_text;
    let output_color;
    if let Some(run) = state.rlm_detail.run.as_ref() {
        if let Some(error) = run.error_message.as_ref() {
            output_title = "ERROR".to_string();
            output_text = error.clone();
            output_color = state_error();
        } else if let Some(output) = run.output.as_ref() {
            output_title = "OUTPUT".to_string();
            output_text = output.clone();
            output_color = text_primary();
        } else {
            output_title = "OUTPUT".to_string();
            output_text = "Output pending...".to_string();
            output_color = text_muted();
        }
    } else {
        output_title = "OUTPUT".to_string();
        output_text = "Output pending...".to_string();
        output_color = text_muted();
    }

    let output_lines = wrap_text(text_system, &output_text, inner_width - summary_padding * 2.0, FONT_BODY);
    let output_height = (output_lines.len() as f32 * (FONT_BODY + 4.0) + 44.0).max(120.0);
    let output_bounds = Bounds::new(inner_x, y, inner_width, output_height);
    scene.draw_quad(
        Quad::new(output_bounds)
            .with_background(bg_panel().with_alpha(0.85))
            .with_border(border_color(), 1.0),
    );
    let output_header = text_system.layout(
        &output_title,
        Point::new(output_bounds.x() + summary_padding, output_bounds.y() + summary_padding),
        FONT_HEADER,
        output_color,
    );
    scene.draw_text(output_header);

    let mut output_y = output_bounds.y() + summary_padding + 24.0;
    for line in output_lines {
        let line_run = text_system.layout(
            &line,
            Point::new(output_bounds.x() + summary_padding, output_y),
            FONT_BODY,
            output_color,
        );
        scene.draw_text(line_run);
        output_y += FONT_BODY + 4.0;
    }

    y += output_height + 16.0;

    // Trace panel
    let trace_panel_height = (height * 0.45).clamp(240.0, 420.0);
    let trace_bounds = Bounds::new(inner_x, y, inner_width, trace_panel_height);
    scene.draw_quad(
        Quad::new(trace_bounds)
            .with_background(bg_panel().with_alpha(0.85))
            .with_border(border_color(), 1.0),
    );

    let trace_header = text_system.layout(
        "TRACE EVENTS",
        Point::new(trace_bounds.x() + summary_padding, trace_bounds.y() + summary_padding),
        FONT_HEADER,
        text_primary(),
    );
    scene.draw_text(trace_header);

    let live_label = if state.rlm_detail.live_connected { "LIVE" } else { "OFFLINE" };
    let live_color = if state.rlm_detail.live_connected { state_active() } else { text_muted() };
    let live_width = text_system.measure(live_label, FONT_SMALL);
    let live_x = trace_bounds.x() + trace_bounds.width() - live_width - summary_padding;
    let live_run = text_system.layout(live_label, Point::new(live_x, trace_bounds.y() + summary_padding + 2.0), FONT_SMALL, live_color);
    scene.draw_text(live_run);

    let trace_inner_bounds = Bounds::new(
        trace_bounds.x() + summary_padding,
        trace_bounds.y() + 36.0,
        trace_bounds.width() - summary_padding * 2.0,
        trace_bounds.height() - 46.0,
    );
    state.rlm_detail.trace_bounds = trace_inner_bounds;

    if state.rlm_detail.trace_loading {
        let loading = "Loading trace...";
        let loading_run = text_system.layout(
            loading,
            Point::new(trace_inner_bounds.x(), trace_inner_bounds.y() + 8.0),
            FONT_BODY,
            text_muted(),
        );
        scene.draw_text(loading_run);
        state.rlm_detail.trace_content_height = 0.0;
    } else if state.rlm_detail.trace_events.is_empty() {
        let empty = "No trace events yet.";
        let empty_run = text_system.layout(
            empty,
            Point::new(trace_inner_bounds.x(), trace_inner_bounds.y() + 8.0),
            FONT_BODY,
            text_muted(),
        );
        scene.draw_text(empty_run);
        state.rlm_detail.trace_content_height = 0.0;
    } else {
        render_trace_events(scene, text_system, state, trace_inner_bounds);
    }

    y += trace_panel_height + inner_padding;

    state.rlm_detail.content_height = y - content_y + state.rlm_detail.scroll_offset;
    let max_scroll = (state.rlm_detail.content_height - state.rlm_detail.content_bounds.size.height).max(0.0);
    state.rlm_detail.scroll_offset = state.rlm_detail.scroll_offset.clamp(0.0, max_scroll);

    let max_trace = (state.rlm_detail.trace_content_height - state.rlm_detail.trace_bounds.size.height).max(0.0);
    state.rlm_detail.trace_scroll = state.rlm_detail.trace_scroll.clamp(0.0, max_trace);
}

fn render_trace_events(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    trace_bounds: Bounds,
) {
    let row_height = 30.0;
    let base_ts = state
        .rlm_detail
        .trace_events
        .first()
        .map(|event| event.timestamp_ms)
        .unwrap_or(0);

    state.rlm_detail.trace_content_height = state.rlm_detail.trace_events.len() as f32 * row_height;

    let visible_top = trace_bounds.y();
    let visible_bottom = trace_bounds.y() + trace_bounds.height();

    for (idx, event) in state.rlm_detail.trace_events.iter().enumerate() {
        let row_y = trace_bounds.y() + (idx as f32 * row_height) - state.rlm_detail.trace_scroll;
        if row_y + row_height < visible_top || row_y > visible_bottom {
            continue;
        }

        let row_bounds = Bounds::new(trace_bounds.x(), row_y, trace_bounds.width(), row_height - 4.0);
        if idx % 2 == 0 {
            scene.draw_quad(Quad::new(row_bounds).with_background(bg_panel().with_alpha(0.7)));
        }

        let delta_ms = (event.timestamp_ms - base_ts).max(0);
        let ts_label = format!("+{}ms", delta_ms);
        let ts_run = text_system.layout(
            &ts_label,
            Point::new(row_bounds.x() + 4.0, row_bounds.y() + 6.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(ts_run);

        let type_label = event.event_type.replace('_', " ").to_uppercase();
        let type_run = text_system.layout(
            &type_label,
            Point::new(row_bounds.x() + 84.0, row_bounds.y() + 6.0),
            FONT_SMALL,
            text_primary(),
        );
        scene.draw_text(type_run);

        let summary_x = row_bounds.x() + 220.0;
        let summary = truncate_text(&event.summary, 160);
        let summary_run = text_system.layout(
            &summary,
            Point::new(summary_x, row_bounds.y() + 6.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(summary_run);
    }
}

pub(crate) fn handle_rlm_detail_mouse_move(state: &mut AppState, x: f32, y: f32) {
    let point = Point::new(x, y);
    state.rlm_detail.back_button_hovered = state.rlm_detail.back_button_bounds.contains(point);
}

pub(crate) fn handle_rlm_detail_click(state: &mut AppState, x: f32, y: f32) -> bool {
    let point = Point::new(x, y);
    state.rlm_detail.back_button_bounds.contains(point)
}

pub(crate) fn handle_rlm_detail_scroll(state: &mut AppState, x: f32, y: f32, delta_y: f32) -> bool {
    let point = Point::new(x, y);
    let delta = delta_y * 0.5;

    if state.rlm_detail.trace_bounds.contains(point) {
        state.rlm_detail.trace_scroll += delta;
        let max_scroll = (state.rlm_detail.trace_content_height - state.rlm_detail.trace_bounds.size.height).max(0.0);
        state.rlm_detail.trace_scroll = state.rlm_detail.trace_scroll.clamp(0.0, max_scroll);
        return true;
    }

    if state.rlm_detail.content_bounds.contains(point) {
        state.rlm_detail.scroll_offset += delta;
        let max_scroll = (state.rlm_detail.content_height - state.rlm_detail.content_bounds.size.height).max(0.0);
        state.rlm_detail.scroll_offset = state.rlm_detail.scroll_offset.clamp(0.0, max_scroll);
        return true;
    }

    false
}

fn status_color(status: &str) -> Hsla {
    let lower = status.to_lowercase();
    if lower.contains("complete") {
        state_complete()
    } else if lower.contains("fail") || lower.contains("error") {
        state_error()
    } else if lower.contains("run") {
        state_active()
    } else {
        state_pending()
    }
}
