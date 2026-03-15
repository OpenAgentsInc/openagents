use wgpui::{Bounds, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::{AppleAdapterTrainingPaneState, PaneLoadState};
use crate::desktop_control::{
    DesktopControlAppleAdapterOperatorRunStatus, DesktopControlTrainingStatus,
};
use crate::pane_renderer::{paint_secondary_button, split_text_for_display};
use crate::pane_system::{
    apple_adapter_training_detail_panel_body_bounds, apple_adapter_training_filter_button_bounds,
    apple_adapter_training_launch_panel_body_bounds, apple_adapter_training_layout,
    apple_adapter_training_run_row_bounds,
};

const TRAINING_CARD_GAP: f32 = 8.0;
const TRAINING_ROW_HEIGHT: f32 = 44.0;
const TRAINING_ROW_GAP: f32 = 8.0;
const TRAINING_MAX_RUN_ROWS: usize = 9;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    paint: &mut PaintContext,
) {
    sync_pane_state(pane_state, training_status);

    let layout = apple_adapter_training_layout(content_bounds);
    let selected_run = selected_run(training_status, pane_state.selected_run_id.as_deref());
    let header_cards = training_status_cards(training_status, selected_run);
    let card_width =
        ((layout.status_row.size.width - TRAINING_CARD_GAP * 3.0) / 4.0).max(120.0);
    for (index, (label, value, accent)) in header_cards.iter().enumerate() {
        let x = layout.status_row.origin.x + index as f32 * (card_width + TRAINING_CARD_GAP);
        let width = if index == header_cards.len().saturating_sub(1) {
            (layout.status_row.max_x() - x).max(0.0)
        } else {
            card_width
        };
        paint_status_card(
            Bounds::new(
                x,
                layout.status_row.origin.y,
                width,
                layout.status_row.size.height,
            ),
            label,
            value,
            *accent,
            paint,
        );
    }

    paint_summary_band(
        layout.summary_band,
        training_summary_text(pane_state, training_status, selected_run).as_str(),
        summary_accent(pane_state, training_status),
        paint,
    );

    paint_panel_shell(
        layout.launch_panel,
        "PREFLIGHT SURFACE",
        training_blue(),
        paint,
    );
    paint_panel_shell(layout.runs_panel, "RUNS", training_amber(), paint);
    paint_panel_shell(layout.detail_panel, "RUN DETAIL", training_green(), paint);

    paint_secondary_button(
        apple_adapter_training_filter_button_bounds(content_bounds),
        pane_state.stage_filter.label(),
        paint,
    );

    paint_preflight_panel(
        apple_adapter_training_launch_panel_body_bounds(content_bounds),
        pane_state,
        training_status,
        paint,
    );
    paint_runs_panel(content_bounds, pane_state, training_status, paint);
    paint_detail_panel(
        apple_adapter_training_detail_panel_body_bounds(content_bounds),
        pane_state,
        training_status,
        selected_run,
        paint,
    );
}

pub fn dispatch_input_event(
    _state: &mut crate::app_state::RenderState,
    _event: &wgpui::InputEvent,
) -> bool {
    false
}

fn sync_pane_state(
    pane_state: &mut AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
) {
    if training_status.last_error.is_some() || training_status.operator.last_error.is_some() {
        pane_state.load_state = PaneLoadState::Error;
    } else {
        pane_state.load_state = PaneLoadState::Ready;
    }
    pane_state.last_error = training_status
        .operator
        .last_error
        .clone()
        .or_else(|| training_status.last_error.clone());
    pane_state.last_action = training_status
        .operator
        .last_action
        .clone()
        .or_else(|| training_status.last_error.clone())
        .or_else(|| Some("Apple adapter training pane synced".to_string()));

    let filtered = filtered_runs(training_status, pane_state.stage_filter);
    if pane_state
        .selected_run_id
        .as_deref()
        .is_some_and(|run_id| filtered.iter().any(|run| run.run_id == run_id))
    {
        return;
    }
    pane_state.selected_run_id = filtered.first().map(|run| run.run_id.clone());
}

fn training_status_cards<'a>(
    training_status: &'a DesktopControlTrainingStatus,
    selected_run: Option<&'a DesktopControlAppleAdapterOperatorRunStatus>,
) -> [(&'static str, String, Hsla); 4] {
    let availability = if training_status.available {
        format!("{} // {}", training_status.source, training_status.control_plane_state)
    } else {
        format!("{} // unavailable", training_status.source)
    };
    let operator = if training_status.operator.available {
        format!(
            "{} // {} active",
            training_status.operator.workflow_state,
            training_status.operator.active_run_count
        )
    } else {
        "operator unavailable".to_string()
    };
    let outcomes = format!(
        "{} accepted // {} exported",
        training_status.operator.accepted_run_count, training_status.operator.exported_run_count
    );
    let selection = selected_run.map_or_else(
        || "none selected".to_string(),
        |run| format!("{} // {}", compact_id(run.run_id.as_str(), 18), run.package_name),
    );
    [
        ("TRAINING", availability, training_blue()),
        ("OPERATOR", operator, training_green()),
        ("OUTCOMES", outcomes, training_amber()),
        ("SELECTION", selection, training_red()),
    ]
}

fn summary_accent(
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
) -> Hsla {
    if pane_state.last_error.is_some()
        || training_status.last_error.is_some()
        || training_status.operator.last_error.is_some()
    {
        training_red()
    } else if training_status.operator.active_run_count > 0 {
        training_green()
    } else {
        training_blue()
    }
}

fn training_summary_text(
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    selected_run: Option<&DesktopControlAppleAdapterOperatorRunStatus>,
) -> String {
    let head = pane_state
        .last_error
        .as_deref()
        .or(training_status.last_error.as_deref())
        .map(|value| format!("FAULT {}", compact_id(value, 44)))
        .or_else(|| {
            pane_state
                .last_action
                .as_deref()
                .map(|value| compact_id(value, 52))
        })
        .unwrap_or_else(|| "Waiting for Apple adapter training snapshot".to_string());
    let selection = selected_run.map_or_else(
        || "no run selected".to_string(),
        |run| format!("run {} {}", compact_id(run.run_id.as_str(), 18), run.package_name),
    );
    format!(
        "APPLE ADAPTER TRAINING // {} runs // {} active // {} accepted outcomes // {head} // {selection}",
        training_status.operator.run_count,
        training_status.operator.active_run_count,
        training_status.accepted_outcome_count,
    )
}

fn paint_preflight_panel(
    bounds: Bounds,
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    paint: &mut PaintContext,
) {
    let lines = [
        format!("Training available: {}", truth_label(training_status.available)),
        format!("Projection source: {}", training_status.source),
        format!("Control plane: {}", training_status.control_plane_state),
        format!("Artifact plane: {}", training_status.artifact_plane_state),
        format!(
            "Operator available: {}",
            truth_label(training_status.operator.available)
        ),
        format!("Operator workflow: {}", training_status.operator.workflow_state),
        format!(
            "Environment versions: {}",
            join_labels(training_status.environment_versions.iter().map(String::as_str))
        ),
        format!(
            "Checkpoint refs: {}",
            join_labels(training_status.checkpoint_refs.iter().map(String::as_str))
        ),
    ];
    let chunk_len = section_chunk_len(bounds);
    let mut y = bounds.origin.y + 6.0;
    for line in lines {
        for wrapped in split_text_for_display(line.as_str(), chunk_len) {
            paint.scene.draw_text(paint.text.layout_mono(
                wrapped.as_str(),
                Point::new(bounds.origin.x, y),
                10.0,
                if wrapped.starts_with("Training available: false")
                    || wrapped.starts_with("Operator available: false")
                {
                    training_red()
                } else {
                    theme::text::PRIMARY
                },
            ));
            y += 16.0;
        }
    }
    if let Some(error) = pane_state
        .last_error
        .as_deref()
        .or(training_status.last_error.as_deref())
        .or(training_status.operator.last_error.as_deref())
    {
        y += 6.0;
        for wrapped in split_text_for_display(format!("Last error: {error}").as_str(), chunk_len) {
            paint.scene.draw_text(paint.text.layout_mono(
                wrapped.as_str(),
                Point::new(bounds.origin.x, y),
                10.0,
                training_red(),
            ));
            y += 16.0;
        }
    }
}

fn paint_runs_panel(
    content_bounds: Bounds,
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    paint: &mut PaintContext,
) {
    let runs = filtered_runs(training_status, pane_state.stage_filter);
    let chunk_len = 42;
    if runs.is_empty() {
        let bounds = apple_adapter_training_run_row_bounds(content_bounds, 0);
        paint.scene.draw_text(paint.text.layout_mono(
            "No operator runs match the current filter.",
            Point::new(bounds.origin.x, bounds.origin.y + 10.0),
            10.0,
            theme::text::MUTED,
        ));
        return;
    }

    for (row_index, run) in runs.into_iter().take(TRAINING_MAX_RUN_ROWS).enumerate() {
        let bounds = apple_adapter_training_run_row_bounds(content_bounds, row_index);
        let selected = pane_state.selected_run_id.as_deref() == Some(run.run_id.as_str());
        paint_run_row(bounds, run, selected, chunk_len, paint);
    }
}

fn paint_detail_panel(
    bounds: Bounds,
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    selected_run: Option<&DesktopControlAppleAdapterOperatorRunStatus>,
    paint: &mut PaintContext,
) {
    let chunk_len = section_chunk_len(bounds);
    let mut y = bounds.origin.y + 6.0;
    let detail_lines = if let Some(run) = selected_run {
        vec![
            format!("Run id: {}", run.run_id),
            format!("Package: {}", run.package_name),
            format!("Author: {}", fallback_dash(run.author.as_str())),
            format!("Launch / eval / export / accept: {}", stage_summary(run)),
            format!(
                "Steps: {}/{}",
                run.completed_step_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                run.expected_step_count
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            ),
            format!(
                "Average loss: {}",
                run.average_loss_label.as_deref().unwrap_or("-")
            ),
            format!(
                "Held-out pass / score: {} / {}",
                run.held_out_pass_rate_bps
                    .map(|value| format_bps(value))
                    .unwrap_or_else(|| "-".to_string()),
                run.held_out_average_score_bps
                    .map(|value| format_bps(value))
                    .unwrap_or_else(|| "-".to_string()),
            ),
            format!(
                "Runtime smoke: {}",
                run.runtime_smoke_passed
                    .map(truth_label)
                    .unwrap_or("unknown")
            ),
            format!(
                "Exported path: {}",
                run.exported_package_path.as_deref().unwrap_or("-")
            ),
            format!(
                "Accepted outcome: {}",
                run.authority
                    .accepted_outcome_id
                    .as_deref()
                    .unwrap_or("-")
            ),
            format!("Last action: {}", run.last_action.as_deref().unwrap_or("-")),
            format!("Last error: {}", run.last_error.as_deref().unwrap_or("-")),
        ]
    } else {
        vec![
            "No Apple adapter operator run is selected yet.".to_string(),
            format!("Current filter: {}", pane_state.stage_filter.label()),
            format!(
                "Visible operator runs: {}",
                filtered_runs(training_status, pane_state.stage_filter).len()
            ),
        ]
    };

    for line in detail_lines {
        for wrapped in split_text_for_display(line.as_str(), chunk_len) {
            paint.scene.draw_text(paint.text.layout_mono(
                wrapped.as_str(),
                Point::new(bounds.origin.x, y),
                10.0,
                if wrapped.starts_with("Last error:") && !wrapped.ends_with("-") {
                    training_red()
                } else {
                    theme::text::PRIMARY
                },
            ));
            y += 16.0;
        }
    }
}

fn paint_status_card(
    bounds: Bounds,
    label: &str,
    value: &str,
    accent: Hsla,
    paint: &mut PaintContext,
) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(training_panel())
            .with_border(training_border(), 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            2.0,
        ))
        .with_background(accent.with_alpha(0.88)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 7.0),
        9.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        value,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 22.0),
        11.0,
        accent,
    ));
}

fn paint_summary_band(bounds: Bounds, text: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(accent.with_alpha(0.08))
            .with_border(accent.with_alpha(0.72), 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            10.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.88)),
    );
    let chunk_len = ((bounds.size.width - 28.0) / 6.2).floor().max(18.0) as usize;
    let summary = split_text_for_display(text, chunk_len)
        .into_iter()
        .next()
        .unwrap_or_else(|| "APPLE ADAPTER TRAINING".to_string());
    paint.scene.draw_text(paint.text.layout_mono(
        summary.as_str(),
        Point::new(bounds.origin.x + 16.0, bounds.origin.y + 9.0),
        11.0,
        theme::text::PRIMARY,
    ));
}

fn paint_panel_shell(bounds: Bounds, title: &str, accent: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(training_panel())
            .with_border(training_border(), 1.0),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            4.0,
            bounds.size.height,
        ))
        .with_background(accent.with_alpha(0.88)),
    );
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.origin.x + 4.0,
            bounds.origin.y,
            (bounds.size.width - 4.0).max(0.0),
            22.0,
        ))
        .with_background(accent.with_alpha(0.12)),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        &format!("\\\\ {title}"),
        Point::new(bounds.origin.x + 12.0, bounds.origin.y + 6.0),
        11.0,
        accent,
    ));
}

fn paint_run_row(
    bounds: Bounds,
    run: &DesktopControlAppleAdapterOperatorRunStatus,
    selected: bool,
    chunk_len: usize,
    paint: &mut PaintContext,
) {
    let accent = if run.last_error.is_some() {
        training_red()
    } else if selected {
        training_green()
    } else {
        training_blue()
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(if selected {
                accent.with_alpha(0.12)
            } else {
                training_panel().with_alpha(0.84)
            })
            .with_border(accent.with_alpha(if selected { 0.82 } else { 0.28 }), 1.0),
    );
    let title = format!("{} // {}", compact_id(run.run_id.as_str(), 18), run.package_name);
    let stage_line = format!(
        "{} // loss {} // held-out {}",
        stage_summary(run),
        run.average_loss_label.as_deref().unwrap_or("-"),
        run.held_out_pass_rate_bps
            .map(format_bps)
            .unwrap_or_else(|| "-".to_string()),
    );
    let subtitle = split_text_for_display(stage_line.as_str(), chunk_len)
        .into_iter()
        .next()
        .unwrap_or(stage_line);
    paint.scene.draw_text(paint.text.layout_mono(
        title.as_str(),
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        10.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        subtitle.as_str(),
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 24.0),
        9.0,
        accent,
    ));
}

fn filtered_runs<'a>(
    training_status: &'a DesktopControlTrainingStatus,
    filter: crate::app_state::AppleAdapterTrainingStageFilter,
) -> Vec<&'a DesktopControlAppleAdapterOperatorRunStatus> {
    training_status
        .operator
        .runs
        .iter()
                .filter(|run| run_matches_filter(run, filter))
        .collect()
}

pub(crate) fn run_matches_filter(
    run: &DesktopControlAppleAdapterOperatorRunStatus,
    filter: crate::app_state::AppleAdapterTrainingStageFilter,
) -> bool {
    match filter {
        crate::app_state::AppleAdapterTrainingStageFilter::All => true,
        crate::app_state::AppleAdapterTrainingStageFilter::Active => matches!(
            (
                run.launch_state.as_str(),
                run.evaluation_state.as_str(),
                run.export_state.as_str(),
                run.acceptance_state.as_str(),
            ),
            ("running", _, _, _)
                | (_, "running", _, _)
                | (_, _, "running", _)
                | (_, _, _, "running")
        ),
        crate::app_state::AppleAdapterTrainingStageFilter::Exportable => {
            run.staged_package_path.is_some()
                || run.exported_package_path.is_some()
                || run.export_state == "completed"
        }
        crate::app_state::AppleAdapterTrainingStageFilter::Accepted => {
            run.acceptance_state == "completed" || run.authority.accepted_outcome_id.is_some()
        }
    }
}

fn selected_run<'a>(
    training_status: &'a DesktopControlTrainingStatus,
    selected_run_id: Option<&str>,
) -> Option<&'a DesktopControlAppleAdapterOperatorRunStatus> {
    selected_run_id.and_then(|run_id| {
        training_status
            .operator
            .runs
            .iter()
            .find(|run| run.run_id == run_id)
    })
}

fn section_chunk_len(bounds: Bounds) -> usize {
    ((bounds.size.width - 16.0).max(64.0) / 6.3).floor() as usize
}

fn stage_summary(run: &DesktopControlAppleAdapterOperatorRunStatus) -> String {
    format!(
        "launch {} // eval {} // export {} // accept {}",
        run.launch_state, run.evaluation_state, run.export_state, run.acceptance_state
    )
}

fn truth_label(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn format_bps(value: u32) -> String {
    format!("{:.2}%", value as f32 / 100.0)
}

fn compact_id(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let prefix_len = max_chars.saturating_sub(2);
    format!("{}..", trimmed.chars().take(prefix_len).collect::<String>())
}

fn join_labels<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let joined = values
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if joined.is_empty() {
        "-".to_string()
    } else {
        joined
    }
}

fn fallback_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}

fn training_panel() -> Hsla {
    theme::bg::ELEVATED
}

fn training_border() -> Hsla {
    theme::border::SUBTLE
}

fn training_blue() -> Hsla {
    theme::accent::PRIMARY
}

fn training_green() -> Hsla {
    theme::status::SUCCESS
}

fn training_amber() -> Hsla {
    theme::status::WARNING
}

fn training_red() -> Hsla {
    theme::status::ERROR
}

#[cfg(test)]
mod tests {
    use super::paint;
    use crate::app_state::AppleAdapterTrainingPaneState;
    use crate::desktop_control::{
        DesktopControlAppleAdapterOperatorRunStatus, DesktopControlTrainingStatus,
    };
    use wgpui::{Bounds, PaintContext, Scene, TextSystem};

    #[test]
    fn apple_adapter_training_pane_paints_shell_and_runs() {
        let mut pane_state = AppleAdapterTrainingPaneState::default();
        let training = DesktopControlTrainingStatus {
            available: true,
            source: "kernel_projection".to_string(),
            control_plane_state: "ready".to_string(),
            artifact_plane_state: "ready".to_string(),
            accepted_outcome_count: 1,
            environment_versions: vec!["apple-env@1".to_string()],
            checkpoint_refs: vec!["ckpt://apple/base".to_string()],
            operator: crate::desktop_control::DesktopControlAppleAdapterOperatorStatus {
                available: true,
                workflow_state: "idle".to_string(),
                run_count: 1,
                active_run_count: 0,
                accepted_run_count: 1,
                exported_run_count: 1,
                runs: vec![DesktopControlAppleAdapterOperatorRunStatus {
                    run_id: "apple-run-1".to_string(),
                    package_name: "weather-helper".to_string(),
                    launch_state: "completed".to_string(),
                    evaluation_state: "completed".to_string(),
                    export_state: "completed".to_string(),
                    acceptance_state: "completed".to_string(),
                    average_loss_label: Some("0.024".to_string()),
                    held_out_pass_rate_bps: Some(9800),
                    exported_package_path: Some("/tmp/weather-helper.fmadapter".to_string()),
                    ..DesktopControlAppleAdapterOperatorRunStatus::default()
                }],
                ..crate::desktop_control::DesktopControlAppleAdapterOperatorStatus::default()
            },
            ..DesktopControlTrainingStatus::default()
        };
        let mut scene = Scene::new();
        let mut text_system = TextSystem::new(1.0);
        let mut paint_context = PaintContext::new(&mut scene, &mut text_system, 1.0);

        paint(
            Bounds::new(0.0, 0.0, 1240.0, 780.0),
            &mut pane_state,
            &training,
            &mut paint_context,
        );

        assert_eq!(pane_state.selected_run_id.as_deref(), Some("apple-run-1"));
    }
}
