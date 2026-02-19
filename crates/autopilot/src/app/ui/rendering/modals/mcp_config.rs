fn render_mcp_config_modal(
    state: &mut AppState,
    scene: &mut Scene,
    _palette: &UiPalette,
    bounds: Bounds,
    logical_width: f32,
    logical_height: f32,
    _scale_factor: f32,
    selected: &usize,
) {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let modal_width = 720.0;
            let modal_height = 420.0;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "MCP Servers",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let project_path = state
                .catalogs
                .mcp_project_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let project_line = format!("Project config: {}", project_path);
            let project_run = state.text_system.layout_styled_mono(
                &truncate_preview(&project_line, 90),
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(project_run);
            y += 18.0;

            if let Some(error) = &state.catalogs.mcp_project_error {
                let error_line = format!("Config warning: {}", error);
                let error_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&error_line, 100),
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(15.0, 0.7, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(error_run);
                y += 18.0;
            }

            if let Some(error) = &state.catalogs.mcp_status_error {
                let status_line = format!("Status warning: {}", error);
                let status_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&status_line, 100),
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(15.0, 0.7, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(status_run);
                y += 18.0;
            }

            let entries = state.catalogs.mcp_entries();
            let counts_line = format!(
                "Servers: {} project · {} runtime · {} disabled",
                state.catalogs.mcp_project_servers.len(),
                state.catalogs.mcp_runtime_servers.len(),
                state.catalogs.mcp_disabled_servers.len()
            );
            let counts_run = state.text_system.layout_styled_mono(
                &counts_line,
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(counts_run);
            y += 20.0;

            let list_top = y;
            let list_bottom = modal_y + modal_height - 48.0;
            let row_height = 22.0;
            let max_visible = ((list_bottom - list_top) / row_height).floor().max(0.0) as usize;
            if entries.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No MCP servers configured.",
                    Point::new(modal_x + 16.0, list_top),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let visible = entries.len().min(max_visible.max(1));
                let selected = (*selected).min(entries.len().saturating_sub(1));
                let mut start = selected.saturating_sub(visible / 2);
                if start + visible > entries.len() {
                    start = entries.len().saturating_sub(visible);
                }

                for idx in 0..visible {
                    let index = start + idx;
                    let entry = &entries[index];
                    let row_y = list_top + idx as f32 * row_height;
                    if index == selected {
                        let highlight = Quad::new(Bounds::new(
                            modal_x + 12.0,
                            row_y - 2.0,
                            modal_width - 24.0,
                            row_height,
                        ))
                        .with_background(Hsla::new(220.0, 0.2, 0.18, 1.0));
                        scene.draw_quad(highlight);
                    }

                    let source_label = match entry.source {
                        Some(McpServerSource::Project) => "project",
                        Some(McpServerSource::Runtime) => "runtime",
                        None => "status",
                    };
                    let mut line = format!("{} [{}]", entry.name, source_label);
                    if let Some(status) = &entry.status {
                        line.push_str(&format!(" · {}", status));
                    }
                    if entry.disabled {
                        line.push_str(" · disabled");
                    }
                    let line = truncate_preview(&line, 120);
                    let line_run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(modal_x + 20.0, row_y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.7, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(line_run);

                    if let Some(config) = &entry.config {
                        let detail = describe_mcp_config(config);
                        let detail = truncate_preview(&detail, 120);
                        let detail_run = state.text_system.layout_styled_mono(
                            &detail,
                            Point::new(modal_x + 260.0, row_y),
                            11.0,
                            Hsla::new(0.0, 0.0, 0.5, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(detail_run);
                    }
                }
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Enter/Esc to close · R reload · S status · Del disable",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
}
