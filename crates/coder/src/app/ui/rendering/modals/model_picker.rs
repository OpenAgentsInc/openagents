        ModalState::ModelPicker { selected } => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            // Semi-transparent overlay
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            // Modal box
            let modal_width = 700.0;
            let modal_height = 200.0;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            // Modal background
            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;

            // Title
            let title_run = state.text_system.layout_styled_mono(
                "Select model",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0), // White
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            // Description
            let desc_run = state.text_system.layout_styled_mono(
                "Switch between Claude models. Applies to this session and future Claude Code sessions.",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);
            y += 30.0;

            // Model options
            let models = ModelOption::all();
            for (i, model) in models.iter().enumerate() {
                let is_selected = i == *selected;
                let is_current = *model == state.settings.selected_model;

                // Selection indicator
                let indicator = if is_selected { ">" } else { " " };
                let indicator_run = state.text_system.layout_styled_mono(
                    indicator,
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(indicator_run);

                // Number
                let num_text = format!("{}.", i + 1);
                let num_run = state.text_system.layout_styled_mono(
                    &num_text,
                    Point::new(modal_x + 32.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(num_run);

                // Name
                let name_color = if is_selected {
                    Hsla::new(120.0, 0.6, 0.6, 1.0) // Green for selected
                } else {
                    Hsla::new(0.0, 0.0, 0.7, 1.0) // White-ish
                };
                let name_run = state.text_system.layout_styled_mono(
                    model.name(),
                    Point::new(modal_x + 56.0, y),
                    14.0,
                    name_color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(name_run);

                // Checkmark if current
                if is_current {
                    let check_run = state.text_system.layout_styled_mono(
                        "✓",
                        Point::new(modal_x + 220.0, y),
                        14.0,
                        Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(check_run);
                }

                // Description
                let desc_run = state.text_system.layout_styled_mono(
                    model.description(),
                    Point::new(modal_x + 240.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(desc_run);

                y += 24.0;
            }

            // Footer
            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Enter to confirm · Esc to exit",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0), // Dim gray
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        },
