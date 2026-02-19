use crate::app::autopilot_issues::{
    issue_priority_label as autopilot_priority_label, issue_state, issue_state_label,
    issue_type_label, sort_autopilot_issues, AutopilotIssueState, AutopilotIssuesStatus,
    IssueCounts,
};

fn render_autopilot_issues_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    _scale_factor: f32,
) {
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let modal_width = AUTOPILOT_ISSUES_MODAL_WIDTH;
            let modal_height = AUTOPILOT_ISSUES_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let label_x = modal_x + 16.0;
            let value_x = modal_x + 170.0;
            let line_height = 18.0;
            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;

            let title_run = state.text_system.layout_styled_mono(
                "Issue Tracker",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status = state.autopilot_issues.status.clone();
            let snapshot = state.autopilot_issues.snapshot.clone();

            let status_color = match &status {
                AutopilotIssuesStatus::Idle => palette.text_secondary,
                AutopilotIssuesStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                AutopilotIssuesStatus::NoWorkspace | AutopilotIssuesStatus::MissingDatabase => {
                    Hsla::new(35.0, 0.8, 0.6, 1.0)
                }
                AutopilotIssuesStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Status",
                status.label(),
                status_color,
            );

            if let Some(error) = status.error() {
                let error_text = format!("Error: {}", error);
                for line in wrap_text(&error_text, max_chars) {
                    let run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(run);
                    y += line_height;
                }
            }

            let db_path_text = snapshot
                .db_path
                .as_ref()
                .map(|path| truncate_preview(&path.display().to_string(), 54))
                .unwrap_or_else(|| "-".to_string());
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "DB path",
                &db_path_text,
                palette.text_secondary,
            );
            let db_present = if snapshot.db_exists { "Yes" } else { "No" };
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "DB present",
                db_present,
                palette.text_secondary,
            );

            let last_refresh = state
                .autopilot_issues
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Last refresh",
                &last_refresh,
                palette.text_secondary,
            );

            let counts = IssueCounts::from(&snapshot.issues);
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Open",
                &counts.open.to_string(),
                Hsla::new(120.0, 0.6, 0.5, 1.0),
            );
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "In progress",
                &counts.in_progress.to_string(),
                Hsla::new(200.0, 0.7, 0.5, 1.0),
            );
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Blocked",
                &counts.blocked.to_string(),
                Hsla::new(0.0, 0.7, 0.55, 1.0),
            );
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Done",
                &counts.done.to_string(),
                palette.text_faint,
            );
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Auto-created",
                &counts.auto_created.to_string(),
                palette.text_secondary,
            );
            draw_autopilot_issue_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Claimed",
                &counts.claimed.to_string(),
                palette.text_secondary,
            );

            y += 6.0;
            let list_header = state.text_system.layout_styled_mono(
                "Issue List",
                Point::new(label_x, y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(list_header);
            y += line_height;

            let list_bottom = modal_y + modal_height - 36.0;
            let mut shown = 0usize;

            for issue in sort_autopilot_issues(&snapshot.issues) {
                if y + line_height > list_bottom {
                    break;
                }
                let state_label = issue_state_label(issue);
                let priority_label = autopilot_priority_label(issue.priority);
                let type_label = issue_type_label(issue.issue_type);
                let mut line = format!(
                    "#{} [{}] [{}] [{}] {}",
                    issue.number, state_label, priority_label, type_label, issue.title
                );
                if issue.auto_created {
                    line.push_str(" • auto");
                }
                if issue.claimed_by.is_some() {
                    line.push_str(" • claimed");
                }
                if issue.is_blocked {
                    if let Some(reason) = &issue.blocked_reason {
                        line.push_str(&format!(" - {}", reason));
                    }
                }
                let line = truncate_preview(&line, max_chars);
                let color = issue_state_color(issue_state(issue), palette);
                let run = state.text_system.layout_styled_mono(
                    &line,
                    Point::new(label_x, y),
                    11.0,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(run);
                y += line_height;
                shown += 1;
            }

            let remaining = snapshot.issues.len().saturating_sub(shown);
            if remaining > 0 && y + line_height <= list_bottom {
                let more_text = format!("... {} more issues", remaining);
                let more_run = state.text_system.layout_styled_mono(
                    &more_text,
                    Point::new(label_x, y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(more_run);
            } else if snapshot.issues.is_empty() {
                let empty_text = if matches!(status, AutopilotIssuesStatus::MissingDatabase) {
                    "No autopilot.db found in .openagents/"
                } else if matches!(status, AutopilotIssuesStatus::NoWorkspace) {
                    "No workspace context found."
                } else {
                    "No issues found in autopilot.db"
                };
                let empty_run = state.text_system.layout_styled_mono(
                    empty_text,
                    Point::new(label_x, y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            }

            let footer = state.text_system.layout_styled_mono(
                "R refresh • Esc close",
                Point::new(label_x, modal_y + modal_height - 24.0),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer);
}

fn draw_autopilot_issue_row(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    label_x: f32,
    value_x: f32,
    y: &mut f32,
    line_height: f32,
    label: &str,
    value: &str,
    value_color: Hsla,
) {
            let label_run = state.text_system.layout_styled_mono(
                label,
                Point::new(label_x, *y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(label_run);
            let value_run = state.text_system.layout_styled_mono(
                value,
                Point::new(value_x, *y),
                12.0,
                value_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(value_run);
            *y += line_height;
}

fn issue_state_color(state: AutopilotIssueState, palette: &UiPalette) -> Hsla {
    match state {
        AutopilotIssueState::Open => Hsla::new(120.0, 0.6, 0.5, 1.0),
        AutopilotIssueState::InProgress => Hsla::new(200.0, 0.7, 0.5, 1.0),
        AutopilotIssueState::Blocked => Hsla::new(0.0, 0.7, 0.55, 1.0),
        AutopilotIssueState::Done => palette.text_faint,
    }
}
