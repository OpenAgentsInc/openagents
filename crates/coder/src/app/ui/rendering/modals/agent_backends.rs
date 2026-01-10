use crate::app::agents::{AgentBackendsStatus, AgentKind};

fn render_agent_backends_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    _scale_factor: f32,
    selected: &usize,
    model_selected: &usize,
) {
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let modal_width = AGENT_BACKENDS_MODAL_WIDTH;
            let modal_height = AGENT_BACKENDS_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let label_x = modal_x + 16.0;
            let line_height = 18.0;
            let max_chars = ((modal_width - 32.0) / 7.0).max(24.0) as usize;

            let title_run = state.text_system.layout_styled_mono(
                "Agent Backends",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let selection_label = state.agent_backends.settings.selected.display_name();
            let selection_run = state.text_system.layout_styled_mono(
                &format!("Selected: {}", selection_label),
                Point::new(label_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(selection_run);
            y += line_height;

            let available_kinds = state.agent_backends.available_kinds();
            let available_line = if available_kinds.is_empty() {
                "Available: none".to_string()
            } else {
                let names = available_kinds
                    .iter()
                    .map(|kind| kind.display_name())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Available: {}", names)
            };
            let available_run = state.text_system.layout_styled_mono(
                &available_line,
                Point::new(label_x, y),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(available_run);
            y += line_height;

            let default_kind = state
                .agent_backends
                .default_kind()
                .map(|kind| kind.display_name())
                .unwrap_or("none");
            let default_run = state.text_system.layout_styled_mono(
                &format!("Default backend: {}", default_kind),
                Point::new(label_x, y),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(default_run);
            y += line_height;

            let status_color = match state.agent_backends.status {
                AgentBackendsStatus::Idle => palette.text_secondary,
                AgentBackendsStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                AgentBackendsStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };
            let status_line = format!("Status: {}", state.agent_backends.status.label());
            let status_run = state.text_system.layout_styled_mono(
                &status_line,
                Point::new(label_x, y),
                12.0,
                status_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(status_run);
            y += line_height;

            if let Some(message) = state.agent_backends.status.error() {
                for line in wrap_text(&format!("Error: {}", message), max_chars) {
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
            } else if let Some(message) = &state.agent_backends.status_message {
                for line in wrap_text(&format!("Status: {}", message), max_chars) {
                    let msg_run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(msg_run);
                    y += line_height;
                }
            }

            let refreshed_text = state
                .agent_backends
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "never".to_string());
            let refresh_run = state.text_system.layout_styled_mono(
                &format!("Last refresh: {}", refreshed_text),
                Point::new(label_x, y),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(refresh_run);
            y += 24.0;

            let kinds = state.agent_backends.kinds();
            let selected_idx = (*selected).min(kinds.len().saturating_sub(1));
            let selected_kind = kinds
                .get(selected_idx)
                .copied()
                .unwrap_or(AgentKind::Claude);

            if let Some(status) = state.agent_backends.status_for_kind(selected_kind) {
                if let Some(error) = status.error.as_ref() {
                    for line in wrap_text(&format!("Backend error: {}", error), max_chars) {
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
            }

            let left_x = label_x;
            let right_x = modal_x + 320.0;
            let mut list_y = y;

            let backends_header = state.text_system.layout_styled_mono(
                "Backends",
                Point::new(left_x, list_y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(backends_header);

            let models_header = state.text_system.layout_styled_mono(
                "Models",
                Point::new(right_x, list_y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(models_header);
            list_y += line_height;

            for (idx, kind) in kinds.iter().enumerate() {
                let is_selected = idx == selected_idx;
                let status = state.agent_backends.status_for_kind(*kind);
                let available = status.as_ref().map(|s| s.available).unwrap_or(false);
                let status_label = if available { "available" } else { "missing" };
                let name = status
                    .as_ref()
                    .map(|s| s.name.as_str())
                    .unwrap_or(kind.display_name());
                let icon = status
                    .as_ref()
                    .map(|s| s.icon.as_str())
                    .unwrap_or("");
                let indicator = if is_selected { ">" } else { " " };
                let error_tag = status
                    .as_ref()
                    .and_then(|status| status.error.as_ref())
                    .map(|_| " (error)")
                    .unwrap_or("");
                let line = if icon.is_empty() {
                    format!("{} {}{} ({})", indicator, name, error_tag, status_label)
                } else {
                    format!(
                        "{} [{}] {}{} ({})",
                        indicator, icon, name, error_tag, status_label
                    )
                };
                let color = if available {
                    palette.text_secondary
                } else {
                    Hsla::new(0.0, 0.6, 0.55, 1.0)
                };
                let line_run = state.text_system.layout_styled_mono(
                    &line,
                    Point::new(left_x, list_y),
                    12.0,
                    if is_selected { palette.text_primary } else { color },
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(line_run);
                list_y += line_height;
            }

            let mut model_y = y + line_height;
            let models = state.agent_backends.models_for_kind(selected_kind);
            let default_model_id = state.agent_backends.default_model_for_kind(selected_kind);
            let mut model_rows = Vec::new();
            let default_desc = default_model_id
                .as_deref()
                .map(|id| format!("Use backend default ({})", id))
                .unwrap_or_else(|| "Use backend default".to_string());
            model_rows.push(("Default".to_string(), default_desc, false));
            for model in &models {
                let mut desc = model.description.clone().unwrap_or_default();
                if model.is_default {
                    if desc.is_empty() {
                        desc = "Default model".to_string();
                    } else {
                        desc = format!("{} (default)", desc);
                    }
                }
                model_rows.push((model.name.clone(), desc, model.is_default));
            }

            if model_rows.len() == 1 {
                let empty_run = state.text_system.layout_styled_mono(
                    "No model data available.",
                    Point::new(right_x, model_y),
                    12.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let clamped_model = (*model_selected).min(model_rows.len().saturating_sub(1));
                let active_model_id = state.agent_backends.settings.selected.model_id.clone();

                for (idx, (name, desc, is_default)) in model_rows.iter().enumerate() {
                    let is_selected = idx == clamped_model;
                    let is_active = if idx == 0 {
                        active_model_id.is_none()
                    } else {
                        models
                            .get(idx - 1)
                            .map(|model| Some(&model.id) == active_model_id.as_ref())
                            .unwrap_or(false)
                    };

                    let indicator = if is_selected { ">" } else { " " };
                    let active_tag = if is_active { " (active)" } else { "" };
                    let default_tag = if *is_default { " (default)" } else { "" };
                    let label = format!("{} {}{}{}", indicator, name, default_tag, active_tag);
                    let label_run = state.text_system.layout_styled_mono(
                        &label,
                        Point::new(right_x, model_y),
                        12.0,
                        if is_selected {
                            palette.text_primary
                        } else if is_active {
                            Hsla::new(120.0, 0.5, 0.6, 1.0)
                        } else {
                            palette.text_secondary
                        },
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(label_run);
                    model_y += line_height;

                    if !desc.is_empty() {
                        let desc_line = truncate_preview(desc, 60);
                        let desc_run = state.text_system.layout_styled_mono(
                            &desc_line,
                            Point::new(right_x + 16.0, model_y - 2.0),
                            10.0,
                            palette.text_faint,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(desc_run);
                        model_y += 10.0;
                    }
                }
            }

            let detail_top = modal_y + modal_height - 96.0;
            let binary_line = format!("Binary: {}", selected_kind.executable_name());
            let binary_run = state.text_system.layout_styled_mono(
                &truncate_preview(&binary_line, max_chars),
                Point::new(label_x, detail_top),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(binary_run);

            let (path_line, version_line, error_line) = match state
                .agent_backends
                .availability_for_kind(selected_kind)
            {
                Some(availability) => {
                    let path_line = availability
                        .executable_path
                        .map(|path| format!("Executable: {}", path.display()))
                        .unwrap_or_else(|| "Executable: not detected".to_string());
                    let version_line = availability
                        .version
                        .as_deref()
                        .map(|version| format!("Version: {}", version))
                        .unwrap_or_else(|| "Version: unknown".to_string());
                    let error_line = availability
                        .error
                        .map(|error| format!("Details: {}", error));
                    (path_line, version_line, error_line)
                }
                None => (
                    "Executable: not detected".to_string(),
                    "Version: unknown".to_string(),
                    None,
                ),
            };

            let path_run = state.text_system.layout_styled_mono(
                &truncate_preview(&path_line, max_chars),
                Point::new(label_x, detail_top + line_height),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(path_run);

            let version_run = state.text_system.layout_styled_mono(
                &truncate_preview(&version_line, max_chars),
                Point::new(label_x, detail_top + line_height * 2.0),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(version_run);

            if let Some(error_line) = error_line {
                let error_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&error_line, max_chars),
                    Point::new(label_x, detail_top + line_height * 3.0),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(error_run);
            }

            let footer_run = state.text_system.layout_styled_mono(
                "Up/Down select backend · Left/Right select model · Enter apply · R refresh · Esc close",
                Point::new(label_x, modal_y + modal_height - 24.0),
                11.0,
                palette.text_dim,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
}
