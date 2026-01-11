use crate::app::pylon_earnings::PylonEarningsStatus;

fn render_pylon_earnings_modal(
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

            let modal_width = PYLON_EARNINGS_MODAL_WIDTH;
            let modal_height = PYLON_EARNINGS_MODAL_HEIGHT;
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
                "Pylon Earnings",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status = state.pylon_earnings.status.clone();
            let snapshot = state.pylon_earnings.snapshot.clone();

            let status_color = match &status {
                PylonEarningsStatus::Idle => palette.text_secondary,
                PylonEarningsStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                PylonEarningsStatus::MissingDatabase | PylonEarningsStatus::NoHomeDir => {
                    Hsla::new(35.0, 0.8, 0.6, 1.0)
                }
                PylonEarningsStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            draw_pylon_earnings_row(
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
            draw_pylon_earnings_row(
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
            draw_pylon_earnings_row(
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
                .pylon_earnings
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());
            draw_pylon_earnings_row(
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

            draw_pylon_earnings_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Entries",
                &snapshot.totals.total_entries.to_string(),
                palette.text_secondary,
            );
            draw_pylon_earnings_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Jobs",
                &snapshot.totals.job_count.to_string(),
                palette.text_secondary,
            );
            draw_pylon_earnings_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Total",
                &format_pylon_msats(snapshot.totals.total_msats),
                palette.text_secondary,
            );

            let job_total = source_total_msats(&snapshot.totals.by_source, "job");
            let tip_total = source_total_msats(&snapshot.totals.by_source, "tip");
            let other_total = source_total_msats(&snapshot.totals.by_source, "other");
            draw_pylon_earnings_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Job earnings",
                &format_pylon_msats(job_total),
                palette.text_secondary,
            );
            draw_pylon_earnings_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Tips",
                &format_pylon_msats(tip_total),
                palette.text_secondary,
            );
            draw_pylon_earnings_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Other",
                &format_pylon_msats(other_total),
                palette.text_secondary,
            );

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            for earning in &snapshot.earnings {
                let time = format_relative_time(earning.earned_at.max(0) as u64);
                let source_label = format_pylon_source(&earning.source);
                let amount = format_pylon_msats(earning.amount_msats);
                let job_label = earning
                    .job_id
                    .as_ref()
                    .map(|id| format!("job:{}", short_pylon_id(id)))
                    .unwrap_or_else(|| "job:-".to_string());
                let hash_label = earning
                    .payment_hash
                    .as_ref()
                    .map(|hash| format!("hash:{}", short_pylon_id(hash)))
                    .unwrap_or_default();
                let summary = if hash_label.is_empty() {
                    format!("{} {} {} {}", time, source_label, amount, job_label)
                } else {
                    format!(
                        "{} {} {} {} {}",
                        time, source_label, amount, job_label, hash_label
                    )
                };
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
                    "No earnings yet.",
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

fn draw_pylon_earnings_row(
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

fn format_pylon_msats(amount_msats: i64) -> String {
    let value = amount_msats.max(0);
    let sats = value / 1000;
    let remainder = value % 1000;
    if remainder == 0 {
        format!("{} sats", sats)
    } else {
        format!("{}.{:03} sats", sats, remainder)
    }
}

fn format_pylon_source(source: &str) -> &str {
    match source {
        "job" => "Job",
        "tip" => "Tip",
        "other" => "Other",
        _ => source,
    }
}

fn source_total_msats(
    totals: &[crate::app::pylon_earnings::PylonEarningSourceTotal],
    source: &str,
) -> i64 {
    totals
        .iter()
        .find(|entry| entry.source == source)
        .map(|entry| entry.amount_msats)
        .unwrap_or(0)
}

fn short_pylon_id(value: &str) -> String {
    if value.len() > 8 {
        format!("{}...", &value[..8])
    } else {
        value.to_string()
    }
}
