fn render_tool_list_modal(
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

            let tools = &state.session.session_info.tools;
            // Semi-transparent overlay
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let modal_width = 520.0;
            let modal_height = 320.0;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "Tools",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let desc_run = state.text_system.layout_styled_mono(
                "Available tools from the active session.",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);
            y += 26.0;

            if tools.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No tool data yet.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let selected = (*selected).min(tools.len().saturating_sub(1));
                for (i, tool) in tools.iter().take(12).enumerate() {
                    let is_selected = i == selected;
                    let indicator = if is_selected { ">" } else { " " };
                    let indicator_run = state.text_system.layout_styled_mono(
                        indicator,
                        Point::new(modal_x + 16.0, y),
                        13.0,
                        Hsla::new(120.0, 0.6, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(indicator_run);

                    let mut label = tool.clone();
                    if state.permissions.tools_disallowed.iter().any(|t| t == tool) {
                        label.push_str(" (disabled)");
                    } else if state.permissions.tools_allowed.iter().any(|t| t == tool) {
                        label.push_str(" (enabled)");
                    }

                    let label_run = state.text_system.layout_styled_mono(
                        &label,
                        Point::new(modal_x + 32.0, y),
                        13.0,
                        if is_selected {
                            Hsla::new(120.0, 0.6, 0.6, 1.0)
                        } else {
                            Hsla::new(0.0, 0.0, 0.7, 1.0)
                        },
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(label_run);
                    y += 20.0;
                }
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Enter to close · Esc to exit",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
}
