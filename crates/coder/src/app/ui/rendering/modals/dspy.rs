fn draw_dspy_row(
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

fn render_dspy_modal(
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

            let modal_width = DSPY_MODAL_WIDTH;
            let modal_height = DSPY_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let snapshot = state.dspy.snapshot.clone();
            let mut y = modal_y + 16.0;

            let title_run = state.text_system.layout_styled_mono(
                "DSPy Status",
                Point::new(modal_x + 16.0, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 22.0;

            let line_height = 18.0;
            let section_gap = 10.0;
            let label_x = modal_x + 16.0;
            let value_x = modal_x + 210.0;
            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;

            let format_percent = |value: f32| format!("{:.1}%", value * 100.0);
            let format_timestamp = |value: Option<u64>| {
                if let Some(ts) = value {
                    if ts == 0 {
                        "Never".to_string()
                    } else {
                        format_relative_time(ts)
                    }
                } else {
                    "Never".to_string()
                }
            };

            let sessions_header = state.text_system.layout_styled_mono(
                "Sessions",
                Point::new(label_x, y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(sessions_header);
            y += line_height;

            if let Some(error) = &snapshot.sessions_error {
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
            } else if let Some(summary) = &snapshot.sessions {
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Total sessions",
                    &summary.total_sessions.to_string(),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Success / Failed / Interrupted",
                    &format!(
                        "{} / {} / {}",
                        summary.success_count, summary.failed_count, summary.interrupted_count
                    ),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Success rate",
                    &format_percent(summary.success_rate),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Index updated",
                    &format_timestamp(Some(summary.updated_ts)),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Last optimization",
                    &format_timestamp(summary.last_optimization_ts),
                    palette.text_secondary,
                );
                if summary.total_sessions == 0 {
                    let hint = state.text_system.layout_styled_mono(
                        "No DSPy sessions recorded yet.",
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(hint);
                    y += line_height;
                }
            } else {
                let empty_run = state.text_system.layout_styled_mono(
                    "No session data available.",
                    Point::new(label_x, y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
                y += line_height;
            }

            y += section_gap;
            let performance_header = state.text_system.layout_styled_mono(
                "Performance",
                Point::new(label_x, y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(performance_header);
            y += line_height;

            if let Some(error) = &snapshot.performance_error {
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
            } else if let Some(summary) = &snapshot.performance {
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Decisions tracked",
                    &format!("{} ({} correct)", summary.total_decisions, summary.total_correct),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Overall accuracy",
                    &format_percent(summary.overall_accuracy),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Complexity accuracy",
                    &format_percent(summary.complexity_accuracy),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Delegation accuracy",
                    &format_percent(summary.delegation_accuracy),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "RLM trigger accuracy",
                    &format_percent(summary.rlm_accuracy),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Optimization runs",
                    &summary.optimization_count.to_string(),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Metrics updated",
                    &format_timestamp(Some(summary.updated_ts)),
                    palette.text_secondary,
                );
                draw_dspy_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Last optimization",
                    &format_timestamp(summary.last_optimization_ts),
                    palette.text_secondary,
                );
                if summary.total_decisions == 0 {
                    let hint = state.text_system.layout_styled_mono(
                        "No decision accuracy data yet.",
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(hint);
                    y += line_height;
                }
            } else {
                let empty_run = state.text_system.layout_styled_mono(
                    "No performance data available.",
                    Point::new(label_x, y),
                    11.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
                y += line_height;
            }

            y += section_gap;
            let auto_header = state.text_system.layout_styled_mono(
                "Auto-Optimizer",
                Point::new(label_x, y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(auto_header);
            y += line_height;

            let auto_snapshot = &snapshot.auto_optimizer;
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Config source",
                auto_snapshot.source.label(),
                palette.text_secondary,
            );
            if let Some(error) = auto_snapshot.source.error() {
                for line in wrap_text(&format!("Error: {}", error), max_chars) {
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

            let config_path = auto_snapshot
                .config_path
                .as_ref()
                .map(|path| truncate_preview(&path.display().to_string(), 56))
                .unwrap_or_else(|| "Not available".to_string());
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Config path",
                &config_path,
                palette.text_secondary,
            );
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Enabled",
                if auto_snapshot.config.enabled { "On" } else { "Off" },
                palette.text_secondary,
            );
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Background optimization",
                if auto_snapshot.config.background_optimization {
                    "On"
                } else {
                    "Off"
                },
                palette.text_secondary,
            );
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Min labeled examples",
                &auto_snapshot.config.min_labeled_examples.to_string(),
                palette.text_secondary,
            );
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Accuracy threshold",
                &format_percent(auto_snapshot.config.accuracy_threshold),
                palette.text_secondary,
            );
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Min hours between",
                &auto_snapshot
                    .config
                    .min_hours_between_optimizations
                    .to_string(),
                palette.text_secondary,
            );
            draw_dspy_row(
                state,
                scene,
                palette,
                label_x,
                value_x,
                &mut y,
                line_height,
                "Candidates / Trials",
                &format!(
                    "{} / {}",
                    auto_snapshot.config.num_candidates, auto_snapshot.config.num_trials
                ),
                palette.text_secondary,
            );

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "R refresh • E toggle auto • B toggle background • Esc to close",
                Point::new(modal_x + 16.0, y),
                11.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
}
