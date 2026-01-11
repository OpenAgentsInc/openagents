use crate::app::dvm::{job_kind_label, DvmStatus};

fn render_dvm_modal(
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

            let modal_width = DVM_MODAL_WIDTH;
            let modal_height = DVM_MODAL_HEIGHT;
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
                "DVM Providers",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status_color = match &state.dvm.status {
                DvmStatus::Idle => palette.text_secondary,
                DvmStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                DvmStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            let status_label = state.dvm.status.label().to_string();
            draw_dvm_row(
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

            if let Some(error) = state.dvm.status.error() {
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
            }

            let relay_text = truncate_preview(&state.dvm.relay_url, 54);
            draw_dvm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Relay",
                &relay_text,
                palette.text_secondary,
            );

            let job_kind_label_text = format!(
                "{} ({})",
                state.dvm.job_kind,
                job_kind_label(state.dvm.job_kind)
            );
            draw_dvm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Job kind",
                &job_kind_label_text,
                palette.text_secondary,
            );

            draw_dvm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Providers",
                &state.dvm.providers.len().to_string(),
                palette.text_secondary,
            );

            let auth_key = truncate_preview(&state.dvm.runtime.pubkey_hex, 42);
            draw_dvm_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Auth pubkey",
                &auth_key,
                palette.text_secondary,
            );

            let last_refresh = state
                .dvm
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());
            draw_dvm_row(
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

            if let Some(message) = &state.dvm.status_message {
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

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            for provider in &state.dvm.providers {
                let name = provider.name.as_deref().unwrap_or("Unnamed provider");
                let pubkey = short_pubkey_dvm(&provider.pubkey);
                let kind_summary = format_supported_kinds(&provider.supported_kinds);
                let relay = provider.relays.first().map(String::as_str).unwrap_or("-");
                let summary = format!(
                    "{} ({}) • {} • {}",
                    name,
                    pubkey,
                    kind_summary,
                    relay
                );
                for wrapped in wrap_text(&summary, max_chars) {
                    lines.push((wrapped, palette.text_secondary));
                }
                if let Some(about) = provider.about.as_deref() {
                    if !about.trim().is_empty() {
                        for wrapped in wrap_text(about, max_chars) {
                            lines.push((wrapped, palette.text_faint));
                        }
                    }
                }
                lines.push(("".to_string(), palette.text_faint));
            }

            let max_lines = ((list_bottom - list_top) / line_height).floor().max(0.0) as usize;
            if lines.len() > max_lines {
                let start = lines.len().saturating_sub(max_lines);
                lines = lines[start..].to_vec();
            }

            let mut line_y = list_top;
            if lines.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No providers discovered yet.",
                    Point::new(label_x, line_y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                for (line, color) in lines {
                    let msg_run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, line_y),
                        11.0,
                        color,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(msg_run);
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

fn draw_dvm_row(
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

fn short_pubkey_dvm(pubkey: &str) -> String {
    if pubkey.len() > 8 {
        format!("{}...", &pubkey[..8])
    } else {
        pubkey.to_string()
    }
}

fn format_supported_kinds(kinds: &[u16]) -> String {
    if kinds.is_empty() {
        return "no kinds".to_string();
    }
    let mut kinds = kinds.to_vec();
    kinds.sort_unstable();
    let labels: Vec<String> = kinds
        .iter()
        .take(3)
        .map(|kind| job_kind_label(*kind).to_string())
        .collect();
    let mut summary = labels.join(", ");
    if kinds.len() > 3 {
        summary.push_str(&format!(" +{}", kinds.len() - 3));
    }
    summary
}
