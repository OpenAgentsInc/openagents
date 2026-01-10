        ModalState::AgentList { selected } => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            // Semi-transparent overlay
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let modal_width = SESSION_MODAL_WIDTH;
            let modal_height = SESSION_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "Agents",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let desc_run = state.text_system.layout_styled_mono(
                "Select an agent to focus the next request.",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);
            y += 18.0;

            if let Some(active) = &state.catalogs.active_agent {
                let active_line = format!("Active agent: {}", active);
                let active_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&active_line, 90),
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(120.0, 0.6, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(active_run);
                y += 18.0;
            }

            let project_path = state
                .catalogs
                .agent_project_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let project_line = format!("Project agents: {}", project_path);
            let project_run = state.text_system.layout_styled_mono(
                &truncate_preview(&project_line, 90),
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(project_run);
            y += 18.0;

            if let Some(user_path) = &state.catalogs.agent_user_path {
                let user_line = format!("User agents: {}", user_path.display());
                let user_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&user_line, 90),
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(user_run);
                y += 18.0;
            }

            if let Some(error) = &state.catalogs.agent_load_error {
                let error_line = format!("Load warning: {}", error);
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

            let project_count = state
                .catalogs
                .agent_entries
                .iter()
                .filter(|entry| entry.source == AgentSource::Project)
                .count();
            let user_count = state
                .catalogs
                .agent_entries
                .iter()
                .filter(|entry| entry.source == AgentSource::User)
                .count();
            let counts_line = format!("Agents: {} project · {} user", project_count, user_count);
            let counts_run = state.text_system.layout_styled_mono(
                &counts_line,
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(counts_run);

            let list_top = agent_modal_content_top(modal_y, state);
            let layout = agent_list_layout(
                logical_width,
                logical_height,
                state.catalogs.agent_entries.len(),
                *selected,
                list_top,
            );

            if state.catalogs.agent_entries.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No agents found.",
                    Point::new(modal_x + 16.0, list_top),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let selected = (*selected).min(state.catalogs.agent_entries.len().saturating_sub(1));
                let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
                for (index, bounds) in &layout.card_bounds {
                    if let Some(card) = state.catalogs.agent_cards.get_mut(*index) {
                        card.paint(*bounds, &mut paint_cx);
                    }
                    if *index == selected {
                        let outline =
                            Quad::new(*bounds).with_border(Hsla::new(120.0, 0.6, 0.5, 1.0), 1.0);
                        paint_cx.scene.draw_quad(outline);
                    }
                }
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Enter to activate · R to reload · Esc to exit",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
