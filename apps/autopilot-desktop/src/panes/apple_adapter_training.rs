use std::collections::BTreeSet;
use std::path::Path;

use wgpui::components::sections::{TerminalLine, TerminalStream};
use wgpui::{Bounds, Component, Hsla, InputEvent, PaintContext, Point, Quad, theme};

use crate::app_state::{
    AppleAdapterTrainingPaneInputs, AppleAdapterTrainingPaneState, AppleFmWorkbenchPaneInputs,
    AppleFmWorkbenchPaneState, PaneLoadState, RenderState,
};
use crate::desktop_control::{
    DesktopControlActionRequest, DesktopControlActionResponse,
    DesktopControlAppleAdapterOperatorRunStatus, DesktopControlTrainingStatus,
};
use crate::pane_renderer::{paint_action_button, paint_secondary_button, split_text_for_display};
use crate::pane_system::{
    apple_adapter_training_accept_button_bounds, apple_adapter_training_arm_accept_button_bounds,
    apple_adapter_training_author_input_bounds, apple_adapter_training_base_url_input_bounds,
    apple_adapter_training_description_input_bounds,
    apple_adapter_training_detail_panel_body_bounds, apple_adapter_training_export_button_bounds,
    apple_adapter_training_export_path_input_bounds, apple_adapter_training_filter_button_bounds,
    apple_adapter_training_held_out_dataset_input_bounds,
    apple_adapter_training_launch_button_bounds, apple_adapter_training_layout,
    apple_adapter_training_license_input_bounds, apple_adapter_training_log_tail_bounds,
    apple_adapter_training_open_workbench_button_bounds,
    apple_adapter_training_package_name_input_bounds,
    apple_adapter_training_preflight_summary_bounds, apple_adapter_training_run_row_bounds,
    apple_adapter_training_train_dataset_input_bounds,
};

const TRAINING_CARD_GAP: f32 = 8.0;
const TRAINING_ROW_HEIGHT: f32 = 44.0;
const TRAINING_ROW_GAP: f32 = 8.0;
const TRAINING_MAX_RUN_ROWS: usize = 9;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &mut AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    inputs: &mut AppleAdapterTrainingPaneInputs,
    paint: &mut PaintContext,
) {
    sync_pane_state(pane_state, training_status);

    let layout = apple_adapter_training_layout(content_bounds);
    let selected_run = selected_run(training_status, pane_state.selected_run_id.as_deref());
    sync_selected_run_detail_state(pane_state, inputs, selected_run);
    let header_cards = training_status_cards(training_status, selected_run);
    let card_width = ((layout.status_row.size.width - TRAINING_CARD_GAP * 3.0) / 4.0).max(120.0);
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
        "PREFLIGHT & LAUNCH",
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

    paint_preflight_panel(content_bounds, pane_state, training_status, inputs, paint);
    paint_runs_panel(content_bounds, pane_state, training_status, paint);
    paint_detail_panel(
        content_bounds,
        pane_state,
        training_status,
        selected_run,
        inputs,
        paint,
    );
}

pub fn dispatch_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pane = state
        .panes
        .iter()
        .filter(|pane| pane.kind == crate::app_state::PaneKind::AppleAdapterTraining)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pane else {
        return false;
    };

    let content_bounds = crate::pane_system::pane_content_bounds(bounds);
    let mut handled = false;
    handled |= state
        .apple_adapter_training_inputs
        .train_dataset_path
        .event(
            event,
            apple_adapter_training_train_dataset_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_adapter_training_inputs
        .held_out_dataset_path
        .event(
            event,
            apple_adapter_training_held_out_dataset_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_adapter_training_inputs
        .package_name
        .event(
            event,
            apple_adapter_training_package_name_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_adapter_training_inputs
        .author
        .event(
            event,
            apple_adapter_training_author_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_adapter_training_inputs
        .description
        .event(
            event,
            apple_adapter_training_description_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_adapter_training_inputs
        .license
        .event(
            event,
            apple_adapter_training_license_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .apple_adapter_training_inputs
        .apple_fm_base_url
        .event(
            event,
            apple_adapter_training_base_url_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    let export_handled = state
        .apple_adapter_training_inputs
        .export_path
        .event(
            event,
            apple_adapter_training_export_path_input_bounds(content_bounds),
            &mut state.event_context,
        )
        .is_handled();
    if export_handled {
        state.apple_adapter_training.pending_export_path = Some(
            state
                .apple_adapter_training_inputs
                .export_path
                .get_value()
                .trim()
                .to_string(),
        );
    }
    handled |= export_handled;
    if !handled
        && state.apple_adapter_training.selected_run_id.is_some()
        && state
            .apple_adapter_training
            .log_tail
            .event(
                event,
                apple_adapter_training_log_tail_bounds(content_bounds),
                &mut state.event_context,
            )
            .is_handled()
    {
        if let InputEvent::Scroll { dy, .. } = event {
            state
                .apple_adapter_training
                .selected_run_log_scroll_offset_px = (state
                .apple_adapter_training
                .selected_run_log_scroll_offset_px
                + *dy)
                .max(0.0);
        }
        handled = true;
    }
    handled
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
    pane_state.pending_export_path = None;
    pane_state.accept_confirmation_armed = false;
    pane_state.log_tail_run_id = None;
    pane_state.log_tail.clear();
    pane_state.selected_run_log_scroll_offset_px = 0.0;
}

fn training_status_cards<'a>(
    training_status: &'a DesktopControlTrainingStatus,
    selected_run: Option<&'a DesktopControlAppleAdapterOperatorRunStatus>,
) -> [(&'static str, String, Hsla); 4] {
    let availability = if training_status.available {
        format!(
            "{} // {}",
            training_status.source, training_status.control_plane_state
        )
    } else {
        format!("{} // unavailable", training_status.source)
    };
    let operator = if training_status.operator.available {
        format!(
            "{} // {} active",
            training_status.operator.workflow_state, training_status.operator.active_run_count
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
        |run| {
            format!(
                "{} // {}",
                compact_id(run.run_id.as_str(), 18),
                run.package_name
            )
        },
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
        |run| {
            format!(
                "run {} {}",
                compact_id(run.run_id.as_str(), 18),
                run.package_name
            )
        },
    );
    format!(
        "APPLE ADAPTER TRAINING // {} runs // {} active // {} windows // {} contributions // contributor {} // {} accepted outcomes // {head} // {selection}",
        training_status.operator.run_count,
        training_status.operator.active_run_count,
        training_status.adapter_window_count,
        training_status.contribution_count,
        training_status.contributor.assignment_state,
        training_status.accepted_outcome_count,
    )
}

fn paint_preflight_panel(
    content_bounds: Bounds,
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    inputs: &mut AppleAdapterTrainingPaneInputs,
    paint: &mut PaintContext,
) {
    let summary_bounds = apple_adapter_training_preflight_summary_bounds(content_bounds);
    let lines = [
        format!(
            "Training available: {}",
            truth_label(training_status.available)
        ),
        format!("Projection source: {}", training_status.source),
        format!("Control plane: {}", training_status.control_plane_state),
        format!("Artifact plane: {}", training_status.artifact_plane_state),
        format!(
            "Operator available: {}",
            truth_label(training_status.operator.available)
        ),
        format!(
            "Operator workflow: {}",
            training_status.operator.workflow_state
        ),
        format!(
            "Environment versions: {}",
            join_labels(
                training_status
                    .environment_versions
                    .iter()
                    .map(String::as_str)
            )
        ),
        format!(
            "Checkpoint refs: {}",
            join_labels(training_status.checkpoint_refs.iter().map(String::as_str))
        ),
        format!(
            "Adapter windows: {} total // {} active // {} promotion-ready",
            training_status.adapter_window_count,
            training_status.active_adapter_window_count,
            training_status.promotion_ready_window_count
        ),
        format!(
            "Contribution receipts: {} total // contributor state {}",
            training_status.contribution_count, training_status.contributor.assignment_state
        ),
        format!(
            "Contributor node: {}",
            training_status
                .contributor
                .local_node_id
                .as_deref()
                .unwrap_or("-")
        ),
        format!(
            "Contributor match: {} // enabled {} // backend ready {}",
            truth_label(training_status.contributor.match_eligible),
            truth_label(training_status.contributor.product_enabled),
            truth_label(training_status.contributor.backend_ready)
        ),
        format!(
            "Contributor detail: {}",
            training_status
                .contributor
                .readiness_detail
                .as_deref()
                .unwrap_or("-")
        ),
    ];
    let chunk_len = section_chunk_len(summary_bounds);
    let mut y = summary_bounds.origin.y + 6.0;
    for line in lines {
        for wrapped in split_text_for_display(line.as_str(), chunk_len) {
            paint.scene.draw_text(paint.text.layout_mono(
                wrapped.as_str(),
                Point::new(summary_bounds.origin.x, y),
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
                Point::new(summary_bounds.origin.x, y),
                10.0,
                training_red(),
            ));
            y += 16.0;
        }
    }

    let train_bounds = apple_adapter_training_train_dataset_input_bounds(content_bounds);
    let held_out_bounds = apple_adapter_training_held_out_dataset_input_bounds(content_bounds);
    let package_bounds = apple_adapter_training_package_name_input_bounds(content_bounds);
    let author_bounds = apple_adapter_training_author_input_bounds(content_bounds);
    let description_bounds = apple_adapter_training_description_input_bounds(content_bounds);
    let license_bounds = apple_adapter_training_license_input_bounds(content_bounds);
    let base_url_bounds = apple_adapter_training_base_url_input_bounds(content_bounds);
    let launch_bounds = apple_adapter_training_launch_button_bounds(content_bounds);

    inputs
        .train_dataset_path
        .set_max_width(train_bounds.size.width.max(200.0));
    inputs
        .held_out_dataset_path
        .set_max_width(held_out_bounds.size.width.max(200.0));
    inputs
        .package_name
        .set_max_width(package_bounds.size.width.max(200.0));
    inputs
        .author
        .set_max_width(author_bounds.size.width.max(160.0));
    inputs
        .description
        .set_max_width(description_bounds.size.width.max(200.0));
    inputs
        .license
        .set_max_width(license_bounds.size.width.max(160.0));
    inputs
        .apple_fm_base_url
        .set_max_width(base_url_bounds.size.width.max(200.0));

    inputs.train_dataset_path.paint(train_bounds, paint);
    inputs.held_out_dataset_path.paint(held_out_bounds, paint);
    inputs.package_name.paint(package_bounds, paint);
    inputs.author.paint(author_bounds, paint);
    inputs.description.paint(description_bounds, paint);
    inputs.license.paint(license_bounds, paint);
    inputs.apple_fm_base_url.paint(base_url_bounds, paint);
    paint_action_button(launch_bounds, "Launch Apple adapter run", paint);

    paint_input_label(paint, train_bounds, "Train dataset path");
    paint_input_label(paint, held_out_bounds, "Held-out dataset path");
    paint_input_label(paint, package_bounds, "Package name");
    paint_input_label(paint, author_bounds, "Author");
    paint_input_label(paint, description_bounds, "Description");
    paint_input_label(paint, license_bounds, "License");
    paint_input_label(paint, base_url_bounds, "Apple FM base URL");
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
    content_bounds: Bounds,
    pane_state: &mut AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
    selected_run: Option<&DesktopControlAppleAdapterOperatorRunStatus>,
    inputs: &mut AppleAdapterTrainingPaneInputs,
    paint: &mut PaintContext,
) {
    let bounds = apple_adapter_training_detail_panel_body_bounds(content_bounds);
    let chunk_len = section_chunk_len(bounds);
    let badge_gap = 8.0;
    let badge_width = ((bounds.size.width - badge_gap * 3.0) / 4.0).max(82.0);
    let badge_y = bounds.origin.y + 4.0;
    if let Some(run) = selected_run {
        for (index, (label, state)) in [
            ("LAUNCH", run.launch_state.as_str()),
            ("EVAL", run.evaluation_state.as_str()),
            ("EXPORT", run.export_state.as_str()),
            ("ACCEPT", run.acceptance_state.as_str()),
        ]
        .iter()
        .enumerate()
        {
            let x = bounds.origin.x + index as f32 * (badge_width + badge_gap);
            let width = if index == 3 {
                (bounds.max_x() - x).max(82.0)
            } else {
                badge_width
            };
            paint_stage_badge(Bounds::new(x, badge_y, width, 26.0), label, state, paint);
        }
    }

    let log_bounds = apple_adapter_training_log_tail_bounds(content_bounds);
    let export_bounds = apple_adapter_training_export_path_input_bounds(content_bounds);
    let export_button_bounds = apple_adapter_training_export_button_bounds(content_bounds);
    let open_workbench_bounds = apple_adapter_training_open_workbench_button_bounds(content_bounds);
    let arm_accept_bounds = apple_adapter_training_arm_accept_button_bounds(content_bounds);
    let accept_bounds = apple_adapter_training_accept_button_bounds(content_bounds);
    let summary_max_y = export_bounds.origin.y - 10.0;
    let mut y = badge_y + 38.0;
    let detail_lines = if let Some(run) = selected_run {
        detail_lines_for_run(run, training_status)
    } else {
        vec![
            "No Apple adapter operator run is selected yet.".to_string(),
            format!("Current filter: {}", pane_state.stage_filter.label()),
            format!(
                "Visible operator runs: {}",
                filtered_runs(training_status, pane_state.stage_filter).len()
            ),
            format!(
                "Contributor state: {}",
                training_status.contributor.assignment_state
            ),
        ]
    };

    let mut truncated = false;
    for line in detail_lines {
        for wrapped in split_text_for_display(line.as_str(), chunk_len) {
            if y + 16.0 > summary_max_y {
                truncated = true;
                break;
            }
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
        if truncated {
            break;
        }
    }
    if truncated {
        paint.scene.draw_text(paint.text.layout_mono(
            "...",
            Point::new(bounds.origin.x, summary_max_y - 14.0),
            10.0,
            theme::text::MUTED,
        ));
    }

    if let Some(run) = selected_run {
        let export_status = export_gate_summary(run);
        let acceptance_status = acceptance_gate_summary(run, pane_state.accept_confirmation_armed);
        paint_input_label(paint, export_bounds, "Export path");
        inputs
            .export_path
            .set_max_width(export_bounds.size.width.max(180.0));
        inputs.export_path.paint(export_bounds, paint);
        paint_action_button(export_button_bounds, export_button_label(run), paint);
        paint_action_button(
            open_workbench_bounds,
            if can_open_workbench(run) {
                "Open Apple FM"
            } else {
                "Workbench blocked"
            },
            paint,
        );
        paint_secondary_button(
            arm_accept_bounds,
            if pane_state.accept_confirmation_armed {
                "Acceptance armed"
            } else {
                "Arm accept"
            },
            paint,
        );
        paint_action_button(
            accept_bounds,
            if can_accept_run(run) {
                "Accept into authority"
            } else {
                "Accept blocked"
            },
            paint,
        );
        paint.scene.draw_text(paint.text.layout_mono(
            export_status.as_str(),
            Point::new(export_bounds.origin.x, export_button_bounds.max_y() + 8.0),
            10.0,
            if run.staged_package_path.is_some() {
                training_green()
            } else {
                training_red()
            },
        ));
        paint.scene.draw_text(paint.text.layout_mono(
            acceptance_status.as_str(),
            Point::new(export_bounds.origin.x, export_button_bounds.max_y() + 24.0),
            10.0,
            if can_accept_run(run) {
                training_green()
            } else {
                training_red()
            },
        ));
        pane_state.log_tail.paint(log_bounds, paint);
    } else {
        paint.scene.draw_text(paint.text.layout_mono(
            "Select a run to export packages, inspect logs, and accept authority outcomes.",
            Point::new(bounds.origin.x, export_bounds.origin.y + 8.0),
            10.0,
            theme::text::MUTED,
        ));
    }
}

fn paint_input_label(paint: &mut PaintContext, bounds: Bounds, label: &str) {
    paint.scene.draw_text(paint.text.layout(
        label,
        Point::new(bounds.origin.x, bounds.origin.y - 12.0),
        10.0,
        theme::text::MUTED,
    ));
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
    let title = format!(
        "{} // {}",
        compact_id(run.run_id.as_str(), 18),
        run.package_name
    );
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

fn sync_selected_run_detail_state(
    pane_state: &mut AppleAdapterTrainingPaneState,
    inputs: &mut AppleAdapterTrainingPaneInputs,
    selected_run: Option<&DesktopControlAppleAdapterOperatorRunStatus>,
) {
    let Some(run) = selected_run else {
        pane_state.pending_export_path = None;
        pane_state.accept_confirmation_armed = false;
        pane_state.log_tail_run_id = None;
        pane_state.log_tail.clear();
        pane_state.selected_run_log_scroll_offset_px = 0.0;
        return;
    };

    if pane_state.log_tail_run_id.as_deref() != Some(run.run_id.as_str()) {
        pane_state.log_tail.clear();
        pane_state.log_tail_run_id = Some(run.run_id.clone());
        pane_state.selected_run_log_scroll_offset_px = 0.0;
    }
    for (index, line) in run.log_lines.iter().enumerate() {
        pane_state.log_tail.push_line(
            TerminalLine::new(training_log_stream(line.as_str()), line.clone())
                .with_key(format!("{}:{index}", run.run_id)),
        );
    }

    if pane_state.pending_export_path.is_none() {
        let suggestion = suggested_export_path(run);
        inputs.export_path.set_value(suggestion.clone());
        pane_state.pending_export_path = Some(suggestion);
    }
}

pub(crate) fn detail_lines_for_run(
    run: &DesktopControlAppleAdapterOperatorRunStatus,
    training_status: &DesktopControlTrainingStatus,
) -> Vec<String> {
    let mut lines = vec![
        format!("Run id: {}", run.run_id),
        format!(
            "Package: {} // author {} // license {}",
            run.package_name,
            fallback_dash(run.author.as_str()),
            fallback_dash(run.license.as_str())
        ),
        format!("Description: {}", fallback_dash(run.description.as_str())),
        format!(
            "Train / held-out: {} // {}",
            run.train_dataset_path, run.held_out_dataset_path
        ),
        format!(
            "Run directory: {}",
            fallback_dash(run.run_directory.as_str())
        ),
        format!(
            "Staged / exported: {} // {}",
            run.staged_package_path.as_deref().unwrap_or("-"),
            run.exported_package_path.as_deref().unwrap_or("-")
        ),
        format!(
            "Steps: {}/{} // average loss {}",
            run.completed_step_count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.expected_step_count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            run.average_loss_label.as_deref().unwrap_or("-")
        ),
        format!(
            "Held-out pass / score: {} / {}",
            run.held_out_pass_rate_bps
                .map(format_bps)
                .unwrap_or_else(|| "-".to_string()),
            run.held_out_average_score_bps
                .map(format_bps)
                .unwrap_or_else(|| "-".to_string()),
        ),
        format!(
            "Runtime smoke / digest: {} / {}",
            run.runtime_smoke_passed
                .map(truth_label)
                .unwrap_or("unknown"),
            run.runtime_smoke_digest.as_deref().unwrap_or("-")
        ),
        format!(
            "Package digest / adapter id: {} / {}",
            run.package_digest.as_deref().unwrap_or("-"),
            run.adapter_identifier.as_deref().unwrap_or("-")
        ),
        format!(
            "Authority refs: training {} // held-out {} // runtime {}",
            run.authority.training_run_id.as_deref().unwrap_or("-"),
            run.authority.held_out_eval_run_id.as_deref().unwrap_or("-"),
            run.authority
                .runtime_validation_eval_run_id
                .as_deref()
                .unwrap_or("-")
        ),
        format!(
            "Kernel refs: env {} // bench {} // package {} // accepted {}",
            run.authority.core_environment_ref.as_deref().unwrap_or("-"),
            run.authority
                .benchmark_environment_ref
                .as_deref()
                .unwrap_or("-"),
            run.authority
                .benchmark_package_ref
                .as_deref()
                .unwrap_or("-"),
            run.authority.accepted_outcome_id.as_deref().unwrap_or("-")
        ),
        format!("Last action: {}", run.last_action.as_deref().unwrap_or("-")),
        format!("Last error: {}", run.last_error.as_deref().unwrap_or("-")),
    ];
    if let Some(training_run_id) = run.authority.training_run_id.as_deref() {
        let matching_windows = training_status
            .windows
            .iter()
            .filter(|window| window.training_run_id == training_run_id)
            .collect::<Vec<_>>();
        if matching_windows.is_empty() {
            lines.push(format!(
                "Decentralized windows: no authority windows projected yet for {}",
                training_run_id
            ));
        } else {
            for window in matching_windows.iter().take(2) {
                lines.push(format!(
                    "Window {} // stage {} // status {} // uploaded {}/{} // accepted {} // promotion {} // accepted outcome {}",
                    window.window_id,
                    fallback_dash(window.stage_id.as_str()),
                    window.status,
                    window.uploaded_contributions,
                    window.total_contributions,
                    window.accepted_contributions,
                    if window.promotion_ready {
                        "ready"
                    } else {
                        "blocked"
                    },
                    window.accepted_outcome_id.as_deref().unwrap_or("-")
                ));
                lines.push(format!(
                    "Window scores {} / {} // runtime smoke {} // gates {} // holds {}",
                    window
                        .held_out_average_score_bps
                        .map(format_bps)
                        .unwrap_or_else(|| "-".to_string()),
                    window
                        .benchmark_pass_rate_bps
                        .map(format_bps)
                        .unwrap_or_else(|| "-".to_string()),
                    window
                        .runtime_smoke_passed
                        .map(truth_label)
                        .unwrap_or("unknown"),
                    join_labels(window.gate_reason_codes.iter().map(String::as_str)),
                    join_labels(window.hold_reason_codes.iter().map(String::as_str))
                ));
            }
            let matching_contributions = training_status
                .contributions
                .iter()
                .filter(|contribution| contribution.training_run_id == training_run_id)
                .collect::<Vec<_>>();
            if matching_contributions.is_empty() {
                lines.push("Contributions: no contributor receipts projected yet".to_string());
            } else {
                for contribution in matching_contributions.iter().take(3) {
                    lines.push(format!(
                        "Contribution {} // node {} // disposition {} // upload {} // payout {}",
                        compact_id(contribution.contribution_id.as_str(), 18),
                        compact_id(contribution.contributor_node_id.as_str(), 18),
                        contribution.validator_disposition,
                        contribution.upload_state,
                        contribution.payout_state
                    ));
                }
            }
        }
    } else {
        lines.push("Decentralized windows: pending kernel training-run linkage".to_string());
    }
    lines.push(format!(
        "Contributor state: {} // eligible {} // local assignments {} // settlement ready {}",
        training_status.contributor.assignment_state,
        truth_label(training_status.contributor.match_eligible),
        training_status.contributor.local_assignment_count,
        training_status.contributor.local_settlement_ready_count
    ));
    lines.push(format!(
        "Contributor detail: {}",
        training_status
            .contributor
            .readiness_detail
            .as_deref()
            .unwrap_or("-")
    ));
    lines
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AppleAdapterTrainingWorkbenchHandoff {
    pub run_id: String,
    pub package_path: String,
    pub adapter_identifier: Option<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AppleAdapterTrainingLaunchForm {
    pub train_dataset_path: String,
    pub held_out_dataset_path: String,
    pub package_name: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub apple_fm_base_url: String,
}

pub(crate) fn build_export_request(
    pane_state: &AppleAdapterTrainingPaneState,
    inputs: &AppleAdapterTrainingPaneInputs,
    training_status: &DesktopControlTrainingStatus,
) -> Result<(String, DesktopControlActionRequest), String> {
    let run = selected_run_for_action(pane_state, training_status)?;
    let export_path = trim_required(inputs.export_path.get_value(), "export path")?;
    if let Some(reason) = export_gate_reason(run) {
        return Err(reason);
    }
    Ok((
        export_path.clone(),
        DesktopControlActionRequest::ExportAppleAdapterTraining {
            run_id: run.run_id.clone(),
            export_path,
        },
    ))
}

pub(crate) fn build_accept_request(
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
) -> Result<DesktopControlActionRequest, String> {
    let run = selected_run_for_action(pane_state, training_status)?;
    if !pane_state.accept_confirmation_armed {
        return Err("Arm acceptance before submitting to kernel authority".to_string());
    }
    if let Some(reason) = acceptance_gate_reason(run) {
        return Err(reason);
    }
    Ok(DesktopControlActionRequest::AcceptAppleAdapterTraining {
        run_id: run.run_id.clone(),
    })
}

pub(crate) fn build_workbench_handoff(
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &DesktopControlTrainingStatus,
) -> Result<AppleAdapterTrainingWorkbenchHandoff, String> {
    let run = selected_run_for_action(pane_state, training_status)?;
    let package_path = workbench_package_path(run)?;
    Ok(AppleAdapterTrainingWorkbenchHandoff {
        run_id: run.run_id.clone(),
        package_path: package_path.clone(),
        adapter_identifier: run.adapter_identifier.clone(),
        summary: format!(
            "run {} // adapter {} // pkg {}",
            compact_id(run.run_id.as_str(), 18),
            run.adapter_identifier
                .as_deref()
                .map(|value| compact_id(value, 18))
                .unwrap_or_else(|| "pending".to_string()),
            compact_id(package_path.as_str(), 34)
        ),
    })
}

pub(crate) fn apply_workbench_handoff(
    workbench_state: &mut AppleFmWorkbenchPaneState,
    workbench_inputs: &mut AppleFmWorkbenchPaneInputs,
    handoff: &AppleAdapterTrainingWorkbenchHandoff,
) {
    workbench_inputs
        .adapter_package_path
        .set_value(handoff.package_path.clone());
    workbench_inputs
        .adapter_id
        .set_value(handoff.adapter_identifier.clone().unwrap_or_default());
    workbench_state.handoff_source_run_id = Some(handoff.run_id.clone());
    workbench_state.handoff_adapter_identifier = handoff.adapter_identifier.clone();
    workbench_state.handoff_package_path = Some(handoff.package_path.clone());
    workbench_state.handoff_summary = Some(handoff.summary.clone());
}

pub(crate) fn validate_launch_form(
    inputs: &AppleAdapterTrainingPaneInputs,
) -> Result<AppleAdapterTrainingLaunchForm, String> {
    let train_dataset_path =
        trim_required(inputs.train_dataset_path.get_value(), "train dataset path")?;
    let held_out_dataset_path = trim_required(
        inputs.held_out_dataset_path.get_value(),
        "held-out dataset path",
    )?;
    let package_name = trim_required(inputs.package_name.get_value(), "package name")?;
    let apple_fm_base_url =
        trim_required(inputs.apple_fm_base_url.get_value(), "Apple FM base URL")?;

    if !Path::new(train_dataset_path.as_str()).exists() {
        return Err(format!(
            "Train dataset path does not exist: {}",
            train_dataset_path
        ));
    }
    if !Path::new(held_out_dataset_path.as_str()).exists() {
        return Err(format!(
            "Held-out dataset path does not exist: {}",
            held_out_dataset_path
        ));
    }

    Ok(AppleAdapterTrainingLaunchForm {
        train_dataset_path,
        held_out_dataset_path,
        package_name,
        author: inputs.author.get_value().trim().to_string(),
        description: inputs.description.get_value().trim().to_string(),
        license: inputs.license.get_value().trim().to_string(),
        apple_fm_base_url,
    })
}

pub(crate) fn apply_launch_response(
    pane_state: &mut AppleAdapterTrainingPaneState,
    previous_run_ids: &BTreeSet<String>,
    response: &DesktopControlActionResponse,
    training_status: &DesktopControlTrainingStatus,
    package_name: &str,
) {
    if response.success {
        pane_state.selected_run_id =
            pick_new_or_matching_run_id(previous_run_ids, training_status, package_name);
        pane_state.last_action = Some(response.message.clone());
        pane_state.last_error = None;
        pane_state.pending_export_path = None;
        pane_state.accept_confirmation_armed = false;
        pane_state.log_tail_run_id = None;
        pane_state.log_tail.clear();
        pane_state.selected_run_log_scroll_offset_px = 0.0;
    } else {
        pane_state.last_error = Some(response.message.clone());
        pane_state.last_action = Some("Apple adapter training launch failed".to_string());
    }
}

pub(crate) fn apply_run_action_response(
    pane_state: &mut AppleAdapterTrainingPaneState,
    response: &DesktopControlActionResponse,
    next_export_path: Option<String>,
    clear_accept_arm: bool,
) {
    if response.success {
        pane_state.last_action = Some(response.message.clone());
        pane_state.last_error = None;
        if let Some(export_path) = next_export_path {
            pane_state.pending_export_path = Some(export_path);
        }
        if clear_accept_arm {
            pane_state.accept_confirmation_armed = false;
        }
        pane_state.log_tail_run_id = None;
        pane_state.log_tail.clear();
    } else {
        pane_state.last_error = Some(response.message.clone());
        pane_state.last_action = Some("Apple adapter training action failed".to_string());
    }
}

fn pick_new_or_matching_run_id(
    previous_run_ids: &BTreeSet<String>,
    training_status: &DesktopControlTrainingStatus,
    package_name: &str,
) -> Option<String> {
    training_status
        .operator
        .runs
        .iter()
        .find(|run| !previous_run_ids.contains(run.run_id.as_str()))
        .or_else(|| {
            training_status
                .operator
                .runs
                .iter()
                .find(|run| run.package_name == package_name)
        })
        .map(|run| run.run_id.clone())
        .or_else(|| {
            training_status
                .operator
                .runs
                .first()
                .map(|run| run.run_id.clone())
        })
}

fn trim_required(value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("Missing {label}"))
    } else {
        Ok(trimmed.to_string())
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

fn selected_run_for_action<'a>(
    pane_state: &AppleAdapterTrainingPaneState,
    training_status: &'a DesktopControlTrainingStatus,
) -> Result<&'a DesktopControlAppleAdapterOperatorRunStatus, String> {
    selected_run(training_status, pane_state.selected_run_id.as_deref())
        .ok_or_else(|| "Select an Apple adapter run first".to_string())
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

fn paint_stage_badge(bounds: Bounds, label: &str, state: &str, paint: &mut PaintContext) {
    let accent = stage_accent(state);
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(accent.with_alpha(0.10))
            .with_border(accent.with_alpha(0.82), 1.0)
            .with_corner_radius(4.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 5.0),
        9.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        state,
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 14.0),
        10.0,
        accent,
    ));
}

fn stage_accent(state: &str) -> Hsla {
    match state {
        "completed" => training_green(),
        "running" => training_blue(),
        "failed" | "interrupted" => training_red(),
        _ => training_amber(),
    }
}

fn suggested_export_path(run: &DesktopControlAppleAdapterOperatorRunStatus) -> String {
    run.exported_package_path
        .clone()
        .or_else(|| {
            let package = run.package_name.trim();
            if package.is_empty() {
                None
            } else {
                Some(format!("/tmp/{package}.fmadapter"))
            }
        })
        .unwrap_or_else(|| "/tmp/apple-adapter.fmadapter".to_string())
}

fn export_button_label(run: &DesktopControlAppleAdapterOperatorRunStatus) -> &'static str {
    if run.exported_package_path.is_some() || run.export_state == "completed" {
        "Re-export package"
    } else {
        "Export package"
    }
}

fn export_gate_reason(run: &DesktopControlAppleAdapterOperatorRunStatus) -> Option<String> {
    if run.staged_package_path.is_none() {
        Some("Export blocked until a staged Apple adapter package exists".to_string())
    } else {
        None
    }
}

fn export_gate_summary(run: &DesktopControlAppleAdapterOperatorRunStatus) -> String {
    export_gate_reason(run).unwrap_or_else(|| {
        format!(
            "Export ready from {}",
            run.staged_package_path.as_deref().unwrap_or("-")
        )
    })
}

pub(crate) fn acceptance_gate_reason(
    run: &DesktopControlAppleAdapterOperatorRunStatus,
) -> Option<String> {
    if run.acceptance_state == "completed" || run.authority.accepted_outcome_id.is_some() {
        return Some("Already accepted into kernel authority".to_string());
    }
    if run.export_state != "completed" {
        return Some("Acceptance blocked until export completes".to_string());
    }
    if run.evaluation_state != "completed" {
        return Some("Acceptance blocked until held-out evaluation completes".to_string());
    }
    None
}

fn acceptance_gate_summary(
    run: &DesktopControlAppleAdapterOperatorRunStatus,
    accept_confirmation_armed: bool,
) -> String {
    if let Some(reason) = acceptance_gate_reason(run) {
        reason
    } else if accept_confirmation_armed {
        "Acceptance armed. Submit to publish the accepted outcome.".to_string()
    } else {
        "Acceptance ready. Arm the action, then submit to kernel authority.".to_string()
    }
}

fn can_accept_run(run: &DesktopControlAppleAdapterOperatorRunStatus) -> bool {
    acceptance_gate_reason(run).is_none()
}

fn can_open_workbench(run: &DesktopControlAppleAdapterOperatorRunStatus) -> bool {
    workbench_package_path(run).is_ok()
}

fn workbench_package_path(
    run: &DesktopControlAppleAdapterOperatorRunStatus,
) -> Result<String, String> {
    run.exported_package_path
        .clone()
        .or_else(|| run.staged_package_path.clone())
        .ok_or_else(|| {
            "Apple FM workbench handoff is blocked until a staged or exported adapter exists"
                .to_string()
        })
}

fn training_log_stream(line: &str) -> TerminalStream {
    let lowered = line.to_ascii_lowercase();
    if lowered.contains("error") || lowered.contains("failed") || lowered.contains("blocked") {
        TerminalStream::Stderr
    } else {
        TerminalStream::Stdout
    }
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
    use super::{
        acceptance_gate_reason, apply_launch_response, build_accept_request, build_export_request,
        build_workbench_handoff, detail_lines_for_run, paint, validate_launch_form,
    };
    use crate::app_state::{
        AppleAdapterTrainingPaneInputs, AppleAdapterTrainingPaneState, AppleFmWorkbenchPaneInputs,
    };
    use crate::desktop_control::{
        DesktopControlActionResponse, DesktopControlAppleAdapterOperatorRunStatus,
        DesktopControlTrainingStatus,
    };
    use wgpui::{Bounds, PaintContext, Scene, TextSystem};

    #[test]
    fn apple_adapter_training_pane_paints_shell_and_runs() {
        let mut pane_state = AppleAdapterTrainingPaneState::default();
        let mut inputs = AppleAdapterTrainingPaneInputs::default();
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
            &mut inputs,
            &mut paint_context,
        );

        assert_eq!(pane_state.selected_run_id.as_deref(), Some("apple-run-1"));
    }

    #[test]
    fn validate_launch_form_requires_existing_paths_and_name() {
        let mut inputs = AppleAdapterTrainingPaneInputs::default();
        assert!(validate_launch_form(&inputs).is_err());

        let train_path = std::env::temp_dir().join("apple-train.jsonl");
        let held_out_path = std::env::temp_dir().join("apple-held-out.jsonl");
        std::fs::write(&train_path, "{}\n").expect("write train fixture");
        std::fs::write(&held_out_path, "{}\n").expect("write held-out fixture");

        inputs
            .train_dataset_path
            .set_value(train_path.display().to_string());
        inputs
            .held_out_dataset_path
            .set_value(held_out_path.display().to_string());
        inputs.package_name.set_value("weather-helper".to_string());

        let form = validate_launch_form(&inputs).expect("launch form should validate");
        assert_eq!(form.package_name, "weather-helper");
        assert!(form.apple_fm_base_url.contains("11435"));

        let _ = std::fs::remove_file(train_path);
        let _ = std::fs::remove_file(held_out_path);
    }

    #[test]
    fn apply_launch_response_selects_new_run_after_success() {
        let mut pane_state = AppleAdapterTrainingPaneState::default();
        let previous_run_ids = std::iter::once("apple-run-1".to_string()).collect();
        let training = DesktopControlTrainingStatus {
            operator: crate::desktop_control::DesktopControlAppleAdapterOperatorStatus {
                available: true,
                workflow_state: "running".to_string(),
                run_count: 2,
                active_run_count: 1,
                runs: vec![
                    DesktopControlAppleAdapterOperatorRunStatus {
                        run_id: "apple-run-2".to_string(),
                        package_name: "weather-helper".to_string(),
                        launch_state: "running".to_string(),
                        ..DesktopControlAppleAdapterOperatorRunStatus::default()
                    },
                    DesktopControlAppleAdapterOperatorRunStatus {
                        run_id: "apple-run-1".to_string(),
                        package_name: "older".to_string(),
                        launch_state: "completed".to_string(),
                        ..DesktopControlAppleAdapterOperatorRunStatus::default()
                    },
                ],
                ..crate::desktop_control::DesktopControlAppleAdapterOperatorStatus::default()
            },
            ..DesktopControlTrainingStatus::default()
        };

        apply_launch_response(
            &mut pane_state,
            &previous_run_ids,
            &DesktopControlActionResponse {
                success: true,
                message: "Completed Apple adapter operator launch for apple-run-2".to_string(),
                payload: None,
                snapshot_revision: None,
                state_signature: None,
            },
            &training,
            "weather-helper",
        );

        assert_eq!(pane_state.selected_run_id.as_deref(), Some("apple-run-2"));
        assert_eq!(
            pane_state.last_action.as_deref(),
            Some("Completed Apple adapter operator launch for apple-run-2")
        );
        assert!(pane_state.last_error.is_none());
    }

    #[test]
    fn build_export_request_uses_selected_run_and_export_path() {
        let mut pane_state = AppleAdapterTrainingPaneState::default();
        pane_state.selected_run_id = Some("apple-run-1".to_string());
        let mut inputs = AppleAdapterTrainingPaneInputs::default();
        inputs
            .export_path
            .set_value("/tmp/weather-helper.fmadapter".to_string());
        let training = DesktopControlTrainingStatus {
            operator: crate::desktop_control::DesktopControlAppleAdapterOperatorStatus {
                runs: vec![DesktopControlAppleAdapterOperatorRunStatus {
                    run_id: "apple-run-1".to_string(),
                    staged_package_path: Some("/tmp/staged/weather-helper".to_string()),
                    ..DesktopControlAppleAdapterOperatorRunStatus::default()
                }],
                ..crate::desktop_control::DesktopControlAppleAdapterOperatorStatus::default()
            },
            ..DesktopControlTrainingStatus::default()
        };

        let (_, request) =
            build_export_request(&pane_state, &inputs, &training).expect("export request");
        assert_eq!(
            request,
            crate::desktop_control::DesktopControlActionRequest::ExportAppleAdapterTraining {
                run_id: "apple-run-1".to_string(),
                export_path: "/tmp/weather-helper.fmadapter".to_string(),
            }
        );
    }

    #[test]
    fn build_accept_request_requires_arm_and_completed_eval_export() {
        let mut pane_state = AppleAdapterTrainingPaneState::default();
        pane_state.selected_run_id = Some("apple-run-1".to_string());
        let training = DesktopControlTrainingStatus {
            operator: crate::desktop_control::DesktopControlAppleAdapterOperatorStatus {
                runs: vec![DesktopControlAppleAdapterOperatorRunStatus {
                    run_id: "apple-run-1".to_string(),
                    evaluation_state: "completed".to_string(),
                    export_state: "completed".to_string(),
                    ..DesktopControlAppleAdapterOperatorRunStatus::default()
                }],
                ..crate::desktop_control::DesktopControlAppleAdapterOperatorStatus::default()
            },
            ..DesktopControlTrainingStatus::default()
        };

        assert!(build_accept_request(&pane_state, &training).is_err());
        pane_state.accept_confirmation_armed = true;
        let request = build_accept_request(&pane_state, &training).expect("accept request");
        assert_eq!(
            request,
            crate::desktop_control::DesktopControlActionRequest::AcceptAppleAdapterTraining {
                run_id: "apple-run-1".to_string(),
            }
        );
    }

    #[test]
    fn detail_lines_include_stage_authority_and_digest_state() {
        let run = DesktopControlAppleAdapterOperatorRunStatus {
            run_id: "apple-run-7".to_string(),
            package_name: "weather-helper".to_string(),
            package_digest: Some("sha256:abc".to_string()),
            adapter_identifier: Some("adapter.weather.helper".to_string()),
            authority: crate::desktop_control::DesktopControlAppleAdapterOperatorAuthorityStatus {
                training_run_id: Some("train-1".to_string()),
                accepted_outcome_id: Some("accepted-1".to_string()),
                ..crate::desktop_control::DesktopControlAppleAdapterOperatorAuthorityStatus::default(
                )
            },
            ..DesktopControlAppleAdapterOperatorRunStatus::default()
        };
        let training = DesktopControlTrainingStatus {
            windows: vec![
                crate::desktop_control::DesktopControlAdapterTrainingWindowStatus {
                    window_id: "window-1".to_string(),
                    training_run_id: "train-1".to_string(),
                    stage_id: "stage-a".to_string(),
                    status: "active".to_string(),
                    total_contributions: 2,
                    accepted_contributions: 1,
                    uploaded_contributions: 1,
                    promotion_ready: false,
                    accepted_outcome_id: Some("accepted-1".to_string()),
                    ..crate::desktop_control::DesktopControlAdapterTrainingWindowStatus::default()
                },
            ],
            contributor: crate::desktop_control::DesktopControlTrainingContributorStatus {
                assignment_state: "awaiting_assignment".to_string(),
                match_eligible: true,
                ..crate::desktop_control::DesktopControlTrainingContributorStatus::default()
            },
            ..DesktopControlTrainingStatus::default()
        };

        let lines = detail_lines_for_run(&run, &training);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Authority refs: training train-1"))
        );
        assert!(lines.iter().any(|line| {
            line.contains("Package digest / adapter id: sha256:abc / adapter.weather.helper")
        }));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("accepted accepted-1"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("Window window-1 // stage stage-a // status active"))
        );
    }

    #[test]
    fn acceptance_gate_reason_blocks_until_eval_and_export_complete() {
        let run = DesktopControlAppleAdapterOperatorRunStatus {
            run_id: "apple-run-8".to_string(),
            evaluation_state: "running".to_string(),
            export_state: "pending".to_string(),
            ..DesktopControlAppleAdapterOperatorRunStatus::default()
        };

        assert_eq!(
            acceptance_gate_reason(&run).as_deref(),
            Some("Acceptance blocked until export completes")
        );
    }

    #[test]
    fn build_workbench_handoff_prefers_exported_path_and_adapter_id() {
        let mut pane_state = AppleAdapterTrainingPaneState::default();
        pane_state.selected_run_id = Some("apple-run-9".to_string());
        let training = DesktopControlTrainingStatus {
            operator: crate::desktop_control::DesktopControlAppleAdapterOperatorStatus {
                runs: vec![DesktopControlAppleAdapterOperatorRunStatus {
                    run_id: "apple-run-9".to_string(),
                    staged_package_path: Some("/tmp/staged/weather-helper".to_string()),
                    exported_package_path: Some("/tmp/export/weather-helper.fmadapter".to_string()),
                    adapter_identifier: Some("adapter.weather.helper".to_string()),
                    ..DesktopControlAppleAdapterOperatorRunStatus::default()
                }],
                ..crate::desktop_control::DesktopControlAppleAdapterOperatorStatus::default()
            },
            ..DesktopControlTrainingStatus::default()
        };

        let handoff = build_workbench_handoff(&pane_state, &training).expect("handoff");
        assert_eq!(handoff.run_id, "apple-run-9");
        assert_eq!(
            handoff.package_path,
            "/tmp/export/weather-helper.fmadapter".to_string()
        );
        assert_eq!(
            handoff.adapter_identifier.as_deref(),
            Some("adapter.weather.helper")
        );
    }

    #[test]
    fn apply_workbench_handoff_populates_inputs_and_context() {
        let handoff = super::AppleAdapterTrainingWorkbenchHandoff {
            run_id: "apple-run-10".to_string(),
            package_path: "/tmp/export/final.fmadapter".to_string(),
            adapter_identifier: Some("adapter.final".to_string()),
            summary: "Training handoff".to_string(),
        };
        let mut workbench = crate::app_state::AppleFmWorkbenchPaneState::default();
        let mut inputs = AppleFmWorkbenchPaneInputs::default();

        super::apply_workbench_handoff(&mut workbench, &mut inputs, &handoff);

        assert_eq!(
            inputs.adapter_package_path.get_value(),
            "/tmp/export/final.fmadapter"
        );
        assert_eq!(inputs.adapter_id.get_value(), "adapter.final");
        assert_eq!(
            workbench.handoff_source_run_id.as_deref(),
            Some("apple-run-10")
        );
        assert_eq!(
            workbench.handoff_package_path.as_deref(),
            Some("/tmp/export/final.fmadapter")
        );
    }
}
