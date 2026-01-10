use crate::app::directives::{
    directive_priority_label, directive_status, directive_status_label, sort_workspace_directives,
    DirectiveStatus,
};

fn render_directives_modal(
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

            let modal_width = DIRECTIVES_MODAL_WIDTH;
            let modal_height = DIRECTIVES_MODAL_HEIGHT;
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
                "Workspace Directives",
                Point::new(label_x, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            if state.autopilot.oanix_manifest_rx.is_some() {
                let pending_run = state.text_system.layout_styled_mono(
                    "OANIX refresh in progress...",
                    Point::new(label_x, y),
                    12.0,
                    palette.text_muted,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(pending_run);
                y += line_height;
            }

            let workspace = state
                .autopilot
                .oanix_manifest
                .as_ref()
                .and_then(|manifest| manifest.workspace.clone());

            if let Some(workspace) = workspace {
                let project_name = workspace.project_name.as_deref().unwrap_or("Unknown");
                draw_directive_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Project",
                    project_name,
                    palette.text_secondary,
                );

                let root_text = truncate_preview(&workspace.root.display().to_string(), 54);
                draw_directive_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Root",
                    &root_text,
                    palette.text_secondary,
                );

                let active_summary = workspace
                    .active_directive
                    .as_ref()
                    .and_then(|id| workspace.directives.iter().find(|d| &d.id == id))
                    .map(|directive| format!("{} - {}", directive.id, directive.title))
                    .unwrap_or_else(|| "None".to_string());
                let active_summary = truncate_preview(&active_summary, 54);
                draw_directive_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Active",
                    &active_summary,
                    palette.text_secondary,
                );

                let mut active = 0;
                let mut paused = 0;
                let mut completed = 0;
                let mut other = 0;
                for directive in &workspace.directives {
                    match directive_status(directive) {
                        DirectiveStatus::Active => active += 1,
                        DirectiveStatus::Paused => paused += 1,
                        DirectiveStatus::Completed => completed += 1,
                        DirectiveStatus::Other => other += 1,
                    }
                }

                draw_directive_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Directives",
                    &workspace.directives.len().to_string(),
                    palette.text_secondary,
                );
                draw_directive_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Active",
                    &active.to_string(),
                    Hsla::new(120.0, 0.6, 0.5, 1.0),
                );
                draw_directive_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Paused",
                    &paused.to_string(),
                    Hsla::new(35.0, 0.8, 0.6, 1.0),
                );
                draw_directive_row(
                    state,
                    scene,
                    palette,
                    label_x,
                    value_x,
                    &mut y,
                    line_height,
                    "Completed",
                    &completed.to_string(),
                    palette.text_faint,
                );
                if other > 0 {
                    draw_directive_row(
                        state,
                        scene,
                        palette,
                        label_x,
                        value_x,
                        &mut y,
                        line_height,
                        "Other",
                        &other.to_string(),
                        palette.text_secondary,
                    );
                }

                y += 6.0;
                let list_header = state.text_system.layout_styled_mono(
                    "Directive List",
                    Point::new(label_x, y),
                    12.0,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(list_header);
                y += line_height;

                let list_bottom = modal_y + modal_height - 36.0;
                let mut shown = 0usize;

                for directive in sort_workspace_directives(&workspace.directives) {
                    if y + line_height > list_bottom {
                        break;
                    }
                    let status_label = directive_status_label(directive);
                    let priority_label = directive_priority_label(directive.priority.as_deref());
                    let progress_text = directive
                        .progress_pct
                        .map(|pct| format!("{}%", pct))
                        .unwrap_or_else(|| "-".to_string());
                    let mut line = format!(
                        "{} [{}] [{}] [{}] {}",
                        &directive.id,
                        status_label,
                        priority_label,
                        progress_text,
                        &directive.title
                    );
                    line = truncate_preview(&line, max_chars);
                    let color = directive_status_color(directive_status(directive), palette);
                    let run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(label_x, y),
                        11.0,
                        color,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(run);
                    y += line_height;
                    shown += 1;
                }

                let remaining = workspace.directives.len().saturating_sub(shown);
                if remaining > 0 && y + line_height <= list_bottom {
                    let more_text = format!("... {} more directives", remaining);
                    let more_run = state.text_system.layout_styled_mono(
                        &more_text,
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(more_run);
                } else if workspace.directives.is_empty() {
                    let empty_run = state.text_system.layout_styled_mono(
                        "No directives found in .openagents/directives",
                        Point::new(label_x, y),
                        11.0,
                        palette.text_faint,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                }
            } else {
                let info = "No workspace context found. Open a repo with .openagents/ to view directives.";
                for line in wrap_text(info, max_chars) {
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

            let footer = state.text_system.layout_styled_mono(
                "R refresh • Esc close",
                Point::new(label_x, modal_y + modal_height - 24.0),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer);
}

fn draw_directive_row(
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

fn directive_status_color(status: DirectiveStatus, palette: &UiPalette) -> Hsla {
    match status {
        DirectiveStatus::Active => Hsla::new(120.0, 0.6, 0.5, 1.0),
        DirectiveStatus::Paused => Hsla::new(35.0, 0.8, 0.6, 1.0),
        DirectiveStatus::Completed => palette.text_faint,
        DirectiveStatus::Other => palette.text_secondary,
    }
}
