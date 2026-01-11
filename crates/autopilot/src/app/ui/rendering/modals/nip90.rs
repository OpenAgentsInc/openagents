fn render_nip90_modal(
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

            let modal_width = NIP90_MODAL_WIDTH;
            let modal_height = NIP90_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let label_x = modal_x + 16.0;
            let value_x = modal_x + 150.0;
            let line_height = 18.0;
            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;

            let title_run = state.text_system.layout_styled_mono(
                "NIP-90 Jobs",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status_color = match &state.nip90.status {
                Nip90ConnectionStatus::Connected | Nip90ConnectionStatus::Authenticated => {
                    Hsla::new(120.0, 0.6, 0.5, 1.0)
                }
                Nip90ConnectionStatus::Connecting | Nip90ConnectionStatus::Authenticating => {
                    Hsla::new(35.0, 0.8, 0.6, 1.0)
                }
                Nip90ConnectionStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
                Nip90ConnectionStatus::Disconnected => palette.text_faint,
            };

            let status_label = state.nip90.status.label().to_string();
            draw_nip90_row(
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

            let status_error = state.nip90.status.error().map(|value| value.to_string());
            if let Some(error) = status_error {
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

            let relay_text = truncate_preview(&state.nip90.relay_url, 54);
            draw_nip90_row(
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

            let mut request_count = 0;
            let mut result_count = 0;
            let mut feedback_count = 0;
            for message in &state.nip90.messages {
                match message.message_kind {
                    Nip90MessageKind::Request => request_count += 1,
                    Nip90MessageKind::Result => result_count += 1,
                    Nip90MessageKind::Feedback => feedback_count += 1,
                }
            }
            let counts = format!(
                "{} requests, {} results, {} feedback",
                request_count, result_count, feedback_count
            );
            draw_nip90_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Totals",
                &counts,
                palette.text_secondary,
            );

            if let Some(message) = &state.nip90.status_message {
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
            let messages_top = y;
            let footer_height = 20.0;
            let messages_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            for message in &state.nip90.messages {
                let time = format_relative_time(message.created_at);
                let sender = short_pubkey_nip90(&message.pubkey);
                let prefix = format!(
                    "{} {} {} {}:",
                    time,
                    message.message_kind.label(),
                    message.kind,
                    sender
                );
                let full = format!("{} {}", prefix, message.summary);
                let color = match message.message_kind {
                    Nip90MessageKind::Request => Hsla::new(210.0, 0.6, 0.6, 1.0),
                    Nip90MessageKind::Result => Hsla::new(120.0, 0.5, 0.55, 1.0),
                    Nip90MessageKind::Feedback => Hsla::new(35.0, 0.7, 0.6, 1.0),
                };
                for wrapped in wrap_text(&full, max_chars) {
                    lines.push((wrapped, color));
                }
            }

            let max_lines =
                ((messages_bottom - messages_top) / line_height).floor().max(0.0) as usize;
            if lines.len() > max_lines {
                let start = lines.len().saturating_sub(max_lines);
                lines = lines[start..].to_vec();
            }

            let mut line_y = messages_top;
            if lines.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No NIP-90 events yet.",
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

fn draw_nip90_row(
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

fn short_pubkey_nip90(pubkey: &str) -> String {
    if pubkey.len() > 8 {
        format!("{}...", &pubkey[..8])
    } else {
        pubkey.to_string()
    }
}
