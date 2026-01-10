use crate::app::rlm::RlmTraceStatus;

fn render_rlm_trace_modal(
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

            let modal_width = RLM_TRACE_MODAL_WIDTH;
            let modal_height = RLM_TRACE_MODAL_HEIGHT;
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
                "RLM Trace",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status = state.rlm_trace.status.clone();
            let snapshot = state.rlm_trace.snapshot.clone();

            let status_color = match &status {
                RlmTraceStatus::Idle => palette.text_secondary,
                RlmTraceStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                RlmTraceStatus::MissingDatabase
                | RlmTraceStatus::NoHomeDir
                | RlmTraceStatus::MissingRun => Hsla::new(35.0, 0.8, 0.6, 1.0),
                RlmTraceStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            draw_rlm_trace_row(
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
            draw_rlm_trace_row(
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
            draw_rlm_trace_row(
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
                .rlm_trace
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());
            draw_rlm_trace_row(
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

            if let Some(run) = snapshot.run.as_ref() {
                let run_id = short_trace_run_id(&run.id);
                draw_rlm_trace_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Run ID",
                    &run_id,
                    palette.text_secondary,
                );

                let status_label = trace_run_status_label(&run.status);
                let status_color = trace_run_status_color(&run.status, palette);
                draw_rlm_trace_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Run status",
                    status_label,
                    status_color,
                );

                draw_rlm_trace_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Fragments",
                    &run.fragment_count.to_string(),
                    palette.text_secondary,
                );

                draw_rlm_trace_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Budget",
                    &format_sats_label(run.budget_sats),
                    palette.text_secondary,
                );
                draw_rlm_trace_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Cost",
                    &format_sats_label(run.total_cost_sats),
                    palette.text_secondary,
                );
                draw_rlm_trace_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Duration",
                    &format_duration_ms(run.total_duration_ms.max(0) as u64),
                    palette.text_secondary,
                );

                let started = format_relative_time(run.created_at.max(0) as u64);
                draw_rlm_trace_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Started",
                    &started,
                    palette.text_secondary,
                );

                if let Some(completed_at) = run.completed_at {
                    let completed = format_relative_time(completed_at.max(0) as u64);
                    draw_rlm_trace_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        "Completed",
                        &completed,
                        palette.text_secondary,
                    );
                }

                let query_text = format!("Query: {}", run.query);
                for line in wrap_text(&query_text, max_chars) {
                    let run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, y),
                        11.0,
                        palette.text_secondary,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(run);
                    y += line_height;
                }

                if let Some(error) = run.error_message.as_ref() {
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
            }

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            for event in &snapshot.events {
                let time_label = format_trace_time(event.timestamp_ms);
                let summary = format!(
                    "#{} {} {} {}",
                    event.seq, time_label, event.event_type, event.event_json
                );
                for wrapped in wrap_text(&summary, max_chars) {
                    lines.push((wrapped, palette.text_secondary));
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
                    "No trace events yet.",
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
                "R refresh | Esc close",
                Point::new(label_x, footer_y),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer);
}

fn draw_rlm_trace_row(
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

fn trace_run_status_label(status: &str) -> &str {
    match status {
        "running" => "Running",
        "completed" => "Completed",
        "failed" => "Failed",
        _ => status,
    }
}

fn trace_run_status_color(status: &str, palette: &UiPalette) -> Hsla {
    match status {
        "running" => Hsla::new(210.0, 0.6, 0.6, 1.0),
        "completed" => Hsla::new(120.0, 0.6, 0.5, 1.0),
        "failed" => Hsla::new(0.0, 0.7, 0.55, 1.0),
        _ => palette.text_secondary,
    }
}

fn short_trace_run_id(run_id: &str) -> String {
    if run_id.len() > 8 {
        format!("{}...", &run_id[..8])
    } else {
        run_id.to_string()
    }
}

fn format_trace_time(timestamp_ms: i64) -> String {
    if timestamp_ms <= 0 {
        return "0ms".to_string();
    }
    if timestamp_ms > 1_000_000_000_000 {
        return format_relative_time((timestamp_ms / 1000) as u64);
    }
    format_duration_ms(timestamp_ms as u64)
}
