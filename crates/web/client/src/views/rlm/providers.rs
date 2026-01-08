//! RLM provider leaderboard view.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::PaintContext;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::AppState;
use super::{
    bg_dark, bg_panel, border_color, format_duration_ms, text_muted, text_primary, truncate_text,
    FONT_BODY, FONT_SMALL, FONT_TITLE,
};

pub(crate) fn build_rlm_providers_page(
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

    if !state.rlm_providers.frame_started {
        state.rlm_providers.frame_started = true;
    }
    let frame_progress = state.rlm_providers.frame_animator.update(AnimatorState::Entering);

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

    let title = "PROVIDER LEADERBOARD";
    let title_run = text_system.layout(title, Point::new(inner_x, y), FONT_TITLE, text_primary());
    scene.draw_text(title_run);

    let count_text = format!("{} providers", state.rlm_providers.providers.len());
    let count_width = text_system.measure(&count_text, FONT_SMALL);
    let count_x = inner_x + inner_width - count_width;
    let count_run = text_system.layout(&count_text, Point::new(count_x, y + 4.0), FONT_SMALL, text_muted());
    scene.draw_text(count_run);

    y += 32.0;
    let subtitle = "Top providers by cost";
    let subtitle_run = text_system.layout(subtitle, Point::new(inner_x, y), FONT_SMALL, text_muted());
    scene.draw_text(subtitle_run);
    y += 20.0;

    let list_height = content_y + content_height - y - inner_padding;
    let list_bounds = Bounds::new(inner_x, y, inner_width, list_height);
    state.rlm_providers.content_bounds = list_bounds;

    if state.rlm_providers.loading {
        let loading = "Loading providers...";
        let loading_width = text_system.measure(loading, FONT_BODY);
        let loading_x = list_bounds.x() + (list_bounds.width() - loading_width) / 2.0;
        let loading_y = list_bounds.y() + list_bounds.height() / 2.0;
        let loading_run = text_system.layout(loading, Point::new(loading_x, loading_y), FONT_BODY, text_muted());
        scene.draw_text(loading_run);
        state.rlm_providers.row_bounds.clear();
        state.rlm_providers.content_height = 0.0;
        return;
    }

    if let Some(error) = state.rlm_providers.error.as_ref() {
        let message = format!("Error loading providers: {}", error);
        let message_width = text_system.measure(&message, FONT_BODY);
        let message_x = list_bounds.x() + (list_bounds.width() - message_width) / 2.0;
        let message_y = list_bounds.y() + list_bounds.height() / 2.0;
        let message_run = text_system.layout(&message, Point::new(message_x, message_y), FONT_BODY, text_muted());
        scene.draw_text(message_run);
        state.rlm_providers.row_bounds.clear();
        state.rlm_providers.content_height = 0.0;
        return;
    }

    if state.rlm_providers.providers.is_empty() {
        let empty = "No provider stats yet.";
        let empty_width = text_system.measure(empty, FONT_BODY);
        let empty_x = list_bounds.x() + (list_bounds.width() - empty_width) / 2.0;
        let empty_y = list_bounds.y() + list_bounds.height() / 2.0;
        let empty_run = text_system.layout(empty, Point::new(empty_x, empty_y), FONT_BODY, text_muted());
        scene.draw_text(empty_run);
        state.rlm_providers.row_bounds.clear();
        state.rlm_providers.content_height = 0.0;
        return;
    }

    let row_height = 64.0;
    state.rlm_providers.content_height = state.rlm_providers.providers.len() as f32 * row_height;
    let max_scroll = (state.rlm_providers.content_height - list_bounds.size.height).max(0.0);
    state.rlm_providers.scroll_offset = state.rlm_providers.scroll_offset.clamp(0.0, max_scroll);
    state.rlm_providers.row_bounds.clear();

    let list_bottom = list_bounds.y() + list_bounds.height();
    for (i, provider) in state.rlm_providers.providers.iter().enumerate() {
        let row_y = list_bounds.y() + (i as f32 * row_height) - state.rlm_providers.scroll_offset;
        if row_y + row_height < list_bounds.y() || row_y > list_bottom {
            state.rlm_providers.row_bounds.push(Bounds::ZERO);
            continue;
        }

        let row_bounds = Bounds::new(list_bounds.x(), row_y, list_bounds.width(), row_height - 8.0);
        state.rlm_providers.row_bounds.push(row_bounds);

        let is_hovered = state.rlm_providers.hovered_provider_idx == Some(i);
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

        let provider_name = truncate_text(&provider.provider_id, 40);
        let provider_run = text_system.layout(
            &provider_name,
            Point::new(row_bounds.x() + 12.0, row_bounds.y() + 12.0),
            FONT_BODY,
            text_primary(),
        );
        scene.draw_text(provider_run);

        let venue = provider.venue.as_deref().unwrap_or("unknown");
        let venue_text = format!("venue: {}", venue);
        let venue_run = text_system.layout(
            &venue_text,
            Point::new(row_bounds.x() + 12.0, row_bounds.y() + 34.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(venue_run);

        let stats = format!(
            "queries {} | success {:.0}% | avg {}",
            provider.total_queries,
            provider.success_rate * 100.0,
            format_duration_ms(provider.avg_duration_ms as i64),
        );
        let stats_width = text_system.measure(&stats, FONT_SMALL);
        let stats_x = row_bounds.x() + row_bounds.width() - stats_width - 12.0;
        let stats_run = text_system.layout(
            &stats,
            Point::new(stats_x, row_bounds.y() + 12.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(stats_run);

        let cost_text = format!("{} sats", provider.total_cost_sats);
        let cost_width = text_system.measure(&cost_text, FONT_SMALL);
        let cost_x = row_bounds.x() + row_bounds.width() - cost_width - 12.0;
        let cost_run = text_system.layout(
            &cost_text,
            Point::new(cost_x, row_bounds.y() + 34.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(cost_run);
    }
}

pub(crate) fn handle_rlm_providers_mouse_move(state: &mut AppState, x: f32, y: f32) {
    let point = Point::new(x, y);
    let hovered = state
        .rlm_providers
        .row_bounds
        .iter()
        .enumerate()
        .find(|(_, bounds)| bounds.contains(point))
        .map(|(idx, _)| idx);
    state.rlm_providers.hovered_provider_idx = hovered;
}

pub(crate) fn handle_rlm_providers_scroll(
    state: &mut AppState,
    x: f32,
    y: f32,
    delta_y: f32,
) -> bool {
    let point = Point::new(x, y);
    if !state.rlm_providers.content_bounds.contains(point) {
        return false;
    }

    let delta = delta_y * 0.5;
    state.rlm_providers.scroll_offset += delta;
    let max_scroll = (state.rlm_providers.content_height - state.rlm_providers.content_bounds.size.height).max(0.0);
    state.rlm_providers.scroll_offset = state.rlm_providers.scroll_offset.clamp(0.0, max_scroll);
    true
}
