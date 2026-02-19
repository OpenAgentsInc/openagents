use crate::app::rlm::RlmStatus;

fn render_rlm_modal(
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

            let modal_width = RLM_MODAL_WIDTH;
            let modal_height = RLM_MODAL_HEIGHT;
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
                "RLM Runs",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status = state.rlm.status.clone();
            let snapshot = state.rlm.snapshot.clone();

            let status_color = match &status {
                RlmStatus::Idle => palette.text_secondary,
                RlmStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                RlmStatus::MissingDatabase | RlmStatus::NoHomeDir => {
                    Hsla::new(35.0, 0.8, 0.6, 1.0)
                }
                RlmStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            draw_rlm_row(
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
            draw_rlm_row(
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
            draw_rlm_row(
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
                .rlm
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());
            draw_rlm_row(
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

            let mut running = 0;
            let mut completed = 0;
            let mut failed = 0;
            let mut total_budget = 0;
            let mut total_cost = 0;
            let mut total_duration = 0;
            for run in &snapshot.runs {
                match run.status.as_str() {
                    "running" => running += 1,
                    "completed" => completed += 1,
                    "failed" => failed += 1,
                    _ => {}
                }
                total_budget += run.budget_sats;
                total_cost += run.total_cost_sats;
                total_duration += run.total_duration_ms;
            }

            draw_rlm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Runs",
                &snapshot.runs.len().to_string(),
                palette.text_secondary,
            );
            draw_rlm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Running",
                &running.to_string(),
                Hsla::new(210.0, 0.6, 0.6, 1.0),
            );
            draw_rlm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Completed",
                &completed.to_string(),
                Hsla::new(120.0, 0.6, 0.5, 1.0),
            );
            draw_rlm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Failed",
                &failed.to_string(),
                Hsla::new(0.0, 0.7, 0.55, 1.0),
            );

            draw_rlm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Budget",
                &format_sats_label(total_budget),
                palette.text_secondary,
            );
            draw_rlm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Cost",
                &format_sats_label(total_cost),
                palette.text_secondary,
            );
            draw_rlm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Duration",
                &format_duration_ms(total_duration.max(0) as u64),
                palette.text_secondary,
            );

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            for run in &snapshot.runs {
                let status_label = run_status_label(&run.status);
                let status_color = run_status_color(&run.status, palette);
                let timestamp = run.completed_at.unwrap_or(run.created_at);
                let time = format_relative_time(timestamp.max(0) as u64);
                let run_id = short_run_id(&run.id);
                let duration = format_duration_ms(run.total_duration_ms.max(0) as u64);
                let summary = format!(
                    "{} {} {} f:{} cost:{} sats dur:{} - {}",
                    time,
                    status_label,
                    run_id,
                    run.fragment_count,
                    format_sats(run.total_cost_sats.max(0) as u64),
                    duration,
                    run.query
                );
                for wrapped in wrap_text(&summary, max_chars) {
                    lines.push((wrapped, status_color));
                }
                if let Some(error) = run.error_message.as_ref() {
                    let error_text = format!("error: {}", error);
                    for wrapped in wrap_text(&error_text, max_chars) {
                        lines.push((wrapped, Hsla::new(0.0, 0.7, 0.55, 1.0)));
                    }
                }
            }

            let max_lines = ((list_bottom - list_top) / line_height)
                .floor()
                .max(0.0) as usize;
            if lines.len() > max_lines {
                let start = lines.len().saturating_sub(max_lines);
                lines = lines[start..].to_vec();
            }

            let mut line_y = list_top;
            if lines.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No RLM runs yet.",
                    Point::new(label_x, line_y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                for (line, color) in lines {
                    let run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, line_y),
                        11.0,
                        color,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(run);
                    line_y += line_height;
                }
            }

            let footer_y = modal_y + modal_height - 24.0;
            let footer = state.text_system.layout_styled_mono(
                "R refresh | T trace | Esc close",
                Point::new(label_x, footer_y),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer);
}

fn draw_rlm_row(
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

fn run_status_label(status: &str) -> &str {
    match status {
        "running" => "Running",
        "completed" => "Completed",
        "failed" => "Failed",
        _ => status,
    }
}

fn run_status_color(status: &str, palette: &UiPalette) -> Hsla {
    match status {
        "running" => Hsla::new(210.0, 0.6, 0.6, 1.0),
        "completed" => Hsla::new(120.0, 0.6, 0.5, 1.0),
        "failed" => Hsla::new(0.0, 0.7, 0.55, 1.0),
        _ => palette.text_secondary,
    }
}

fn short_run_id(run_id: &str) -> String {
    if run_id.len() > 8 {
        format!("{}...", &run_id[..8])
    } else {
        run_id.to_string()
    }
}

fn format_sats_label(value: i64) -> String {
    let value = value.max(0) as u64;
    format!("{} sats", format_sats(value))
}
