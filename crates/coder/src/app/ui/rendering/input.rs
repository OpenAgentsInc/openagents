fn render_input(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
    _logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) {
    // Input box (max width 768px, centered)
    let max_input_width = 768.0_f32;
    let available_input_width = sidebar_layout.main.size.width - INPUT_PADDING * 2.0;
    let input_width = available_input_width.min(max_input_width);
    let input_x =
        sidebar_layout.main.origin.x + (sidebar_layout.main.size.width - input_width) / 2.0;
    // Set max width for wrapping, then calculate dynamic height
    state.input.set_max_width(input_width);
    let input_height = state.input.current_height().max(40.0);

    // Input area background - flush with top of input box
    let input_area_y = logical_height - input_height - INPUT_PADDING - STATUS_BAR_HEIGHT;
    let input_area_bounds = Bounds::new(
        sidebar_layout.main.origin.x,
        input_area_y,
        sidebar_layout.main.size.width,
        logical_height - input_area_y,
    );
    scene.draw_quad(Quad::new(input_area_bounds).with_background(palette.background));

    let input_bounds = Bounds::new(
        input_x,
        logical_height - input_height - INPUT_PADDING - STATUS_BAR_HEIGHT,
        input_width,
        input_height,
    );

    let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
    state.input.paint(input_bounds, &mut paint_cx);

    // Draw ">" prompt inside input, aligned with the cursor line (bottom)
    let prompt_font = state.settings.coder_settings.font_size;
    let line_height = prompt_font * 1.4;
    let cursor_line = state.input.cursor_line();
    let prompt_y =
        input_bounds.origin.y + 8.0 + line_height * cursor_line as f32 + prompt_font * 0.15;
    let prompt_run = state.text_system.layout_styled_mono(
        ">",
        Point::new(input_bounds.origin.x + 12.0, prompt_y),
        prompt_font,
        palette.prompt,
        wgpui::text::FontStyle::default(),
    );
    scene.draw_text(prompt_run);

    let mode_label = coder_mode_display(state.permissions.coder_mode);
    let mode_color = coder_mode_color(state.permissions.coder_mode, palette);
    if state.session.session_info.permission_mode.is_empty() {
        let mode_text = format!("Mode: {}", mode_label);
        let mode_run = state.text_system.layout_styled_mono(
            &mode_text,
            Point::new(
                input_bounds.origin.x,
                input_bounds.origin.y + input_bounds.size.height + 2.0,
            ),
            10.0,
            mode_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(mode_run);
    }

    // Draw status bar at very bottom (centered vertically)
    let status_y = logical_height - STATUS_BAR_HEIGHT - 3.0;

    // Left side: mode (colored) + hint (gray), flush with left edge of 768px container
    if !state.session.session_info.permission_mode.is_empty() {
        let mode_x = input_x;
        let mode_text = coder_mode_display(state.permissions.coder_mode);
        let mode_run = state.text_system.layout_styled_mono(
            mode_text,
            Point::new(mode_x, status_y),
            STATUS_BAR_FONT_SIZE,
            mode_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(mode_run);

        // Draw hint in gray after the mode text
        let hint_text = " (shift+tab to cycle)";
        let mode_width = mode_text.len() as f32 * 7.8; // Approx char width at 13pt
        let hint_run = state.text_system.layout_styled_mono(
            hint_text,
            Point::new(mode_x + mode_width, status_y),
            STATUS_BAR_FONT_SIZE,
            palette.status_right,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(hint_run);
    }

    // Right side: backend, model, available open models, tools, session
    if !state.session.session_info.model.is_empty() || true {
        // Format: "claude | haiku | gptoss | 18 tools | abc123"
        let model_short = state
            .session
            .session_info
            .model
            .replace("claude-", "")
            .replace("-20251101", "")
            .replace("-20250929", "")
            .replace("-20251001", "");
        let session_short = if state.session.session_info.session_id.len() > 8 {
            &state.session.session_info.session_id[..8]
        } else {
            &state.session.session_info.session_id
        };
        let mut parts = Vec::new();
        // Add current backend name
        use crate::app::config::AgentKindConfig;
        let backend_name = match state.agent_selection.agent {
            AgentKindConfig::Claude => "claude",
            AgentKindConfig::Codex => "codex",
        };
        parts.push(backend_name.to_string());
        if !model_short.is_empty() {
            parts.push(model_short);
        }
        if let Some(summary) = state.catalogs.mcp_status_summary() {
            parts.push(summary);
        }
        if let Some(active_agent) = &state.catalogs.active_agent {
            parts.push(format!("agent {}", truncate_preview(active_agent, 12)));
        }
        // Only show session if we have an actual session ID
        if !state.session.session_info.session_id.is_empty() {
            parts.push(format!("session {}", session_short));
        }
        let right_text = parts.join(" | ");
        // Measure and right-align within the 768px container
        let text_width = right_text.len() as f32 * 7.8; // Approx char width at 13pt
        let right_x = input_x + input_width - text_width;
        let right_run = state.text_system.layout_styled_mono(
            &right_text,
            Point::new(right_x, status_y),
            STATUS_BAR_FONT_SIZE,
            palette.status_right,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(right_run);
    }
}
