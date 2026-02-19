/// Render the "Validating Issue" modal (shows a spinner/message while validation runs)
fn render_validating_issue_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    _logical_width: f32,
    _logical_height: f32,
    _scale_factor: f32,
    issue_number: u32,
    title: &str,
) {
    let modal_width = 400.0;
    let modal_height = 120.0;
    let modal_x = bounds.origin.x + (bounds.size.width - modal_width) / 2.0;
    let modal_y = bounds.origin.y + (bounds.size.height - modal_height) / 2.0;
    let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

    // Layer 2 for modals
    scene.set_layer(2);

    // Modal background with shadow
    scene.draw_quad(
        Quad::new(Bounds::new(
            modal_x + 4.0,
            modal_y + 4.0,
            modal_width,
            modal_height,
        ))
        .with_background(Hsla::new(0.0, 0.0, 0.0, 0.3))
        .with_corner_radius(8.0),
    );

    // Modal panel
    scene.draw_quad(
        Quad::new(modal_bounds)
            .with_background(palette.panel)
            .with_border(palette.panel_border, 1.0)
            .with_corner_radius(8.0),
    );

    // Title
    let title_y = modal_y + 20.0;
    let title_text = format!("Validating Issue #{}", issue_number);
    let title_run = state.text_system.layout_styled_mono(
        &title_text,
        Point::new(modal_x + 20.0, title_y),
        14.0,
        palette.text_primary,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(title_run);

    // Issue title (truncated)
    let issue_title_y = title_y + 24.0;
    let truncated_title = if title.len() > 50 {
        format!("{}...", &title[..47])
    } else {
        title.to_string()
    };
    let issue_run = state.text_system.layout_styled_mono(
        &truncated_title,
        Point::new(modal_x + 20.0, issue_title_y),
        12.0,
        palette.text_secondary,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(issue_run);

    // Status message
    let status_y = issue_title_y + 28.0;
    let status_run = state.text_system.layout_styled_mono(
        "Checking if issue is still valid...",
        Point::new(modal_x + 20.0, status_y),
        11.0,
        palette.text_muted,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(status_run);

    scene.set_layer(0);
}

/// Render the "Issue Validation Failed" warning modal
fn render_issue_validation_failed_modal(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
    _logical_width: f32,
    _logical_height: f32,
    _scale_factor: f32,
    issue_number: u32,
    title: &str,
    reason: &str,
) {
    let modal_width = 480.0;
    let modal_height = 200.0;
    let modal_x = bounds.origin.x + (bounds.size.width - modal_width) / 2.0;
    let modal_y = bounds.origin.y + (bounds.size.height - modal_height) / 2.0;
    let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

    // Layer 2 for modals
    scene.set_layer(2);

    // Modal background with shadow
    scene.draw_quad(
        Quad::new(Bounds::new(
            modal_x + 4.0,
            modal_y + 4.0,
            modal_width,
            modal_height,
        ))
        .with_background(Hsla::new(0.0, 0.0, 0.0, 0.3))
        .with_corner_radius(8.0),
    );

    // Modal panel
    scene.draw_quad(
        Quad::new(modal_bounds)
            .with_background(palette.panel)
            .with_border(palette.panel_border, 1.0)
            .with_corner_radius(8.0),
    );

    // Warning icon and title
    let title_y = modal_y + 20.0;
    let warning_text = format!("Issue #{} may already be addressed", issue_number);
    let title_run = state.text_system.layout_styled_mono(
        &warning_text,
        Point::new(modal_x + 20.0, title_y),
        14.0,
        palette.prompt, // Use prompt color (warning/orange)
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(title_run);

    // Issue title (truncated)
    let issue_title_y = title_y + 24.0;
    let truncated_title = if title.len() > 60 {
        format!("{}...", &title[..57])
    } else {
        title.to_string()
    };
    let issue_run = state.text_system.layout_styled_mono(
        &truncated_title,
        Point::new(modal_x + 20.0, issue_title_y),
        12.0,
        palette.text_secondary,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(issue_run);

    // Reason (wrapped)
    let reason_y = issue_title_y + 28.0;
    let reason_lines = wrap_text(reason, 65); // ~65 chars per line
    for (i, line) in reason_lines.iter().take(3).enumerate() {
        let line_run = state.text_system.layout_styled_mono(
            line,
            Point::new(modal_x + 20.0, reason_y + i as f32 * 16.0),
            11.0,
            palette.text_muted,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(line_run);
    }

    // Actions
    let actions_y = modal_y + modal_height - 36.0;
    let actions_text = "[P]roceed anyway   [S]kip   [C]ancel";
    let actions_run = state.text_system.layout_styled_mono(
        actions_text,
        Point::new(modal_x + 20.0, actions_y),
        12.0,
        palette.text_secondary,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(actions_run);

    scene.set_layer(0);
}
