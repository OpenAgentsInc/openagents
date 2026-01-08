//! RLM experiments list view.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::PaintContext;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::AppState;
use super::{
    bg_dark, bg_panel, border_color, format_time_ago, text_muted, text_primary, truncate_text,
    FONT_BODY, FONT_SMALL, FONT_TITLE,
};

pub(crate) fn build_rlm_experiments_page(
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

    if !state.rlm_experiments.frame_started {
        state.rlm_experiments.frame_started = true;
    }
    let frame_progress = state.rlm_experiments.frame_animator.update(AnimatorState::Entering);

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

    let title = "RLM EXPERIMENTS";
    let title_run = text_system.layout(title, Point::new(inner_x, y), FONT_TITLE, text_primary());
    scene.draw_text(title_run);

    let count_text = format!("{} experiments", state.rlm_experiments.experiments.len());
    let count_width = text_system.measure(&count_text, FONT_SMALL);
    let count_x = inner_x + inner_width - count_width;
    let count_run = text_system.layout(&count_text, Point::new(count_x, y + 4.0), FONT_SMALL, text_muted());
    scene.draw_text(count_run);

    y += 32.0;
    let subtitle = "Experiment groups";
    let subtitle_run = text_system.layout(subtitle, Point::new(inner_x, y), FONT_SMALL, text_muted());
    scene.draw_text(subtitle_run);
    y += 20.0;

    let list_height = content_y + content_height - y - inner_padding;
    let list_bounds = Bounds::new(inner_x, y, inner_width, list_height);
    state.rlm_experiments.content_bounds = list_bounds;

    if state.rlm_experiments.loading {
        let loading = "Loading experiments...";
        let loading_width = text_system.measure(loading, FONT_BODY);
        let loading_x = list_bounds.x() + (list_bounds.width() - loading_width) / 2.0;
        let loading_y = list_bounds.y() + list_bounds.height() / 2.0;
        let loading_run = text_system.layout(loading, Point::new(loading_x, loading_y), FONT_BODY, text_muted());
        scene.draw_text(loading_run);
        state.rlm_experiments.row_bounds.clear();
        state.rlm_experiments.content_height = 0.0;
        return;
    }

    if let Some(error) = state.rlm_experiments.error.as_ref() {
        let message = format!("Error loading experiments: {}", error);
        let message_width = text_system.measure(&message, FONT_BODY);
        let message_x = list_bounds.x() + (list_bounds.width() - message_width) / 2.0;
        let message_y = list_bounds.y() + list_bounds.height() / 2.0;
        let message_run = text_system.layout(&message, Point::new(message_x, message_y), FONT_BODY, text_muted());
        scene.draw_text(message_run);
        state.rlm_experiments.row_bounds.clear();
        state.rlm_experiments.content_height = 0.0;
        return;
    }

    if state.rlm_experiments.experiments.is_empty() {
        let empty = "No experiments yet. Create one via API.";
        let empty_width = text_system.measure(empty, FONT_BODY);
        let empty_x = list_bounds.x() + (list_bounds.width() - empty_width) / 2.0;
        let empty_y = list_bounds.y() + list_bounds.height() / 2.0;
        let empty_run = text_system.layout(empty, Point::new(empty_x, empty_y), FONT_BODY, text_muted());
        scene.draw_text(empty_run);
        state.rlm_experiments.row_bounds.clear();
        state.rlm_experiments.content_height = 0.0;
        return;
    }

    let row_height = 72.0;
    state.rlm_experiments.content_height = state.rlm_experiments.experiments.len() as f32 * row_height;
    let max_scroll = (state.rlm_experiments.content_height - list_bounds.size.height).max(0.0);
    state.rlm_experiments.scroll_offset = state.rlm_experiments.scroll_offset.clamp(0.0, max_scroll);
    state.rlm_experiments.row_bounds.clear();

    let list_bottom = list_bounds.y() + list_bounds.height();
    for (i, exp) in state.rlm_experiments.experiments.iter().enumerate() {
        let row_y = list_bounds.y() + (i as f32 * row_height) - state.rlm_experiments.scroll_offset;
        if row_y + row_height < list_bounds.y() || row_y > list_bottom {
            state.rlm_experiments.row_bounds.push(Bounds::ZERO);
            continue;
        }

        let row_bounds = Bounds::new(list_bounds.x(), row_y, list_bounds.width(), row_height - 8.0);
        state.rlm_experiments.row_bounds.push(row_bounds);

        let is_hovered = state.rlm_experiments.hovered_experiment_idx == Some(i);
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

        let name = truncate_text(&exp.name, 80);
        let name_run = text_system.layout(&name, Point::new(row_bounds.x() + 12.0, row_bounds.y() + 12.0), FONT_BODY, text_primary());
        scene.draw_text(name_run);

        let desc = exp.description.as_deref().unwrap_or("No description");
        let desc = truncate_text(desc, 120);
        let desc_run = text_system.layout(&desc, Point::new(row_bounds.x() + 12.0, row_bounds.y() + 34.0), FONT_SMALL, text_muted());
        scene.draw_text(desc_run);

        let runs_label = format!("{} runs", exp.run_count);
        let runs_width = text_system.measure(&runs_label, FONT_SMALL);
        let runs_x = row_bounds.x() + row_bounds.width() - runs_width - 12.0;
        let runs_run = text_system.layout(&runs_label, Point::new(runs_x, row_bounds.y() + 12.0), FONT_SMALL, text_muted());
        scene.draw_text(runs_run);

        let updated = format_time_ago(exp.updated_at);
        let updated_width = text_system.measure(&updated, FONT_SMALL);
        let updated_x = row_bounds.x() + row_bounds.width() - updated_width - 12.0;
        let updated_run = text_system.layout(&updated, Point::new(updated_x, row_bounds.y() + 34.0), FONT_SMALL, text_muted());
        scene.draw_text(updated_run);
    }
}

pub(crate) fn handle_rlm_experiments_mouse_move(state: &mut AppState, x: f32, y: f32) {
    let point = Point::new(x, y);
    let hovered = state
        .rlm_experiments
        .row_bounds
        .iter()
        .enumerate()
        .find(|(_, bounds)| bounds.contains(point))
        .map(|(idx, _)| idx);
    state.rlm_experiments.hovered_experiment_idx = hovered;
}

pub(crate) fn handle_rlm_experiments_click(state: &mut AppState, x: f32, y: f32) -> Option<String> {
    let point = Point::new(x, y);
    for (idx, bounds) in state.rlm_experiments.row_bounds.iter().enumerate() {
        if bounds.contains(point) {
            if let Some(exp) = state.rlm_experiments.experiments.get(idx) {
                return Some(exp.id.clone());
            }
        }
    }
    None
}

pub(crate) fn handle_rlm_experiments_scroll(
    state: &mut AppState,
    x: f32,
    y: f32,
    delta_y: f32,
) -> bool {
    let point = Point::new(x, y);
    if !state.rlm_experiments.content_bounds.contains(point) {
        return false;
    }

    let delta = delta_y * 0.5;
    state.rlm_experiments.scroll_offset += delta;
    let max_scroll = (state.rlm_experiments.content_height - state.rlm_experiments.content_bounds.size.height).max(0.0);
    state.rlm_experiments.scroll_offset = state.rlm_experiments.scroll_offset.clamp(0.0, max_scroll);
    true
}
