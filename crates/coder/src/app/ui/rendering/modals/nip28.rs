fn render_nip28_modal(
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

            let modal_width = NIP28_MODAL_WIDTH;
            let modal_height = NIP28_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let label_x = modal_x + 16.0;
            let value_x = modal_x + 160.0;
            let line_height = 18.0;
            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;

            let title_run = state.text_system.layout_styled_mono(
                "NIP-28 Chat",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status_color = match &state.nip28.status {
                Nip28ConnectionStatus::Connected | Nip28ConnectionStatus::Authenticated => {
                    Hsla::new(120.0, 0.6, 0.5, 1.0)
                }
                Nip28ConnectionStatus::Connecting | Nip28ConnectionStatus::Authenticating => {
                    Hsla::new(35.0, 0.8, 0.6, 1.0)
                }
                Nip28ConnectionStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
                Nip28ConnectionStatus::Disconnected => palette.text_faint,
            };

            let status_text = state.nip28.status.label().to_string();
            draw_nip28_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Status",
                &status_text,
                status_color,
            );

            if let Some(error) = state.nip28.status.error() {
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

            let relay_text = truncate_preview(&state.nip28.relay_url, 54);
            draw_nip28_row(
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

            let channel_text = state
                .nip28
                .channel_id
                .as_deref()
                .map(|value| truncate_preview(value, 54))
                .unwrap_or_else(|| "Not set".to_string());
            draw_nip28_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Channel",
                &channel_text,
                palette.text_secondary,
            );

            let pubkey_text = state
                .nip28
                .runtime
                .npub
                .as_deref()
                .map(|value| truncate_preview(value, 54))
                .unwrap_or_else(|| truncate_preview(&state.nip28.runtime.pubkey_hex, 54));
            draw_nip28_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Pubkey",
                &pubkey_text,
                palette.text_secondary,
            );

            if let Some(message) = &state.nip28.status_message {
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
            let input_block_height = 58.0;
            let footer_height = 20.0;
            let messages_bottom = modal_y + modal_height - input_block_height - footer_height;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            for message in &state.nip28.messages {
                let time = format_relative_time(message.created_at);
                let sender = short_pubkey(&message.pubkey);
                let prefix = format!("{} {}:", time, sender);
                let full = format!("{} {}", prefix, message.content);
                let color = if message.pubkey == state.nip28.runtime.pubkey_hex {
                    palette.text_primary
                } else {
                    palette.text_secondary
                };
                for wrapped in wrap_text(&full, max_chars) {
                    lines.push((wrapped, color));
                }
            }

            let max_lines = ((messages_bottom - messages_top) / line_height).floor().max(0.0) as usize;
            if lines.len() > max_lines {
                let start = lines.len().saturating_sub(max_lines);
                lines = lines[start..].to_vec();
            }

            let mut line_y = messages_top;
            if lines.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No chat messages yet.",
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

            let input_label_y = modal_y + modal_height - input_block_height + 8.0;
            let input_label = state.text_system.layout_styled_mono(
                "Message",
                Point::new(label_x, input_label_y),
                11.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(input_label);

            let input_text = format_nip28_input(&state.nip28.input, state.nip28.cursor);
            let input_run = state.text_system.layout_styled_mono(
                &input_text,
                Point::new(value_x, input_label_y),
                11.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(input_run);

            let footer_y = modal_y + modal_height - 24.0;
            let footer = state.text_system.layout_styled_mono(
                "Enter send • Ctrl+R reconnect • Esc close",
                Point::new(label_x, footer_y),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer);
}

fn draw_nip28_row(
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

fn short_pubkey(pubkey: &str) -> String {
    if pubkey.len() > 8 {
        format!("{}...", &pubkey[..8])
    } else {
        pubkey.to_string()
    }
}

fn format_nip28_input(text: &str, cursor: usize) -> String {
    let mut value = text.to_string();
    let idx = cursor.min(value.len());
    value.insert(idx, '|');
    value
}
