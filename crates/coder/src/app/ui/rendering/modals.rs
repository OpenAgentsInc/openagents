fn render_modals(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    _sidebar_layout: &SidebarLayout,
    logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) {
    use crate::app::ui::{
        agent_list_layout, agent_modal_content_top, hook_event_layout, modal_y_in_content,
        session_list_layout, skill_list_layout, skill_modal_content_top, HELP_MODAL_HEIGHT,
        HELP_MODAL_WIDTH, HOOK_MODAL_HEIGHT, HOOK_MODAL_WIDTH, SESSION_MODAL_HEIGHT,
        SESSION_MODAL_PADDING, SESSION_MODAL_WIDTH, SETTINGS_MODAL_HEIGHT, SETTINGS_MODAL_WIDTH,
        SETTINGS_ROW_HEIGHT, SETTINGS_TAB_HEIGHT,
    };

    let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

    // Draw modal if active
    let should_refresh_sessions = matches!(state.modal_state, ModalState::SessionList { .. })
        && state.session.session_cards.len() != state.session.session_index.len();
    if should_refresh_sessions {
        state.session.refresh_session_cards(state.chat.is_thinking);
    }
    let should_refresh_agents = matches!(state.modal_state, ModalState::AgentList { .. })
        && state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len();
    if should_refresh_agents {
        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
    }
    let should_refresh_skills = matches!(state.modal_state, ModalState::SkillList { .. })
        && state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len();
    if should_refresh_skills {
        state.catalogs.refresh_skill_cards();
    }
    match &state.modal_state {
        ModalState::None => {}
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
        }
        ModalState::SessionList { selected } => {
            let sessions = &state.session.session_index;
            // Semi-transparent overlay
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let selected = (*selected).min(sessions.len().saturating_sub(1));
            let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                0.0
            } else {
                state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
            };
            let layout = session_list_layout(
                logical_width,
                logical_height,
                sessions.len(),
                selected,
                checkpoint_height,
            );
            let modal_bounds = layout.modal_bounds;
            let modal_x = modal_bounds.origin.x;
            let modal_y = modal_bounds.origin.y;
            let _modal_width = modal_bounds.size.width;
            let modal_height = modal_bounds.size.height;

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + SESSION_MODAL_PADDING;
            let title_run = state.text_system.layout_styled_mono(
                "Sessions",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let desc_run = state.text_system.layout_styled_mono(
                "Click a card to resume, or fork from a previous session.",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);

            if sessions.is_empty() {
                y += 26.0;
                let empty_run = state.text_system.layout_styled_mono(
                    "No sessions recorded yet.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
                for (index, bounds) in &layout.card_bounds {
                    if let Some(card) = state.session.session_cards.get_mut(*index) {
                        card.paint(*bounds, &mut paint_cx);
                    }
                    if *index == selected {
                        let outline =
                            Quad::new(*bounds).with_border(Hsla::new(120.0, 0.6, 0.5, 1.0), 1.0);
                        paint_cx.scene.draw_quad(outline);
                    }
                }
            }

            if let Some(bounds) = layout.checkpoint_bounds {
                let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
                state.session.checkpoint_restore.paint(bounds, &mut paint_cx);
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Enter to resume · Esc to exit · Fork with button",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
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
        ModalState::SkillList { selected } => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

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
                "Skills",
                Point::new(modal_x + 16.0, y),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let desc_run = state.text_system.layout_styled_mono(
                "Filesystem skills available to Claude.",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);
            y += 18.0;

            let project_path = state
                .catalogs
                .skill_project_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let project_line = format!("Project skills: {}", project_path);
            let project_run = state.text_system.layout_styled_mono(
                &truncate_preview(&project_line, 90),
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(project_run);
            y += 18.0;

            if let Some(user_path) = &state.catalogs.skill_user_path {
                let user_line = format!("User skills: {}", user_path.display());
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

            if let Some(error) = &state.catalogs.skill_load_error {
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
                .skill_entries
                .iter()
                .filter(|entry| entry.source == SkillSource::Project)
                .count();
            let user_count = state
                .catalogs
                .skill_entries
                .iter()
                .filter(|entry| entry.source == SkillSource::User)
                .count();
            let counts_line = format!("Skills: {} project · {} user", project_count, user_count);
            let counts_run = state.text_system.layout_styled_mono(
                &counts_line,
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.6, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(counts_run);

            let list_top = skill_modal_content_top(modal_y, state);
            let layout = skill_list_layout(
                logical_width,
                logical_height,
                state.catalogs.skill_entries.len(),
                *selected,
                list_top,
            );

            if state.catalogs.skill_entries.is_empty() {
                let empty_run = state.text_system.layout_styled_mono(
                    "No skills found.",
                    Point::new(modal_x + 16.0, list_top),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(empty_run);
            } else {
                let selected = (*selected).min(state.catalogs.skill_entries.len().saturating_sub(1));
                let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
                for (index, bounds) in &layout.card_bounds {
                    if let Some(card) = state.catalogs.skill_cards.get_mut(*index) {
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
                "Enter to close · R to reload · Esc to exit",
                Point::new(modal_x + 16.0, y),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
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
        }
        ModalState::ToolList { selected } => {
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
        }
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
        ModalState::McpConfig { selected } => {
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
        ModalState::Help => {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            let overlay = Quad::new(bounds).with_background(palette.overlay);
            scene.draw_quad(overlay);

            let modal_width = HELP_MODAL_WIDTH;
            let modal_height = HELP_MODAL_HEIGHT;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(palette.panel)
                .with_border(palette.panel_border, 1.0);
            scene.draw_quad(modal_bg);

            let mut y = modal_y + 16.0;
            let title_run = state.text_system.layout_styled_mono(
                "Help",
                Point::new(modal_x + 16.0, y),
                14.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);
            y += 20.0;

            let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;
            let line_height = 14.0;
            let section_gap = 6.0;

            let interrupt =
                keybinding_labels(&state.settings.keybindings, KeyAction::Interrupt, "Ctrl+C");
            let palette_key = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::OpenCommandPalette,
                "Ctrl+K",
            );
            let settings_key =
                keybinding_labels(&state.settings.keybindings, KeyAction::OpenSettings, "Ctrl+,");
            let left_sidebar = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::ToggleLeftSidebar,
                "Ctrl+[",
            );
            let right_sidebar = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::ToggleRightSidebar,
                "Ctrl+]",
            );
            let toggle_sidebars = keybinding_labels(
                &state.settings.keybindings,
                KeyAction::ToggleSidebars,
                "Ctrl+\\",
            );

            let sections: Vec<(&str, Vec<String>)> = vec![
                (
                    "Hotkeys",
                    vec![
                        "F1 - Help".to_string(),
                        "Enter - Send message".to_string(),
                        "Shift+Tab - Cycle permission mode".to_string(),
                        format!("{} - Interrupt request", interrupt),
                        format!("{} - Command palette", palette_key),
                        format!("{} - Settings", settings_key),
                        format!("{} - Toggle left sidebar", left_sidebar),
                        format!("{} - Toggle right sidebar", right_sidebar),
                        format!("{} - Toggle both sidebars", toggle_sidebars),
                    ],
                ),
                (
                    "Core",
                    vec![
                        "/model - choose model; /output-style <name> - style output".to_string(),
                        "/clear - reset chat; /compact - compact context; /undo - undo last exchange"
                            .to_string(),
                        "/cancel - cancel active run; /bug - report issue".to_string(),
                    ],
                ),
                (
                    "Sessions",
                    vec![
                        "/session list - list sessions; /session resume <id> - resume".to_string(),
                        "/session fork - fork current; /session export - export markdown".to_string(),
                    ],
                ),
                (
                    "Permissions",
                    vec![
                        "/permission mode <default|plan|acceptEdits|bypassPermissions|dontAsk>"
                            .to_string(),
                        "/permission rules - manage rules".to_string(),
                        "/permission allow|deny <tool|bash:pattern>".to_string(),
                    ],
                ),
                (
                    "Tools, MCP, Hooks",
                    vec![
                        "/tools - list tools; /tools enable|disable <tool>".to_string(),
                        "/mcp - open MCP servers; /mcp add|remove <name> <json>".to_string(),
                        "/hooks - hook panel; /hooks reload - reload scripts".to_string(),
                    ],
                ),
                (
                    "Agents, Skills, Prompts",
                    vec![
                        "/agents - manage agents; /agent select <name>; /agent clear".to_string(),
                        "/skills - manage skills; /skills reload".to_string(),
                        "@file - insert file; !command - run bash and insert output".to_string(),
                    ],
                ),
            ];

            for (title, lines) in sections {
                let heading = state.text_system.layout_styled_mono(
                    title,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(heading);
                y += line_height;

                for line in lines {
                    for wrapped in wrap_text(&line, max_chars) {
                        let text_run = state.text_system.layout_styled_mono(
                            &wrapped,
                            Point::new(modal_x + 20.0, y),
                            11.0,
                            palette.text_muted,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(text_run);
                        y += line_height;
                    }
                }

                y += section_gap;
            }

            y = modal_y + modal_height - 24.0;
            let footer_run = state.text_system.layout_styled_mono(
                "Esc/F1 to close",
                Point::new(modal_x + 16.0, y),
                12.0,
                palette.text_faint,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
        // Placeholder for new feature modals - render a basic "coming soon" overlay
        ModalState::Wallet
        | ModalState::DvmProviders
        | ModalState::Gateway
        | ModalState::LmRouter
        | ModalState::Nexus
        | ModalState::SparkWallet
        | ModalState::Nip90Jobs
        | ModalState::Oanix
        | ModalState::Directives
        | ModalState::Issues
        | ModalState::AutopilotIssues
        | ModalState::Rlm
        | ModalState::RlmTrace
        | ModalState::PylonEarnings
        | ModalState::Dspy
        | ModalState::Nip28Chat => {
            scene.set_layer(1);
            let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
            scene.draw_quad(overlay);

            let modal_width = 400.0;
            let modal_height = 120.0;
            let modal_x = (logical_width - modal_width) / 2.0;
            let modal_y = modal_y_in_content(logical_height, modal_height);
            let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

            let modal_bg = Quad::new(modal_bounds)
                .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
            scene.draw_quad(modal_bg);

            let title_run = state.text_system.layout_styled_mono(
                "Feature in development",
                Point::new(modal_x + 16.0, modal_y + 16.0),
                14.0,
                Hsla::new(0.0, 0.0, 0.9, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(title_run);

            let desc_run = state.text_system.layout_styled_mono(
                "This feature is coming soon.",
                Point::new(modal_x + 16.0, modal_y + 50.0),
                12.0,
                Hsla::new(0.0, 0.0, 0.5, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(desc_run);

            let footer_run = state.text_system.layout_styled_mono(
                "Press Esc to close",
                Point::new(modal_x + 16.0, modal_y + modal_height - 24.0),
                12.0,
                Hsla::new(0.0, 0.0, 0.4, 1.0),
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(footer_run);
        }
    }
}
