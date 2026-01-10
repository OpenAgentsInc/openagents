        ModalState::Wallet => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let modal_width = 680.0;
            let modal_height = 420.0;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let snapshot = &state.wallet.snapshot;
            let mut y = modal_y + 16.0;

            let title_run = state.text_system.layout_styled_mono(
                "Wallet",
                Point::new(modal_x + 16.0, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let refresh_pending = state.autopilot.oanix_manifest_rx.is_some();
            if refresh_pending {
                let pending_run = state.text_system.layout_styled_mono(
                    "OANIX refresh in progress...",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    palette.text_muted,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(pending_run);
                y += 18.0;
            }

            let label_x = modal_x + 16.0;
            let value_x = modal_x + 150.0;
            let line_height = 18.0;

            let identity_header = state.text_system.layout_styled_mono(
                "Identity",
                Point::new(label_x, y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(identity_header);
            y += line_height;

            let status_label = state.text_system.layout_styled_mono(
                "Status",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(status_label);

            let (status_text, status_color) = match snapshot.identity.state {
                WalletIdentityState::Initialized => ("Initialized", Hsla::new(120.0, 0.6, 0.5, 1.0)),
                WalletIdentityState::Uninitialized => ("Not initialized", Hsla::new(35.0, 0.8, 0.6, 1.0)),
                WalletIdentityState::Unknown => ("Pending", palette.text_faint),
            };
            let status_run = state.text_system.layout_styled_mono(
                status_text,
                Point::new(value_x, y),
                12.0,
                status_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(status_run);
            y += line_height;

            let npub_label = state.text_system.layout_styled_mono(
                "Npub",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(npub_label);
            let npub_text = snapshot
                .identity
                .npub
                .as_deref()
                .map(|npub| truncate_preview(npub, 54))
                .unwrap_or_else(|| "Not available".to_string());
            let npub_run = state.text_system.layout_styled_mono(
                &npub_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(npub_run);
            y += line_height;

            let network_label = state.text_system.layout_styled_mono(
                "Network",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(network_label);
            let network_text = snapshot
                .identity
                .network
                .as_deref()
                .or(snapshot.config.network.as_deref())
                .unwrap_or("Unknown");
            let network_run = state.text_system.layout_styled_mono(
                network_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(network_run);
            y += line_height;

            let balance_label = state.text_system.layout_styled_mono(
                "Balance",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(balance_label);
            let balance_text = snapshot
                .identity
                .balance_sats
                .map(|sats| format!("{} sats", sats))
                .unwrap_or_else(|| "Not detected".to_string());
            let balance_run = state.text_system.layout_styled_mono(
                &balance_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(balance_run);
            y += line_height;

            if snapshot.identity.state == WalletIdentityState::Uninitialized {
                let hint_run = state.text_system.layout_styled_mono(
                    "Run `pylon init` to create identity",
                    Point::new(label_x, y),
                    12.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(hint_run);
                y += line_height;
            }

            y += 10.0;
            let config_header = state.text_system.layout_styled_mono(
                "Config",
                Point::new(label_x, y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(config_header);
            y += line_height;

            let config_label = state.text_system.layout_styled_mono(
                "Config file",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(config_label);
            let config_text = match &snapshot.config.config_path {
                Some(path) => {
                    let path_text = path.display().to_string();
                    if snapshot.config.exists {
                        truncate_preview(&path_text, 54)
                    } else {
                        truncate_preview(&format!("Missing ({})", path_text), 54)
                    }
                }
                None => "Unavailable".to_string(),
            };
            let config_run = state.text_system.layout_styled_mono(
                &config_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(config_run);
            y += line_height;

            let payments_label = state.text_system.layout_styled_mono(
                "Payments",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(payments_label);
            let payments_text = snapshot
                .config
                .enable_payments
                .map(|enabled| if enabled { "Enabled" } else { "Disabled" })
                .unwrap_or("Unknown");
            let payments_run = state.text_system.layout_styled_mono(
                payments_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(payments_run);
            y += line_height;

            let spark_url_label = state.text_system.layout_styled_mono(
                "Spark URL",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(spark_url_label);
            let spark_url_text = snapshot
                .config
                .spark_url
                .as_deref()
                .map(|url| truncate_preview(url, 54))
                .unwrap_or_else(|| "Not set".to_string());
            let spark_url_run = state.text_system.layout_styled_mono(
                &spark_url_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(spark_url_run);
            y += line_height;

            let spark_token_label = state.text_system.layout_styled_mono(
                "Spark token",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(spark_token_label);
            let spark_token_text = snapshot
                .config
                .spark_token_present
                .map(|present| if present { "Set" } else { "Missing" })
                .unwrap_or("Unknown");
            let spark_token_run = state.text_system.layout_styled_mono(
                spark_token_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(spark_token_run);
            y += line_height;

            let data_dir_label = state.text_system.layout_styled_mono(
                "Data dir",
                Point::new(label_x, y),
                12.0,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(data_dir_label);
            let data_dir_text = snapshot
                .config
                .data_dir
                .as_ref()
                .map(|dir| truncate_preview(&dir.display().to_string(), 54))
                .unwrap_or_else(|| "Unknown".to_string());
            let data_dir_run = state.text_system.layout_styled_mono(
                &data_dir_text,
                Point::new(value_x, y),
                12.0,
                palette.text_secondary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(data_dir_run);
            y += line_height;

            if let Some(error) = &snapshot.config.error {
                let error_text = truncate_preview(error, 76);
                let error_run = state.text_system.layout_styled_mono(
                    &error_text,
                    Point::new(label_x, y),
                    12.0,
                    Hsla::new(15.0, 0.7, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(error_run);
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "R refresh · Enter/Esc to close",
                Point::new(modal_x + 16.0, y),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
