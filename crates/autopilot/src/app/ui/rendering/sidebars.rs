fn render_sidebars(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
) {
    // Sidebar background color #0a0a0a
    let sidebar_bg = Hsla::new(0.0, 0.0, 0.039, 1.0);

    if let Some(left_bounds) = sidebar_layout.left {
        scene.draw_quad(
            Quad::new(left_bounds)
                .with_background(sidebar_bg)
                .with_border(palette.panel_border, 1.0),
        );

        // New Session button
        let btn_bounds = new_session_button_bounds(left_bounds);
        let btn_bg = if state.new_session_button_hovered {
            Hsla::new(0.0, 0.0, 0.15, 1.0)
        } else {
            Hsla::new(0.0, 0.0, 0.1, 1.0)
        };
        scene.draw_quad(
            Quad::new(btn_bounds)
                .with_background(btn_bg)
                .with_corner_radius(4.0),
        );
        let btn_text_y = btn_bounds.origin.y + (btn_bounds.size.height - 12.0) / 2.0;
        let btn_run = state.text_system.layout_styled_mono(
            "+ New Session",
            Point::new(btn_bounds.origin.x + 12.0, btn_text_y),
            12.0,
            palette.text_primary,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(btn_run);
    }

    if let Some(right_bounds) = sidebar_layout.right {
        scene.draw_quad(
            Quad::new(right_bounds)
                .with_background(sidebar_bg)
                .with_border(palette.panel_border, 1.0),
        );

        // Usage display
        let padding = 12.0;
        let mut y = right_bounds.origin.y + padding;
        let x = right_bounds.origin.x + padding;
        let w = right_bounds.size.width - padding * 2.0;

        let label_color = Hsla::new(0.0, 0.0, 0.5, 1.0);
        let value_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
        let muted_color = Hsla::new(0.0, 0.0, 0.4, 1.0);
        let font_size = 10.0;
        let line_height = 14.0;

        // Header
        let header = state.text_system.layout_styled_mono(
            "SESSION USAGE",
            Point::new(x, y),
            font_size,
            label_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(header);
        y += line_height + 8.0;

        // Model
        let model_text = &state.session.session_info.model;
        if !model_text.is_empty() {
            let model_run = state.text_system.layout_styled_mono(
                model_text,
                Point::new(x, y),
                11.0,
                value_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(model_run);
            y += line_height + 8.0;
        }

        // Cost and turns
        let cost_text = format!("${:.4}", state.session.session_usage.total_cost_usd);
        let cost_run = state.text_system.layout_styled_mono(
            &cost_text,
            Point::new(x, y),
            11.0,
            value_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(cost_run);

        let turns_text = format!("{} turns", state.session.session_usage.num_turns);
        let turns_run = state.text_system.layout_styled_mono(
            &turns_text,
            Point::new(x + 70.0, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(turns_run);
        y += line_height + 8.0;

        // Tokens
        let in_text = format!(
            "{} in",
            format_tokens(state.session.session_usage.input_tokens)
        );
        let in_run = state.text_system.layout_styled_mono(
            &in_text,
            Point::new(x, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(in_run);

        let out_text = format!(
            "{} out",
            format_tokens(state.session.session_usage.output_tokens)
        );
        let out_run = state.text_system.layout_styled_mono(
            &out_text,
            Point::new(x + w / 2.0, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(out_run);
        y += line_height + 4.0;

        // Duration
        let dur_text = format_duration_ms(state.session.session_usage.duration_ms);
        let dur_run = state.text_system.layout_styled_mono(
            &dur_text,
            Point::new(x, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(dur_run);
        y += line_height + 16.0;

        // Rate limits section
        let green_color = Hsla::new(0.389, 0.7, 0.5, 1.0);

        // Render each rate limit
        let rate_limits_to_render: Vec<_> = [
            state.session.rate_limits.primary.clone(),
            state.session.rate_limits.secondary.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();

        if !rate_limits_to_render.is_empty() {
            let header = state.text_system.layout_styled_mono(
                "RATE LIMITS",
                Point::new(x, y),
                font_size,
                label_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(header);
            y += line_height + 4.0;

            for limit in rate_limits_to_render {
                // Limit name and percentage
                let limit_text = format!("{} {:.0}%", limit.name, limit.percent_used);
                let limit_run = state.text_system.layout_styled_mono(
                    &limit_text,
                    Point::new(x, y),
                    font_size,
                    muted_color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(limit_run);
                y += line_height;

                // Progress bar
                let bar_h = 4.0;
                scene.draw_quad(
                    Quad::new(Bounds::new(x, y, w, bar_h))
                        .with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)),
                );
                let bar_color = if limit.percent_used < 50.0 {
                    green_color
                } else if limit.percent_used < 75.0 {
                    Hsla::new(0.125, 0.8, 0.5, 1.0) // yellow
                } else {
                    Hsla::new(0.0, 0.8, 0.5, 1.0) // red
                };
                let fill_w = (w * limit.percent_used as f32 / 100.0).min(w);
                scene.draw_quad(
                    Quad::new(Bounds::new(x, y, fill_w, bar_h)).with_background(bar_color),
                );
                y += bar_h + 2.0;

                // Reset time
                if !limit.resets_at.is_empty() {
                    let reset_text = format!("resets {}", limit.resets_at);
                    let reset_run = state.text_system.layout_styled_mono(
                        &reset_text,
                        Point::new(x, y),
                        9.0,
                        muted_color,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(reset_run);
                    y += line_height + 4.0;
                }
            }
        }
    }
}

