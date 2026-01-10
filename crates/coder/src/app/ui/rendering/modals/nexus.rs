use crate::app::nexus::{NexusKindCount, NexusStatus};

fn render_nexus_modal(
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

            let modal_width = NEXUS_MODAL_WIDTH;
            let modal_height = NEXUS_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let label_x = modal_x + 16.0;
            let value_x = modal_x + 190.0;
            let line_height = 18.0;
            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;

            let title_run = state.text_system.layout_styled_mono(
                "Nexus Relay Stats",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status_color = match &state.nexus.status {
                NexusStatus::Idle => palette.text_secondary,
                NexusStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                NexusStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };
            let status_label = state.nexus.status.label().to_string();
            draw_nexus_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Status",
                &status_label,
                status_color,
            );

            if let Some(error) = state.nexus.status.error() {
                let error_text = format!("Error: {}", error);
                for line in wrap_text(&error_text, max_chars) {
                    let error_run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(error_run);
                    y += line_height;
                }
            } else if let Some(message) = &state.nexus.status_message {
                let status_text = format!("Status: {}", message);
                for line in wrap_text(&status_text, max_chars) {
                    let status_run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(status_run);
                    y += line_height;
                }
            }

            let stats_url = truncate_preview(&state.nexus.stats_url, 54);
            let last_refresh = state
                .nexus
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());

            draw_nexus_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Stats URL",
                &stats_url,
                palette.text_secondary,
            );
            draw_nexus_row(
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

            let snapshot = state.nexus.snapshot.clone();
            let timestamp_label = snapshot
                .as_ref()
                .filter(|snapshot| snapshot.stats.timestamp > 0)
                .map(|snapshot| format_relative_time(snapshot.stats.timestamp))
                .unwrap_or_else(|| "Unknown".to_string());
            draw_nexus_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Stats timestamp",
                &timestamp_label,
                palette.text_secondary,
            );

            if let Some(snapshot) = snapshot.as_ref() {
                draw_nexus_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Events total",
                    &snapshot.stats.events.total.to_string(),
                    palette.text_secondary,
                );
                draw_nexus_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Events 24h",
                    &snapshot.stats.events.last_24h.to_string(),
                    palette.text_secondary,
                );
                draw_nexus_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Jobs pending",
                    &snapshot.stats.jobs.pending.to_string(),
                    palette.text_secondary,
                );
                draw_nexus_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Jobs 24h",
                    &snapshot.stats.jobs.completed_24h.to_string(),
                    palette.text_secondary,
                );
                draw_nexus_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "RLM subqueries",
                    &format!(
                        "{} total, {} 24h",
                        snapshot.stats.rlm.subqueries_total, snapshot.stats.rlm.subqueries_24h
                    ),
                    palette.text_secondary,
                );
                draw_nexus_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "RLM results",
                    &format!(
                        "{} total, {} 24h",
                        snapshot.stats.rlm.results_total, snapshot.stats.rlm.results_24h
                    ),
                    palette.text_secondary,
                );
                draw_nexus_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "RLM providers",
                    &snapshot.stats.rlm.providers_active.to_string(),
                    palette.text_secondary,
                );
            }

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            if let Some(snapshot) = snapshot {
                let event_kinds = top_kinds(&snapshot.stats.events.by_kind, 6);
                let job_kinds = top_kinds(&snapshot.stats.jobs.by_kind, 6);

                if !event_kinds.is_empty() {
                    lines.push(("Event kinds".to_string(), palette.text_primary));
                    for kind in event_kinds {
                        lines.push((
                            format!("{} - {}", kind.kind, kind.count),
                            palette.text_secondary,
                        ));
                    }
                }

                if !job_kinds.is_empty() {
                    if !lines.is_empty() {
                        lines.push(("".to_string(), palette.text_faint));
                    }
                    lines.push(("Job kinds".to_string(), palette.text_primary));
                    for kind in job_kinds {
                        lines.push((
                            format!("{} - {}", kind.kind, kind.count),
                            palette.text_secondary,
                        ));
                    }
                }
            }

            let max_lines = ((list_bottom - list_top) / line_height).floor().max(0.0) as usize;
            let mut rendered_lines: Vec<(String, Hsla)> = Vec::new();
            for (text, color) in lines {
                for wrapped in wrap_text(&text, max_chars) {
                    rendered_lines.push((wrapped, color));
                }
            }
            if rendered_lines.len() > max_lines {
                rendered_lines.truncate(max_lines);
            }

            let mut line_y = list_top;
            if rendered_lines.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No Nexus stats yet.",
                    Point::new(label_x, line_y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                for (line, color) in rendered_lines {
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

fn draw_nexus_row(
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

fn top_kinds(kinds: &[NexusKindCount], limit: usize) -> Vec<NexusKindCount> {
    let mut sorted = kinds.to_vec();
    sorted.sort_by(|a, b| b.count.cmp(&a.count));
    sorted.truncate(limit);
    sorted
}
