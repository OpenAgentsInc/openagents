use crate::app::gateway::GatewayStatus;

fn render_gateway_modal(
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

            let modal_width = GATEWAY_MODAL_WIDTH;
            let modal_height = GATEWAY_MODAL_HEIGHT;
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
                "Gateway Health",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status_color = match &state.gateway.status {
                GatewayStatus::Idle => palette.text_secondary,
                GatewayStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                GatewayStatus::NotConfigured => Hsla::new(35.0, 0.8, 0.6, 1.0),
                GatewayStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            let status_label = state.gateway.status.label().to_string();
            draw_gateway_row(
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

            if let Some(error) = state.gateway.status.error() {
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
            } else if let Some(message) = &state.gateway.status_message {
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

            let snapshot = state.gateway.snapshot.clone();
            let provider = snapshot
                .as_ref()
                .map(|snapshot| snapshot.provider.as_str())
                .unwrap_or("Unknown");
            let name = snapshot
                .as_ref()
                .map(|snapshot| snapshot.name.as_str())
                .unwrap_or("Unknown");
            let configured_text = if matches!(state.gateway.status, GatewayStatus::NotConfigured) {
                "No"
            } else if let Some(snapshot) = snapshot.as_ref() {
                if snapshot.configured {
                    "Yes"
                } else {
                    "No"
                }
            } else {
                "Unknown"
            };

            draw_gateway_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Provider",
                &truncate_preview(provider, 54),
                palette.text_secondary,
            );
            draw_gateway_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Gateway",
                &truncate_preview(name, 54),
                palette.text_secondary,
            );
            draw_gateway_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Configured",
                configured_text,
                palette.text_secondary,
            );

            let last_refresh = state
                .gateway
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());
            draw_gateway_row(
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

            if let Some(snapshot) = snapshot.as_ref() {
                let health = &snapshot.health;
                let health_text = if health.available {
                    "Available"
                } else {
                    "Unavailable"
                };
                let health_color = if health.available {
                    Hsla::new(120.0, 0.6, 0.5, 1.0)
                } else {
                    Hsla::new(0.0, 0.7, 0.55, 1.0)
                };
                draw_gateway_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Health",
                    health_text,
                    health_color,
                );

                let latency_text = health
                    .latency_ms
                    .map(format_duration_ms)
                    .unwrap_or_else(|| "n/a".to_string());
                draw_gateway_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Latency",
                    &latency_text,
                    palette.text_secondary,
                );

                let last_check = if health.last_check > 0 {
                    format_relative_time(health.last_check as u64)
                } else {
                    "Never".to_string()
                };
                draw_gateway_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Last check",
                    &last_check,
                    palette.text_secondary,
                );

                if let Some(error) = &health.error {
                    let error_text = format!("Health error: {}", error);
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
                }

                draw_gateway_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Models",
                    &snapshot.models.len().to_string(),
                    palette.text_secondary,
                );
            } else {
                draw_gateway_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Health",
                    "Unknown",
                    palette.text_faint,
                );
                draw_gateway_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Models",
                    "0",
                    palette.text_faint,
                );
            }

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<String> = Vec::new();
            if let Some(snapshot) = snapshot.as_ref() {
                for model in &snapshot.models {
                    let line = format_model_line(model);
                    for wrapped in wrap_text(&line, max_chars) {
                        lines.push(wrapped);
                    }
                }
            }

            let max_lines = ((list_bottom - list_top) / line_height).floor().max(0.0) as usize;
            if lines.len() > max_lines {
                lines.truncate(max_lines);
            }

            let mut line_y = list_top;
            if lines.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No models loaded.",
                    Point::new(label_x, line_y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                for line in lines {
                    let model_run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, line_y),
                        11.0,
                        palette.text_secondary,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(model_run);
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

fn draw_gateway_row(
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

fn format_model_line(model: &gateway::ModelInfo) -> String {
    let label = if model.name == model.id {
        model.name.clone()
    } else {
        format!("{} ({})", model.name, model.id)
    };
    format!("{} - {} ctx", label, model.context_length)
}
