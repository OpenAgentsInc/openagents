use crate::app::pylon_jobs::PylonJobsStatus;

fn render_pylon_jobs_modal(
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

            let modal_width = PYLON_JOBS_MODAL_WIDTH;
            let modal_height = PYLON_JOBS_MODAL_HEIGHT;
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
                "Pylon Jobs",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let status = state.pylon_jobs.status.clone();
            let snapshot = state.pylon_jobs.snapshot.clone();

            let status_color = match &status {
                PylonJobsStatus::Idle => palette.text_secondary,
                PylonJobsStatus::Refreshing => Hsla::new(35.0, 0.8, 0.6, 1.0),
                PylonJobsStatus::MissingDatabase | PylonJobsStatus::NoHomeDir => {
                    Hsla::new(35.0, 0.8, 0.6, 1.0)
                }
                PylonJobsStatus::Error(_) => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };

            draw_pylon_jobs_row(
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
            draw_pylon_jobs_row(
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
            draw_pylon_jobs_row(
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
                .pylon_jobs
                .last_refresh
                .map(format_relative_time)
                .unwrap_or_else(|| "Never".to_string());
            draw_pylon_jobs_row(
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

            let totals = &snapshot.totals;
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Total jobs",
                &totals.total_jobs.to_string(),
                palette.text_secondary,
            );
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Pending",
                &totals.pending.to_string(),
                Hsla::new(35.0, 0.8, 0.6, 1.0),
            );
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Processing",
                &totals.processing.to_string(),
                Hsla::new(210.0, 0.6, 0.6, 1.0),
            );
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Completed",
                &totals.completed.to_string(),
                Hsla::new(120.0, 0.6, 0.5, 1.0),
            );
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Failed",
                &totals.failed.to_string(),
                Hsla::new(0.0, 0.7, 0.55, 1.0),
            );
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Cancelled",
                &totals.cancelled.to_string(),
                palette.text_faint,
            );
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Total price",
                &format_job_msats(totals.total_price_msats),
                palette.text_secondary,
            );
            draw_pylon_jobs_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Completed price",
                &format_job_msats(totals.completed_price_msats),
                palette.text_secondary,
            );

            y += 6.0;
            let list_top = y;
            let footer_height = 20.0;
            let list_bottom = modal_y + modal_height - footer_height - 12.0;

            let mut lines: Vec<(String, Hsla)> = Vec::new();
            for job in &snapshot.jobs {
                let timestamp = job.completed_at.unwrap_or(job.created_at);
                let time = format_relative_time(timestamp.max(0) as u64);
                let status_label = format_job_status(&job.status);
                let status_color = job_status_color(&job.status, palette);
                let kind_label = format_job_kind(job.kind);
                let price = format_job_msats(job.price_msats);
                let job_id = short_job_id(&job.id);
                let customer = short_pylon_pubkey(&job.customer_pubkey);
                let summary = format!(
                    "{} {} kind:{} price:{} job:{} cust:{}",
                    time, status_label, kind_label, price, job_id, customer
                );
                for wrapped in wrap_text(&summary, max_chars) {
                    lines.push((wrapped, status_color));
                }
                if let Some(error) = job.error_message.as_ref() {
                    let error_text = format!("error: {}", error);
                    for wrapped in wrap_text(&error_text, max_chars) {
                        lines.push((wrapped, Hsla::new(0.0, 0.7, 0.55, 1.0)));
                    }
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
                    "No jobs yet.",
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

fn draw_pylon_jobs_row(
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

fn format_job_msats(amount_msats: i64) -> String {
    let value = amount_msats.max(0);
    let sats = value / 1000;
    let remainder = value % 1000;
    if remainder == 0 {
        format!("{} sats", sats)
    } else {
        format!("{}.{:03} sats", sats, remainder)
    }
}

fn format_job_kind(kind: i64) -> String {
    if !(0..=u16::MAX as i64).contains(&kind) {
        return kind.to_string();
    }
    let label = crate::app::dvm::job_kind_label(kind as u16);
    if label == "Custom" {
        kind.to_string()
    } else {
        format!("{}({})", kind, label)
    }
}

fn format_job_status(status: &str) -> &str {
    match status {
        "pending" => "Pending",
        "processing" => "Processing",
        "completed" => "Completed",
        "failed" => "Failed",
        "cancelled" => "Cancelled",
        _ => status,
    }
}

fn job_status_color(status: &str, palette: &UiPalette) -> Hsla {
    match status {
        "pending" => Hsla::new(35.0, 0.8, 0.6, 1.0),
        "processing" => Hsla::new(210.0, 0.6, 0.6, 1.0),
        "completed" => Hsla::new(120.0, 0.6, 0.5, 1.0),
        "failed" => Hsla::new(0.0, 0.7, 0.55, 1.0),
        "cancelled" => palette.text_faint,
        _ => palette.text_secondary,
    }
}

fn short_job_id(value: &str) -> String {
    if value.len() > 8 {
        format!("{}...", &value[..8])
    } else {
        value.to_string()
    }
}

fn short_pylon_pubkey(value: &str) -> String {
    if value.len() > 8 {
        format!("{}...", &value[..8])
    } else {
        value.to_string()
    }
}
