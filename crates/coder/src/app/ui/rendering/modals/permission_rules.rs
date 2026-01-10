        ModalState::PermissionRules => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let modal_width = 560.0;
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
                "Permission rules",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let default_text = if state.permissions.permission_default_allow {
                "Default: allow"
            } else {
                "Default: deny"
            };
            let default_run = state.text_system.layout_styled_mono(
                default_text,
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(default_run);
            y += 22.0;

            let allow_label = state.text_system.layout_styled_mono(
                "Allow:",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(120.0, 0.6, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(allow_label);
            let allow_text = if state.permissions.permission_allow_tools.is_empty() {
                "None".to_string()
            } else {
                state.permissions.permission_allow_tools.join(", ")
            };
            let allow_run = state.text_system.layout_styled_mono(
                &allow_text,
                Point::new(modal_x + 80.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(allow_run);
            y += 22.0;

            let deny_label = state.text_system.layout_styled_mono(
                "Deny:",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.6, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(deny_label);
            let deny_text = if state.permissions.permission_deny_tools.is_empty() {
                "None".to_string()
            } else {
                state.permissions.permission_deny_tools.join(", ")
            };
            let deny_run = state.text_system.layout_styled_mono(
                &deny_text,
                Point::new(modal_x + 80.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(deny_run);

            y += 22.0;
            let bash_allow_label = state.text_system.layout_styled_mono(
                "Bash allow:",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(120.0, 0.6, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(bash_allow_label);
            let bash_allow_text = if state.permissions.permission_allow_bash_patterns.is_empty() {
                "None".to_string()
            } else {
                state.permissions.permission_allow_bash_patterns.join(", ")
            };
            let bash_allow_run = state.text_system.layout_styled_mono(
                &bash_allow_text,
                Point::new(modal_x + 120.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(bash_allow_run);
            y += 22.0;

            let bash_deny_label = state.text_system.layout_styled_mono(
                "Bash deny:",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.6, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(bash_deny_label);
            let bash_deny_text = if state.permissions.permission_deny_bash_patterns.is_empty() {
                "None".to_string()
            } else {
                state.permissions.permission_deny_bash_patterns.join(", ")
            };
            let bash_deny_run = state.text_system.layout_styled_mono(
                &bash_deny_text,
                Point::new(modal_x + 120.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(bash_deny_run);
            y += 26.0;

            let history_title = state.text_system.layout_styled_mono(
                "Recent decisions:",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.85, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(history_title);
            y += 18.0;

            if state.permissions.permission_history.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No recent permission decisions.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                for entry in state.permissions.permission_history.iter().rev().take(5) {
                    let mut line =
                        format!("@{} [{}] {}", entry.timestamp, entry.decision, entry.tool_name);
                    if let Some(detail) = &entry.detail {
                        if !detail.trim().is_empty() {
                            line.push_str(" - ");
                            line.push_str(detail);
                        }
                    }
                    let line = truncate_preview(&line, 120);
                    let entry_run = state.text_system.layout_styled_mono(
                        &line,
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(entry_run);
                    y += 18.0;
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
        },
