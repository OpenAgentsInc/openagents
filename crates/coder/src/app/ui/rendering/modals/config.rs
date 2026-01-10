        ModalState::Config {
            tab,
            selected,
            search,
            input_mode,
        } => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let modal_width = SETTINGS_MODAL_WIDTH;
            let modal_height = SETTINGS_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg =
                Quad::new(modal_bounds).with_background(palette.panel).with_border(
                    palette.panel_border,
                    1.0,
                );
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "Settings",
                Point::new(modal_x + 16.0, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let search_label = if matches!(input_mode, SettingsInputMode::Search) {
                format!("Search: {}_", search)
            } else if search.trim().is_empty() {
                "Search: /".to_string()
            } else {
                format!("Search: {}", search)
            };
            let search_run = state.text_system.layout_styled_mono(
                &search_label,
                Point::new(modal_x + 16.0, y),
                12.0,
                if matches!(input_mode, SettingsInputMode::Search) {
                    palette.text_primary
                } else {
                    palette.text_muted
                },
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(search_run);
            y += 18.0;

            let tabs = SettingsTab::all();
            let mut tab_x = modal_x + 16.0;
            let tab_y = y;
            for entry in tabs {
                let label = entry.label();
                let tab_width = (label.len() as f32 * 7.0).max(48.0);
                if *entry == *tab {
                    let highlight = Quad::new(Bounds::new(
                        tab_x - 6.0,
                        tab_y - 2.0,
                        tab_width + 12.0,
                        SETTINGS_TAB_HEIGHT,
                    ))
                    .with_background(palette.panel_highlight);
                    scene.draw_quad(highlight);
                }
                let tab_run = state.text_system.layout_styled_mono(
                    label,
                    Point::new(tab_x, tab_y),
                    12.0,
                    if *entry == *tab {
                        palette.text_primary
                    } else {
                        palette.text_muted
                    },
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(tab_run);
                tab_x += tab_width + 16.0;
            }
            y += SETTINGS_TAB_HEIGHT + 8.0;

            let snapshot = SettingsSnapshot::from_state(state);
            let rows = settings_rows(&snapshot, *tab, search);
            let list_top = y;
            let list_bottom = modal_y + modal_height - 48.0;
            let max_visible =
                ((list_bottom - list_top) / SETTINGS_ROW_HEIGHT).floor().max(0.0) as usize;

            if rows.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No settings match this search.",
                    Point::new(modal_x + 16.0, list_top),
                    12.0,
                    palette.text_dim,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let visible = rows.len().min(max_visible.max(1));
                let selected = (*selected).min(rows.len().saturating_sub(1));
                let mut start = selected.saturating_sub(visible / 2);
                if start + visible > rows.len() {
                    start = rows.len().saturating_sub(visible);
                }
                let value_x = modal_x + modal_width * 0.55;
                let hint_x = modal_x + modal_width * 0.75;
                let capture_action = match input_mode {
                    SettingsInputMode::Capture(action) => Some(*action),
                    _ => None,
                };

                for idx in 0..visible {
                    let index = start + idx;
                    let row = &rows[index];
                    let row_y = list_top + idx as f32 * SETTINGS_ROW_HEIGHT;
                    let is_selected = index == selected;
                    if is_selected {
                        let highlight = Quad::new(Bounds::new(
                            modal_x + 12.0,
                            row_y - 2.0,
                            modal_width - 24.0,
                            SETTINGS_ROW_HEIGHT,
                        ))
                        .with_background(palette.panel_highlight);
                        scene.draw_quad(highlight);
                    }

                    let label_run = state.text_system.layout_styled_mono(
                        &row.label,
                        Point::new(modal_x + 20.0, row_y),
                        12.0,
                        if is_selected {
                            palette.text_primary
                        } else {
                            palette.text_muted
                        },
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(label_run);

                    let value_text = if let Some(action) = capture_action {
                        if is_selected
                            && matches!(row.item, SettingsItem::Keybinding(a) if a == action)
                        {
                            "Press keys...".to_string()
                        } else {
                            row.value.clone()
                        }
                    } else {
                        row.value.clone()
                    };
                    let value_run = state.text_system.layout_styled_mono(
                        &value_text,
                        Point::new(value_x, row_y),
                        12.0,
                        palette.text_secondary,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(value_run);

                    if let Some(hint) = &row.hint {
                        let hint_run = state.text_system.layout_styled_mono(
                            hint,
                            Point::new(hint_x, row_y),
                            11.0,
                            palette.text_faint,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(hint_run);
                    }
                }
            }

            y = modal_y + modal_height - 24.0;
            let footer_text = match input_mode {
                SettingsInputMode::Search => "Type to search · Enter/Esc to finish",
                SettingsInputMode::Capture(_) => "Press new shortcut · Esc to cancel",
                SettingsInputMode::Normal => {
                    "Tab to switch · / to search · Enter/Left/Right to change · Esc to close"
                }
            };
            let footer_run = state.text_system.layout_styled_mono(
                footer_text,
                Point::new(modal_x + 16.0, y),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
