use crate::app::spark_wallet::{
    SparkPaymentDirection, SparkPaymentState, SparkWalletStatus,
};

fn render_spark_wallet_modal(
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

            let modal_width = SPARK_WALLET_MODAL_WIDTH;
            let modal_height = SPARK_WALLET_MODAL_HEIGHT;
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
                "Spark Wallet",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status_color = match &state.spark_wallet.status {
                SparkWalletStatus::Idle => palette.text_secondary,
                SparkWalletStatus::Refreshing | SparkWalletStatus::NotConfigured => {
                    Hsla::new(35.0, 0.8, 0.6, 1.0)
                }
                SparkWalletStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            let status_label = state.spark_wallet.status.label().to_string();
            draw_spark_wallet_row(
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

            if let Some(error) = state.spark_wallet.status.error() {
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
            } else if let Some(message) = &state.spark_wallet.status_message {
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

            let snapshot = state.spark_wallet.snapshot.clone();
            let last_refresh = state
                .spark_wallet
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());

            if let Some(snapshot) = snapshot.as_ref() {
                let network_label = format!("{:?}", snapshot.network);
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Network",
                    &network_label,
                    palette.text_secondary,
                );
                let api_key_label = if snapshot.api_key_present { "Yes" } else { "No" };
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "API key",
                    api_key_label,
                    palette.text_secondary,
                );
                let oa_label = if !snapshot.openagents_api_key_present {
                    "Not set (add openagents_api_key to pylon config)"
                } else if state.spark_wallet.openagents_linked == Some(true) {
                    "Linked"
                } else if state.spark_wallet.openagents_linked == Some(false) {
                    "Not linked — /spark attach to link"
                } else {
                    "— /spark attach to link"
                };
                let oa_color = if state.spark_wallet.openagents_linked == Some(true) {
                    Hsla::new(120.0, 0.5, 0.5, 1.0)
                } else {
                    palette.text_secondary
                };
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "OpenAgents account",
                    oa_label,
                    oa_color,
                );
                if let Some(ref err) = state.spark_wallet.openagents_attach_error {
                    for line in wrap_text(&format!("Attach error: {}", err), max_chars) {
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
                let storage_text = snapshot.storage_dir.display().to_string();
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Storage",
                    &truncate_preview(&storage_text, 54),
                    palette.text_secondary,
                );
                draw_spark_wallet_row(
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

                let balance = &snapshot.balance;
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Spark balance",
                    &format_sats(balance.spark_sats),
                    palette.text_secondary,
                );
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Lightning balance",
                    &format_sats(balance.lightning_sats),
                    palette.text_secondary,
                );
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "On-chain balance",
                    &format_sats(balance.onchain_sats),
                    palette.text_secondary,
                );
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Total",
                    &format_sats(balance.total_sats()),
                    palette.text_secondary,
                );

                let net_status = snapshot.network_status.status.as_str().to_string();
                let net_color = match snapshot.network_status.status {
                    openagents_spark::NetworkStatus::Connected => {
                        Hsla::new(120.0, 0.6, 0.5, 1.0)
                    }
                    openagents_spark::NetworkStatus::Disconnected => {
                        Hsla::new(0.0, 0.7, 0.55, 1.0)
                    }
                };
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Network status",
                    &net_status,
                    net_color,
                );

                if let Some(detail) = &snapshot.network_status.detail {
                    let detail_text = format!("Detail: {}", detail);
                    for line in wrap_text(&detail_text, max_chars) {
                        let detail_run = state.text_system.layout_styled_mono(
                            &line,
                            Point::new(label_x, y),
                            11.0,
                            palette.text_faint,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(detail_run);
                        y += line_height;
                    }
                }

                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Spark address",
                    &truncate_preview(&snapshot.spark_address, 54),
                    palette.text_secondary,
                );
                draw_spark_wallet_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Bitcoin address",
                    &truncate_preview(&snapshot.bitcoin_address, 54),
                    palette.text_secondary,
                );
            } else {
                draw_spark_wallet_row(
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
            }

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            if let Some(snapshot) = snapshot {
                if snapshot.payments.is_empty() {
                    lines.push(("No payments found.".to_string(), palette.text_faint));
                } else {
                    lines.push(("Recent payments".to_string(), palette.text_primary));
                    for payment in snapshot.payments {
                        let time = if payment.timestamp > 0 {
                            format_relative_time(payment.timestamp)
                        } else {
                            "Unknown".to_string()
                        };
                        let label = format!(
                            "{} {} {} sats {}",
                            payment.direction.label(),
                            payment.status.label(),
                            payment.amount_sats,
                            time
                        );
                        let color = payment_color(&payment);
                        lines.push((label, color));
                        let id_line = format!("id {}", truncate_preview(&payment.id, 16));
                        lines.push((id_line, palette.text_faint));
                    }
                }
            } else {
                lines.push(("No Spark wallet data yet.".to_string(), palette.text_faint));
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

fn draw_spark_wallet_row(
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

fn format_sats(sats: u64) -> String {
    if sats >= 1_000_000 {
        format!("{:.2}M", sats as f64 / 1_000_000.0)
    } else if sats >= 1_000 {
        format!("{:.1}K", sats as f64 / 1_000.0)
    } else {
        sats.to_string()
    }
}

fn payment_color(payment: &crate::app::spark_wallet::SparkPaymentSummary) -> Hsla {
    match payment.status {
        SparkPaymentState::Completed => match payment.direction {
            SparkPaymentDirection::Receive => Hsla::new(120.0, 0.6, 0.5, 1.0),
            SparkPaymentDirection::Send => Hsla::new(210.0, 0.5, 0.6, 1.0),
        },
        SparkPaymentState::Pending => Hsla::new(35.0, 0.8, 0.6, 1.0),
        SparkPaymentState::Failed => Hsla::new(0.0, 0.7, 0.55, 1.0),
    }
}
