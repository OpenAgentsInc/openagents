impl AppState {
    pub(crate) fn build_input_layout(
        &mut self,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
    ) -> InputLayout {
        let available_input_width = sidebar_layout.main.size.width - CONTENT_PADDING_X * 2.0;
        let input_width = available_input_width.max(0.0);
        let text_width = (input_width - COMPOSER_SEND_WIDTH - COMPOSER_SEND_GAP).max(0.0);
        let input_x = sidebar_layout.main.origin.x + CONTENT_PADDING_X;
        self.input.set_max_width(text_width);
        let input_height = self.input.current_height().max(40.0);

        let input_y = logical_height
            - STATUS_BAR_HEIGHT
            - INPUT_PADDING
            - COMPOSER_BAR_HEIGHT
            - COMPOSER_BAR_GAP
            - input_height;
        let input_bounds = Bounds::new(input_x, input_y, text_width, input_height);
        let send_bounds = Bounds::new(
            input_bounds.origin.x + text_width + COMPOSER_SEND_GAP,
            input_y,
            COMPOSER_SEND_WIDTH,
            input_height,
        );
        let bar_bounds = Bounds::new(
            input_x,
            input_y + input_height + COMPOSER_BAR_GAP,
            input_width,
            COMPOSER_BAR_HEIGHT,
        );
        let area_y = (input_y - INPUT_PADDING).max(0.0);
        let area_bounds = Bounds::new(
            sidebar_layout.main.origin.x,
            area_y,
            sidebar_layout.main.size.width,
            logical_height - area_y,
        );

        InputLayout {
            area_bounds,
            input_bounds,
            send_bounds,
            bar_bounds,
        }
    }
}

#[allow(dead_code)]
fn render_input(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
    _logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) {
    if state.git.center_mode == crate::app::CenterMode::Diff {
        return;
    }

    let input_layout = state.build_input_layout(sidebar_layout, logical_height);
    let composer_disabled = state.workspaces.active_thread_is_reviewing();
    let composer_labels = state.workspaces.composer_labels();
    let labels = ComposerLabels {
        model: format!("{} ▾", composer_labels.model),
        effort: format!("{} ▾", composer_labels.effort),
        access: format!("{} ▾", composer_labels.access),
        skill: format!("{} ▾", composer_labels.skill),
    };

    scene.draw_quad(Quad::new(input_layout.area_bounds).with_background(palette.chrome));
    scene.draw_quad(
        Quad::new(Bounds::new(
            sidebar_layout.main.origin.x,
            input_layout.area_bounds.origin.y,
            sidebar_layout.main.size.width,
            1.0,
        ))
        .with_background(palette.panel_border),
    );

    let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
    state.input.paint(input_layout.input_bounds, &mut paint_cx);

    let prompt_font = state.settings.coder_settings.font_size;
    let line_height = prompt_font * 1.4;
    let cursor_line = state.input.cursor_line();
    let prompt_y = input_layout.input_bounds.origin.y + 8.0
        + line_height * cursor_line as f32
        + prompt_font * 0.15;
    let prompt_color = if composer_disabled {
        palette.text_dim
    } else {
        palette.prompt
    };
    let prompt_run = state.text_system.layout_styled_mono(
        ">",
        Point::new(input_layout.input_bounds.origin.x + 12.0, prompt_y),
        prompt_font,
        prompt_color,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(prompt_run);

    let input_value = state.input.get_value();
    if input_value.trim().is_empty() {
        let placeholder_text = if composer_disabled {
            "Review in progress. Chat will re-enable when it completes."
        } else {
            "Ask Codex to do something..."
        };
        let placeholder_run = state.text_system.layout_styled_mono(
            placeholder_text,
            Point::new(
                input_layout.input_bounds.origin.x + 32.0,
                input_layout.input_bounds.origin.y + 10.0,
            ),
            prompt_font,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(placeholder_run);
    }

    let send_bg = if composer_disabled {
        palette.panel
    } else {
        palette.panel_highlight
    };
    scene.draw_quad(
        Quad::new(input_layout.send_bounds)
            .with_background(send_bg)
            .with_border(palette.panel_border, 1.0)
            .with_corner_radius(8.0),
    );
    let send_text_color = if composer_disabled {
        palette.text_dim
    } else {
        palette.text_primary
    };
    let send_run = state.text_system.layout_styled_mono(
        "Send",
        Point::new(
            input_layout.send_bounds.origin.x + 12.0,
            input_layout.send_bounds.origin.y + 10.0,
        ),
        12.0,
        send_text_color,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(send_run);

    let bar_layout = composer_bar_layout(&mut state.text_system, input_layout.bar_bounds, &labels);
    let pill_color = if composer_disabled {
        palette.panel
    } else {
        palette.panel_highlight
    };
    let pill_text_color = if composer_disabled {
        palette.text_dim
    } else {
        palette.text_secondary
    };
    for (bounds, label) in [
        (bar_layout.model_bounds, labels.model.as_str()),
        (bar_layout.effort_bounds, labels.effort.as_str()),
        (bar_layout.access_bounds, labels.access.as_str()),
        (bar_layout.skill_bounds, labels.skill.as_str()),
    ] {
        scene.draw_quad(
            Quad::new(bounds)
                .with_background(pill_color)
                .with_border(palette.panel_border, 1.0)
                .with_corner_radius(bounds.size.height / 2.0),
        );
        let text_run = state.text_system.layout_styled_mono(
            label,
            Point::new(bounds.origin.x + COMPOSER_PILL_PADDING_X, bounds.origin.y + 4.0),
            11.0,
            pill_text_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(text_run);
    }

    if let Some(menu) = state.workspaces.composer_menu {
        let items = composer_menu_items(state, menu);
        if !items.is_empty() {
            let anchor = match menu {
                ComposerMenuKind::Model => bar_layout.model_bounds,
                ComposerMenuKind::Effort => bar_layout.effort_bounds,
                ComposerMenuKind::Access => bar_layout.access_bounds,
                ComposerMenuKind::Skill => bar_layout.skill_bounds,
            };
            let menu_layout = composer_menu_layout(anchor, items.len());
            scene.draw_quad(
                Quad::new(menu_layout.bounds)
                    .with_background(palette.panel)
                    .with_border(palette.panel_border, 1.0)
                    .with_corner_radius(10.0),
            );
            for (index, bounds) in menu_layout.item_bounds {
                if let Some(item) = items.get(index) {
                    if item.selected {
                        scene.draw_quad(
                            Quad::new(bounds)
                                .with_background(palette.panel_highlight)
                                .with_corner_radius(6.0),
                        );
                    }
                    let item_run = state.text_system.layout_styled_mono(
                        &item.label,
                        Point::new(bounds.origin.x + 6.0, bounds.origin.y + 4.0),
                        11.0,
                        palette.text_primary,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(item_run);
                }
            }
        }
    }

    let status_y = logical_height - STATUS_BAR_HEIGHT - 3.0;
    let session_short = if state.session.session_info.session_id.len() > 8 {
        &state.session.session_info.session_id[..8]
    } else {
        &state.session.session_info.session_id
    };
    let mut parts = Vec::new();
    use crate::app::config::AgentKindConfig;
    let backend_name = match state.agent_selection.agent {
        AgentKindConfig::Codex => "codex",
    };
    parts.push(backend_name.to_string());
    if let Some(summary) = state.catalogs.mcp_status_summary() {
        parts.push(summary);
    }
    if let Some(active_agent) = &state.catalogs.active_agent {
        parts.push(format!("agent {}", truncate_preview(active_agent, 12)));
    }
    if !state.session.session_info.session_id.is_empty() {
        parts.push(format!("session {}", session_short));
    }
    let right_text = parts.join(" | ");
    let text_width = right_text.len() as f32 * 7.8;
    let right_x = input_layout.bar_bounds.origin.x + input_layout.bar_bounds.size.width - text_width;
    let right_run = state.text_system.layout_styled_mono(
        &right_text,
        Point::new(right_x, status_y),
        STATUS_BAR_FONT_SIZE,
        palette.status_right,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(right_run);
}

#[allow(dead_code)]
struct ComposerMenuItem {
    label: String,
    selected: bool,
}

#[allow(dead_code)]
fn composer_menu_items(state: &AppState, menu: ComposerMenuKind) -> Vec<ComposerMenuItem> {
    let Some(composer) = state.workspaces.active_composer() else {
        return Vec::new();
    };
    match menu {
        ComposerMenuKind::Model => composer
            .models
            .iter()
            .map(|model| {
                let label = if !model.display_name.trim().is_empty() {
                    model.display_name.clone()
                } else if !model.model.trim().is_empty() {
                    model.model.clone()
                } else {
                    model.id.clone()
                };
                ComposerMenuItem {
                    label,
                    selected: composer
                        .selected_model_id
                        .as_ref()
                        .map(|id| id == &model.id)
                        .unwrap_or(false),
                }
            })
            .collect(),
        ComposerMenuKind::Effort => {
            let options = composer.reasoning_options();
            if options.is_empty() {
                return vec![ComposerMenuItem {
                    label: "default".to_string(),
                    selected: true,
                }];
            }
            options
                .into_iter()
                .map(|effort| ComposerMenuItem {
                    label: reasoning_effort_label(effort).to_string(),
                    selected: composer.selected_effort == Some(effort),
                })
                .collect()
        }
        ComposerMenuKind::Access => WorkspaceAccessMode::all()
            .iter()
            .map(|mode| ComposerMenuItem {
                label: mode.label().to_string(),
                selected: composer.access_mode == *mode,
            })
            .collect(),
        ComposerMenuKind::Skill => {
            if composer.skills.is_empty() {
                return vec![ComposerMenuItem {
                    label: "No skills available".to_string(),
                    selected: false,
                }];
            }
            composer
                .skills
                .iter()
                .map(|skill| ComposerMenuItem {
                    label: skill.name.clone(),
                    selected: false,
                })
                .collect()
        }
    }
}
