/// Default width for the plan panel
const PLAN_PANEL_WIDTH: f32 = 230.0;
/// Minimum height for resizing
const PLAN_PANEL_MIN_HEIGHT: f32 = 140.0;
/// Maximum height for resizing
const PLAN_PANEL_MAX_HEIGHT: f32 = 420.0;
/// Padding inside the panel
const PLAN_PANEL_PADDING: f32 = 12.0;
/// Header height
const PLAN_HEADER_HEIGHT: f32 = 28.0;
/// Task row height
const PLAN_TASK_ROW_HEIGHT: f32 = 22.0;

/// Calculate the bounds for the plan panel (right sidebar)
fn plan_panel_bounds(window_width: f32, window_height: f32, panel_height: f32) -> Bounds {
    Bounds::new(
        window_width - PLAN_PANEL_WIDTH,
        window_height - panel_height,
        PLAN_PANEL_WIDTH,
        panel_height,
    )
}

/// Render the plan panel in the right sidebar
fn render_plan_panel(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    bounds: Bounds,
) {
    use crate::app::plan::TaskStatus;

    let Some(ref plan) = state.active_plan else {
        return;
    };

    // Render on layer 1 to appear above chat content
    scene.set_layer(1);

    // Background
    scene.draw_quad(
        Quad::new(bounds)
            .with_background(palette.chrome)
            .with_border(palette.panel_border, 1.0),
    );

    // Header
    let header_bounds = Bounds::new(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        PLAN_HEADER_HEIGHT,
    );
    scene.draw_quad(
        Quad::new(header_bounds)
            .with_background(palette.panel)
            .with_border(palette.panel_border, 1.0),
    );

    // Title and progress
    let completed = plan.completed_count();
    let total = plan.total_count();
    let title = if total > 0 {
        format!("Plan ({}/{})", completed, total)
    } else {
        "Plan".to_string()
    };

    let title_run = state.text_system.layout_styled_mono(
        &title,
        Point::new(
            header_bounds.origin.x + PLAN_PANEL_PADDING,
            header_bounds.origin.y + 8.0,
        ),
        13.0,
        palette.text_primary,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(title_run);

    // Progress bar (if tasks exist)
    if total > 0 {
        let progress_width = 60.0;
        let progress_height = 4.0;
        let progress_x = bounds.origin.x + bounds.size.width - progress_width - PLAN_PANEL_PADDING;
        let progress_y = header_bounds.origin.y + (PLAN_HEADER_HEIGHT - progress_height) / 2.0;

        // Background track
        scene.draw_quad(
            Quad::new(Bounds::new(progress_x, progress_y, progress_width, progress_height))
                .with_background(palette.panel_highlight)
                .with_corner_radius(progress_height / 2.0),
        );

        // Progress fill
        let fill_width = progress_width * plan.progress();
        if fill_width > 0.0 {
            scene.draw_quad(
                Quad::new(Bounds::new(progress_x, progress_y, fill_width, progress_height))
                    .with_background(palette.tool_progress_fg)
                    .with_corner_radius(progress_height / 2.0),
            );
        }
    }

    // Content area (scrollable)
    let content_y = bounds.origin.y + PLAN_HEADER_HEIGHT + 8.0;
    let _content_height = bounds.size.height - PLAN_HEADER_HEIGHT - 16.0;
    let content_width = bounds.size.width - PLAN_PANEL_PADDING * 2.0;

    // Explanation text (if present)
    let mut task_start_y = content_y;
    if let Some(ref explanation) = plan.explanation {
        let truncated = if explanation.len() > 200 {
            format!("{}...", &explanation[..200])
        } else {
            explanation.clone()
        };

        let explanation_run = state.text_system.layout_styled_mono(
            &truncated,
            Point::new(bounds.origin.x + PLAN_PANEL_PADDING, content_y),
            11.0,
            palette.text_secondary,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(explanation_run);

        // Calculate rough height (simple approximation)
        let lines = (truncated.len() as f32 / 30.0).ceil().max(1.0);
        task_start_y += lines * 14.0 + 8.0;
    }

    // Task list
    for (i, task) in plan.tasks.iter().enumerate() {
        let task_y = task_start_y + i as f32 * PLAN_TASK_ROW_HEIGHT;

        // Stop if we're outside visible area
        if task_y > bounds.origin.y + bounds.size.height - 8.0 {
            break;
        }

        // Status indicator
        let (indicator, indicator_color) = match task.status {
            TaskStatus::Pending => ("[ ]", palette.text_muted),
            TaskStatus::InProgress => ("[>]", palette.link),
            TaskStatus::Completed => ("[x]", palette.tool_progress_fg),
            TaskStatus::Failed => ("[!]", palette.prompt), // Use prompt color for errors (reddish)
        };

        let indicator_run = state.text_system.layout_styled_mono(
            indicator,
            Point::new(bounds.origin.x + PLAN_PANEL_PADDING, task_y),
            11.0,
            indicator_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(indicator_run);

        // Task description (truncated to fit)
        let desc_x = bounds.origin.x + PLAN_PANEL_PADDING + 28.0;
        let max_desc_chars = ((content_width - 28.0) / 7.0) as usize; // ~7px per char at 11px
        let desc = if task.description.len() > max_desc_chars {
            format!("{}...", &task.description[..max_desc_chars.saturating_sub(3)])
        } else {
            task.description.clone()
        };

        let desc_color = match task.status {
            TaskStatus::Completed => palette.text_muted,
            TaskStatus::Failed => palette.prompt, // Use prompt color for errors (reddish)
            _ => palette.text_secondary,
        };

        let desc_run = state.text_system.layout_styled_mono(
            &desc,
            Point::new(desc_x, task_y),
            11.0,
            desc_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(desc_run);
    }

    // Reset to layer 0
    scene.set_layer(0);
}
