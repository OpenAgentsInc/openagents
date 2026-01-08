//! RLM dashboard run list view.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::PaintContext;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::AppState;
use super::{
    bg_dark, bg_panel, border_color, format_duration_ms, format_time_ago, state_active,
    state_complete, state_error, state_pending, text_muted, text_primary, truncate_text,
    FONT_BODY, FONT_SMALL, FONT_TITLE,
};

pub(crate) fn build_rlm_list_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(bg_dark()));

    // Dots grid background
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

    if !state.rlm_list.frame_started {
        state.rlm_list.frame_started = true;
    }
    let frame_progress = state.rlm_list.frame_animator.update(AnimatorState::Entering);

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
    let mut y = content_y + inner_padding;

    let title = "RLM DASHBOARD";
    let title_run = text_system.layout(title, Point::new(inner_x, y), FONT_TITLE, text_primary());
    scene.draw_text(title_run);

    let count_text = format!("{} runs", state.rlm_list.runs.len());
    let count_width = text_system.measure(&count_text, FONT_SMALL);
    let count_x = inner_x + inner_width - count_width;
    let count_run =
        text_system.layout(&count_text, Point::new(count_x, y + 4.0), FONT_SMALL, text_muted());
    scene.draw_text(count_run);

    y += 32.0;
    let subtitle = "Recent runs";
    let subtitle_run = text_system.layout(subtitle, Point::new(inner_x, y), FONT_SMALL, text_muted());
    scene.draw_text(subtitle_run);
    y += 20.0;

    let list_height = content_y + content_height - y - inner_padding;
    let list_bounds = Bounds::new(inner_x, y, inner_width, list_height);
    state.rlm_list.content_bounds = list_bounds;

    if state.rlm_list.loading {
        let loading = "Loading runs...";
        let loading_width = text_system.measure(loading, FONT_BODY);
        let loading_x = list_bounds.x() + (list_bounds.width() - loading_width) / 2.0;
        let loading_y = list_bounds.y() + list_bounds.height() / 2.0;
        let loading_run = text_system.layout(loading, Point::new(loading_x, loading_y), FONT_BODY, text_muted());
        scene.draw_text(loading_run);
        state.rlm_list.row_bounds.clear();
        state.rlm_list.content_height = 0.0;
        return;
    }

    if let Some(error) = state.rlm_list.error.as_ref() {
        let message = format!("Error loading runs: {}", error);
        let message_width = text_system.measure(&message, FONT_BODY);
        let message_x = list_bounds.x() + (list_bounds.width() - message_width) / 2.0;
        let message_y = list_bounds.y() + list_bounds.height() / 2.0;
        let message_run = text_system.layout(&message, Point::new(message_x, message_y), FONT_BODY, state_error());
        scene.draw_text(message_run);
        state.rlm_list.row_bounds.clear();
        state.rlm_list.content_height = 0.0;
        return;
    }

    if state.rlm_list.runs.is_empty() {
        let empty = "No runs yet. Sync one with `pylon rlm sync`.";
        let empty_width = text_system.measure(empty, FONT_BODY);
        let empty_x = list_bounds.x() + (list_bounds.width() - empty_width) / 2.0;
        let empty_y = list_bounds.y() + list_bounds.height() / 2.0;
        let empty_run = text_system.layout(empty, Point::new(empty_x, empty_y), FONT_BODY, text_muted());
        scene.draw_text(empty_run);
        state.rlm_list.row_bounds.clear();
        state.rlm_list.content_height = 0.0;
        return;
    }

    let row_height = 64.0;
    state.rlm_list.content_height = state.rlm_list.runs.len() as f32 * row_height;
    let max_scroll = (state.rlm_list.content_height - list_bounds.size.height).max(0.0);
    state.rlm_list.scroll_offset = state.rlm_list.scroll_offset.clamp(0.0, max_scroll);
    state.rlm_list.row_bounds.clear();

    let list_bottom = list_bounds.y() + list_bounds.height();
    for (i, run) in state.rlm_list.runs.iter().enumerate() {
        let row_y = list_bounds.y() + (i as f32 * row_height) - state.rlm_list.scroll_offset;
        if row_y + row_height < list_bounds.y() || row_y > list_bottom {
            state.rlm_list.row_bounds.push(Bounds::ZERO);
            continue;
        }

        let row_bounds = Bounds::new(list_bounds.x(), row_y, list_bounds.width(), row_height - 8.0);
        state.rlm_list.row_bounds.push(row_bounds);

        let is_hovered = state.rlm_list.hovered_run_idx == Some(i);
        let row_bg = if is_hovered {
            bg_panel().with_alpha(0.95)
        } else {
            bg_panel().with_alpha(0.85)
        };
        scene.draw_quad(
            Quad::new(row_bounds)
                .with_background(row_bg)
                .with_border(border_color(), 1.0),
        );

        let status = run.status.to_uppercase();
        let status_color = status_color(&run.status);
        let status_width = text_system.measure(&status, FONT_SMALL) + 12.0;
        let status_bounds = Bounds::new(row_bounds.x() + 12.0, row_bounds.y() + 10.0, status_width, 18.0);
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

        let query_x = status_bounds.x() + status_bounds.width() + 12.0;
        let query = truncate_text(&run.query, 120);
        let query_run = text_system.layout(&query, Point::new(query_x, row_bounds.y() + 10.0), FONT_BODY, text_primary());
        scene.draw_text(query_run);

        let meta = format!(
            "{} sats | {} | {} fragments",
            run.total_cost_sats,
            format_duration_ms(run.total_duration_ms),
            run.fragment_count
        );
        let meta_run = text_system.layout(&meta, Point::new(query_x, row_bounds.y() + 34.0), FONT_SMALL, text_muted());
        scene.draw_text(meta_run);

        let created = format_time_ago(run.created_at);
        let created_width = text_system.measure(&created, FONT_SMALL);
        let created_x = row_bounds.x() + row_bounds.width() - created_width - 12.0;
        let created_run = text_system.layout(&created, Point::new(created_x, row_bounds.y() + 34.0), FONT_SMALL, text_muted());
        scene.draw_text(created_run);
    }
}

pub(crate) fn handle_rlm_list_mouse_move(state: &mut AppState, x: f32, y: f32) {
    let point = Point::new(x, y);
    let hovered = state
        .rlm_list
        .row_bounds
        .iter()
        .enumerate()
        .find(|(_, bounds)| bounds.contains(point))
        .map(|(idx, _)| idx);
    state.rlm_list.hovered_run_idx = hovered;
}

pub(crate) fn handle_rlm_list_click(state: &mut AppState, x: f32, y: f32) -> Option<String> {
    let point = Point::new(x, y);
    for (idx, bounds) in state.rlm_list.row_bounds.iter().enumerate() {
        if bounds.contains(point) {
            if let Some(run) = state.rlm_list.runs.get(idx) {
                return Some(run.id.clone());
            }
        }
    }
    None
}

pub(crate) fn handle_rlm_list_scroll(state: &mut AppState, x: f32, y: f32, delta_y: f32) -> bool {
    let point = Point::new(x, y);
    if !state.rlm_list.content_bounds.contains(point) {
        return false;
    }

    let delta = delta_y * 0.5;
    state.rlm_list.scroll_offset += delta;
    let max_scroll = (state.rlm_list.content_height - state.rlm_list.content_bounds.size.height).max(0.0);
    state.rlm_list.scroll_offset = state.rlm_list.scroll_offset.clamp(0.0, max_scroll);
    true
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
