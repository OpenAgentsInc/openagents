        ModalState::Hooks { view, selected } => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let modal_width = HOOK_MODAL_WIDTH;
            let modal_height = HOOK_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "Hooks",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let view_label = match view {
                HookModalView::Config => "Config",
                HookModalView::Events => "Events",
            };
            let view_line = format!("View: {} (Tab to switch)", view_label);
            let view_run = state.text_system.layout_styled_mono(
                &view_line,
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(view_run);
            y += 18.0;

            match view {
                HookModalView::Config => {
                    let desc_run = state.text_system.layout_styled_mono(
                        "Configure built-in hooks and review loaded scripts.",
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(desc_run);
                    y += 20.0;

                    let config_lines = [
                        (
                            HookSetting::ToolBlocker,
                            "ToolBlocker",
                            state.catalogs.hook_config.tool_blocker,
                        ),
                        (
                            HookSetting::ToolLogger,
                            "ToolLogger",
                            state.catalogs.hook_config.tool_logger,
                        ),
                        (
                            HookSetting::OutputTruncator,
                            "OutputTruncator",
                            state.catalogs.hook_config.output_truncator,
                        ),
                        (
                            HookSetting::ContextInjection,
                            "ContextInjection",
                            state.catalogs.hook_config.context_injection,
                        ),
                        (
                            HookSetting::TodoEnforcer,
                            "TodoEnforcer",
                            state.catalogs.hook_config.todo_enforcer,
                        ),
                    ];

                    for (idx, (_setting, label, enabled)) in config_lines.iter().enumerate() {
                        let marker = if *enabled { "[x]" } else { "[ ]" };
                        let line = format!("{}. {} {}", idx + 1, marker, label);
                        let line_run = state.text_system.layout_styled_mono(
                            &line,
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            if *enabled {
                                Hsla::new(120.0, 0.6, 0.6, 1.0)
                            } else {
                                Hsla::new(0.0, 0.0, 0.6, 1.0)
                            },
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(line_run);
                        y += 18.0;
                    }

                    y += 4.0;
                    let project_path = state
                        .catalogs
                        .hook_project_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let project_line = format!("Project hooks: {}", project_path);
                    let project_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&project_line, 90),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(project_run);
                    y += 18.0;

                    if let Some(user_path) = &state.catalogs.hook_user_path {
                        let user_line = format!("User hooks: {}", user_path.display());
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

                    if let Some(error) = &state.catalogs.hook_load_error {
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

                    let script_count = state.catalogs.hook_scripts.len();
                    let scripts_line = format!("Scripts: {}", script_count);
                    let scripts_run = state.text_system.layout_styled_mono(
                        &scripts_line,
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(scripts_run);
                    y += 18.0;

                    let list_top = y;
                    let list_bottom = modal_y + modal_height - 48.0;
                    let row_height = 18.0;
                    let max_rows = ((list_bottom - list_top) / row_height).floor().max(0.0) as usize;
                    if script_count == 0 {
                        let empty_run = state.text_system.layout_styled_mono(
                            "No hook scripts found.",
                            Point::new(modal_x + 16.0, list_top),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.5, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(empty_run);
                    } else {
                        for (idx, script) in state.catalogs.hook_scripts.iter().take(max_rows).enumerate()
                        {
                            let source_label = match script.source {
                                HookScriptSource::Project => "project",
                                HookScriptSource::User => "user",
                            };
                            let matcher = script
                                .matcher
                                .as_ref()
                                .map(|matcher| format!(" ({})", matcher))
                                .unwrap_or_default();
                            let line = format!(
                                "- {}{} · {} · {}",
                                hook_event_label(script.event),
                                matcher,
                                source_label,
                                script.path.display()
                            );
                            let line_run = state.text_system.layout_styled_mono(
                                &truncate_preview(&line, 120),
                                Point::new(modal_x + 16.0, list_top + idx as f32 * row_height),
                                12.0,
                                Hsla::new(0.0, 0.0, 0.55, 1.0),
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(line_run);
                        }
                    }

                    y = modal_y + modal_height - 24.0;
                    let footer_run = state.text_system.layout_styled_mono(
                        "1-5 toggle · Tab for events · R to reload · Esc to exit",
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.4, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(footer_run);
                }
                HookModalView::Events => {
                    let desc_run = state.text_system.layout_styled_mono(
                        "Hook callbacks executed during the current session.",
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(desc_run);

                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        *selected,
                    );

                    if state.catalogs.hook_event_log.is_empty() {
                        let empty_run = state.text_system.layout_styled_mono(
                            "No hook events logged yet.",
                            Point::new(modal_x + 16.0, layout.list_bounds.origin.y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.5, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(empty_run);
                    } else {
                        let selected = (*selected).min(state.catalogs.hook_event_log.len().saturating_sub(1));
                        for (index, bounds) in &layout.row_bounds {
                            if let Some(entry) = state.catalogs.hook_event_log.get(*index) {
                                if *index == selected {
                                    let highlight = Quad::new(*bounds)
                                        .with_background(Hsla::new(220.0, 0.2, 0.18, 1.0));
                                    scene.draw_quad(highlight);
                                }
                                let timestamp = format_relative_time(entry.timestamp);
                                let mut label =
                                    format!("{} · {}", timestamp, hook_event_label(entry.event));
                                if let Some(tool) = &entry.tool_name {
                                    label.push_str(" · ");
                                    label.push_str(tool);
                                }
                                let label_run = state.text_system.layout_styled_mono(
                                    &truncate_preview(&label, 42),
                                    Point::new(bounds.origin.x + 6.0, bounds.origin.y + 4.0),
                                    11.0,
                                    Hsla::new(0.0, 0.0, 0.7, 1.0),
                                    wgpui::text::FontStyle::default(),
                                );
                                scene.draw_text(label_run);
                            }
                        }

                        if state.catalogs.hook_inspector.is_none() {
                            state.sync_hook_inspector(selected);
                        }
                        if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                            let mut paint_cx =
                                PaintContext::new(scene, &mut state.text_system, scale_factor);
                            inspector.paint(layout.inspector_bounds, &mut paint_cx);
                        }
                    }

                    y = modal_y + modal_height - 24.0;
                    let footer_run = state.text_system.layout_styled_mono(
                        "Up/Down to select · Tab for config · C to clear · Esc to exit",
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.4, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(footer_run);
                }
            }
        },
