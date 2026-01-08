//! RLM experiment detail + comparison view.

use wgpui::animation::AnimatorState;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::components::Component;
use wgpui::PaintContext;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

use crate::state::AppState;
use super::{
    bg_dark, bg_panel, border_color, format_duration_ms, format_time_ago, state_active,
    state_complete, state_error, state_pending, text_muted, text_primary, truncate_text,
    FONT_BODY, FONT_HEADER, FONT_SMALL, FONT_TITLE,
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ExperimentDetailAction {
    Back,
    ExportCsv,
    ExportJson,
    OpenRun(String),
}

pub(crate) fn build_rlm_experiment_detail_page(
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
    state.rlm_experiment_detail.content_bounds = Bounds::new(content_x, content_y, content_width, content_height);

    if !state.rlm_experiment_detail.frame_started {
        state.rlm_experiment_detail.frame_started = true;
    }
    let frame_progress = state.rlm_experiment_detail.frame_animator.update(AnimatorState::Entering);

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
    let mut y = content_y + inner_padding - state.rlm_experiment_detail.scroll_offset;

    let title = "EXPERIMENT";
    let title_run = text_system.layout(title, Point::new(inner_x, y), FONT_TITLE, text_primary());
    scene.draw_text(title_run);

    let back_text = "Back";
    let back_width = text_system.measure(back_text, FONT_SMALL) + 16.0;
    let back_bounds = Bounds::new(inner_x + inner_width - back_width, y + 2.0, back_width, 22.0);
    state.rlm_experiment_detail.back_button_bounds = back_bounds;
    let back_color = if state.rlm_experiment_detail.back_button_hovered {
        text_primary()
    } else {
        text_muted()
    };
    scene.draw_quad(
        Quad::new(back_bounds)
            .with_background(bg_panel().with_alpha(0.3))
            .with_border(back_color, 1.0),
    );
    let back_run = text_system.layout(
        back_text,
        Point::new(back_bounds.x() + 8.0, back_bounds.y() + 4.0),
        FONT_SMALL,
        back_color,
    );
    scene.draw_text(back_run);

    let export_text = "Export CSV";
    let export_width = text_system.measure(export_text, FONT_SMALL) + 16.0;
    let export_bounds = Bounds::new(inner_x + inner_width - back_width - export_width - 12.0, y + 2.0, export_width, 22.0);
    state.rlm_experiment_detail.export_csv_bounds = export_bounds;
    let export_color = if state.rlm_experiment_detail.export_csv_hovered {
        text_primary()
    } else {
        text_muted()
    };
    scene.draw_quad(
        Quad::new(export_bounds)
            .with_background(bg_panel().with_alpha(0.3))
            .with_border(export_color, 1.0),
    );
    let export_run = text_system.layout(
        export_text,
        Point::new(export_bounds.x() + 8.0, export_bounds.y() + 4.0),
        FONT_SMALL,
        export_color,
    );
    scene.draw_text(export_run);

    let export_json_text = "Export JSON";
    let export_json_width = text_system.measure(export_json_text, FONT_SMALL) + 16.0;
    let export_json_bounds = Bounds::new(export_bounds.x() - export_json_width - 8.0, y + 2.0, export_json_width, 22.0);
    state.rlm_experiment_detail.export_json_bounds = export_json_bounds;
    let export_json_color = if state.rlm_experiment_detail.export_json_hovered {
        text_primary()
    } else {
        text_muted()
    };
    scene.draw_quad(
        Quad::new(export_json_bounds)
            .with_background(bg_panel().with_alpha(0.3))
            .with_border(export_json_color, 1.0),
    );
    let export_json_run = text_system.layout(
        export_json_text,
        Point::new(export_json_bounds.x() + 8.0, export_json_bounds.y() + 4.0),
        FONT_SMALL,
        export_json_color,
    );
    scene.draw_text(export_json_run);

    y += 32.0;

    let summary_height = 110.0;
    let summary_bounds = Bounds::new(inner_x, y, inner_width, summary_height);
    scene.draw_quad(
        Quad::new(summary_bounds)
            .with_background(bg_panel().with_alpha(0.85))
            .with_border(border_color(), 1.0),
    );

    if state.rlm_experiment_detail.loading {
        let loading = "Loading experiment...";
        let loading_run = text_system.layout(
            loading,
            Point::new(summary_bounds.x() + 12.0, summary_bounds.y() + 12.0),
            FONT_BODY,
            text_muted(),
        );
        scene.draw_text(loading_run);
    } else if let Some(error) = state.rlm_experiment_detail.error.as_ref() {
        let message = format!("Error loading experiment: {}", error);
        let message_run = text_system.layout(
            &message,
            Point::new(summary_bounds.x() + 12.0, summary_bounds.y() + 12.0),
            FONT_BODY,
            text_muted(),
        );
        scene.draw_text(message_run);
    } else if let Some(exp) = state.rlm_experiment_detail.experiment.as_ref() {
        let name_run = text_system.layout(
            &exp.name,
            Point::new(summary_bounds.x() + 12.0, summary_bounds.y() + 12.0),
            FONT_HEADER,
            text_primary(),
        );
        scene.draw_text(name_run);

        let desc = exp.description.as_deref().unwrap_or("No description");
        let desc = truncate_text(desc, 120);
        let desc_run = text_system.layout(
            &desc,
            Point::new(summary_bounds.x() + 12.0, summary_bounds.y() + 34.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(desc_run);

        let meta = format!(
            "{} runs | updated {}",
            exp.run_count,
            format_time_ago(exp.updated_at)
        );
        let meta_run = text_system.layout(
            &meta,
            Point::new(summary_bounds.x() + 12.0, summary_bounds.y() + 54.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(meta_run);

        let stats = compute_summary_stats(state);
        let stats_line = format!(
            "success {:.0}% | avg cost {} sats | avg duration {}",
            stats.success_rate * 100.0,
            stats.avg_cost_sats,
            format_duration_ms(stats.avg_duration_ms),
        );
        let stats_run = text_system.layout(
            &stats_line,
            Point::new(summary_bounds.x() + 12.0, summary_bounds.y() + 74.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(stats_run);
    }

    y += summary_height + 16.0;

    let table_title = "COMPARISON";
    let table_title_run = text_system.layout(table_title, Point::new(inner_x, y), FONT_HEADER, text_primary());
    scene.draw_text(table_title_run);
    y += 22.0;

    let row_count = state.rlm_experiment_detail.runs.len().max(1) as f32;
    let table_height = 8.0 + 28.0 + 6.0 + row_count * 32.0 + 8.0;
    let table_bounds = Bounds::new(inner_x, y, inner_width, table_height);
    state.rlm_experiment_detail.table_bounds = table_bounds;
    scene.draw_quad(
        Quad::new(table_bounds)
            .with_background(bg_panel().with_alpha(0.85))
            .with_border(border_color(), 1.0),
    );

    render_table(scene, text_system, state, table_bounds);

    y += table_bounds.height() + inner_padding;

    state.rlm_experiment_detail.content_height = y - content_y + state.rlm_experiment_detail.scroll_offset;
    let max_scroll = (state.rlm_experiment_detail.content_height - state.rlm_experiment_detail.content_bounds.size.height).max(0.0);
    state.rlm_experiment_detail.scroll_offset = state.rlm_experiment_detail.scroll_offset.clamp(0.0, max_scroll);
}

pub(crate) fn handle_rlm_experiment_detail_mouse_move(state: &mut AppState, x: f32, y: f32) {
    let point = Point::new(x, y);
    state.rlm_experiment_detail.back_button_hovered =
        state.rlm_experiment_detail.back_button_bounds.contains(point);
    state.rlm_experiment_detail.export_csv_hovered =
        state.rlm_experiment_detail.export_csv_bounds.contains(point);
    state.rlm_experiment_detail.export_json_hovered =
        state.rlm_experiment_detail.export_json_bounds.contains(point);

    let hovered = state
        .rlm_experiment_detail
        .row_bounds
        .iter()
        .enumerate()
        .find(|(_, bounds)| bounds.contains(point))
        .map(|(idx, _)| idx);
    state.rlm_experiment_detail.hovered_run_idx = hovered;
}

pub(crate) fn handle_rlm_experiment_detail_click(
    state: &mut AppState,
    x: f32,
    y: f32,
) -> Option<ExperimentDetailAction> {
    let point = Point::new(x, y);
    if state.rlm_experiment_detail.back_button_bounds.contains(point) {
        return Some(ExperimentDetailAction::Back);
    }
    if state.rlm_experiment_detail.export_csv_bounds.contains(point) {
        return Some(ExperimentDetailAction::ExportCsv);
    }
    if state.rlm_experiment_detail.export_json_bounds.contains(point) {
        return Some(ExperimentDetailAction::ExportJson);
    }

    for (idx, bounds) in state.rlm_experiment_detail.row_bounds.iter().enumerate() {
        if bounds.contains(point) {
            if let Some(run) = state.rlm_experiment_detail.runs.get(idx) {
                return Some(ExperimentDetailAction::OpenRun(run.run.id.clone()));
            }
        }
    }

    None
}

pub(crate) fn handle_rlm_experiment_detail_scroll(
    state: &mut AppState,
    x: f32,
    y: f32,
    delta_y: f32,
) -> bool {
    let point = Point::new(x, y);
    if !state.rlm_experiment_detail.content_bounds.contains(point) {
        return false;
    }

    let delta = delta_y * 0.5;
    state.rlm_experiment_detail.scroll_offset += delta;
    let max_scroll = (state.rlm_experiment_detail.content_height - state.rlm_experiment_detail.content_bounds.size.height).max(0.0);
    state.rlm_experiment_detail.scroll_offset = state.rlm_experiment_detail.scroll_offset.clamp(0.0, max_scroll);
    true
}

struct ExperimentSummaryStats {
    success_rate: f64,
    avg_cost_sats: i64,
    avg_duration_ms: i64,
}

fn compute_summary_stats(state: &AppState) -> ExperimentSummaryStats {
    let total_runs = state.rlm_experiment_detail.runs.len() as i64;
    if total_runs == 0 {
        return ExperimentSummaryStats {
            success_rate: 0.0,
            avg_cost_sats: 0,
            avg_duration_ms: 0,
        };
    }

    let mut success_count = 0i64;
    let mut total_cost = 0i64;
    let mut total_duration = 0i64;
    for run in &state.rlm_experiment_detail.runs {
        if run.run.status == "completed" {
            success_count += 1;
        }
        total_cost += run.run.total_cost_sats;
        total_duration += run.run.total_duration_ms;
    }

    ExperimentSummaryStats {
        success_rate: success_count as f64 / total_runs as f64,
        avg_cost_sats: total_cost / total_runs,
        avg_duration_ms: total_duration / total_runs,
    }
}

fn render_table(scene: &mut Scene, text_system: &mut TextSystem, state: &mut AppState, table_bounds: Bounds) {
    let header_height = 28.0;
    let row_height = 32.0;
    let mut y = table_bounds.y() + 8.0;

    let header_bg = Quad::new(Bounds::new(table_bounds.x(), y, table_bounds.width(), header_height))
        .with_background(bg_panel().with_alpha(0.6))
        .with_border(border_color(), 1.0);
    scene.draw_quad(header_bg);

    let col_label = table_bounds.x() + 12.0;
    let col_status = table_bounds.x() + table_bounds.width() * 0.45;
    let col_cost = table_bounds.x() + table_bounds.width() * 0.62;
    let col_duration = table_bounds.x() + table_bounds.width() * 0.75;
    let col_frag = table_bounds.x() + table_bounds.width() * 0.88;

    let header_runs = text_system.layout("RUN", Point::new(col_label, y + 7.0), FONT_SMALL, text_muted());
    let header_status = text_system.layout("STATUS", Point::new(col_status, y + 7.0), FONT_SMALL, text_muted());
    let header_cost = text_system.layout("COST", Point::new(col_cost, y + 7.0), FONT_SMALL, text_muted());
    let header_duration = text_system.layout("DUR", Point::new(col_duration, y + 7.0), FONT_SMALL, text_muted());
    let header_frag = text_system.layout("FRAG", Point::new(col_frag, y + 7.0), FONT_SMALL, text_muted());
    scene.draw_text(header_runs);
    scene.draw_text(header_status);
    scene.draw_text(header_cost);
    scene.draw_text(header_duration);
    scene.draw_text(header_frag);

    y += header_height + 6.0;

    state.rlm_experiment_detail.row_bounds.clear();
    let visible_bottom = table_bounds.y() + table_bounds.height();

    if state.rlm_experiment_detail.runs.is_empty() {
        let empty = "No runs attached to this experiment.";
        let empty_run = text_system.layout(
            empty,
            Point::new(table_bounds.x() + 12.0, y + 8.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(empty_run);
        return;
    }

    for (idx, entry) in state.rlm_experiment_detail.runs.iter().enumerate() {
        if y + row_height < table_bounds.y() || y > visible_bottom {
            state.rlm_experiment_detail.row_bounds.push(Bounds::ZERO);
            y += row_height;
            continue;
        }

        let row_bounds = Bounds::new(table_bounds.x() + 6.0, y, table_bounds.width() - 12.0, row_height - 4.0);
        state.rlm_experiment_detail.row_bounds.push(row_bounds);

        let is_hovered = state.rlm_experiment_detail.hovered_run_idx == Some(idx);
        if is_hovered {
            scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(bg_panel().with_alpha(0.9))
                    .with_border(border_color(), 1.0),
            );
        }

        let label = entry
            .label
            .as_deref()
            .unwrap_or(&entry.run.query);
        let label = truncate_text(label, 48);
        let label_run = text_system.layout(&label, Point::new(col_label, y + 6.0), FONT_SMALL, text_primary());
        scene.draw_text(label_run);

        let status_color = status_color(&entry.run.status);
        let status_run = text_system.layout(&entry.run.status.to_uppercase(), Point::new(col_status, y + 6.0), FONT_SMALL, status_color);
        scene.draw_text(status_run);

        let cost_run = text_system.layout(
            &format!("{}", entry.run.total_cost_sats),
            Point::new(col_cost, y + 6.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(cost_run);

        let dur_run = text_system.layout(
            &format_duration_ms(entry.run.total_duration_ms),
            Point::new(col_duration, y + 6.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(dur_run);

        let frag_run = text_system.layout(
            &format!("{}", entry.run.fragment_count),
            Point::new(col_frag, y + 6.0),
            FONT_SMALL,
            text_muted(),
        );
        scene.draw_text(frag_run);

        y += row_height;
    }
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
