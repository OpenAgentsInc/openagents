fn render_chat(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
    logical_height: f32,
    scale_factor: f32,
) {
    let chat_layout = state.build_chat_layout(sidebar_layout, logical_height);
    let viewport_top = chat_layout.viewport_top;
    let viewport_bottom = chat_layout.viewport_bottom;
    let content_x = chat_layout.content_x;
    let available_width = chat_layout.available_width;
    let chat_font_size = chat_layout.chat_font_size;
    let chat_line_height = chat_layout.chat_line_height;
    let streaming_height = chat_layout.streaming_height;

    let chat_clip_height = (viewport_bottom - viewport_top).max(0.0);
    let chat_clip_bounds = Bounds::new(
        sidebar_layout.main.origin.x,
        viewport_top,
        sidebar_layout.main.size.width,
        chat_clip_height,
    );
    let chat_clip_active = chat_clip_height > 0.0;
    if chat_clip_active {
        scene.push_clip(chat_clip_bounds);
    }

    if let Some(selection) = state.chat.chat_selection {
        if !selection.is_empty() {
            let (start, end) = selection.normalized();
            for layout in &chat_layout.message_layouts {
                for line in &layout.lines {
                    if line.y + line.line_height < viewport_top || line.y > viewport_bottom {
                        continue;
                    }
                    if line.message_index < start.message_index
                        || line.message_index > end.message_index
                    {
                        continue;
                    }
                    let mut sel_start = if line.message_index == start.message_index {
                        start.offset
                    } else {
                        line.display_range.start
                    };
                    let mut sel_end = if line.message_index == end.message_index {
                        end.offset
                    } else {
                        line.display_range.end
                    };
                    sel_start =
                        sel_start.clamp(line.display_range.start, line.display_range.end);
                    sel_end = sel_end.clamp(line.display_range.start, line.display_range.end);
                    if sel_end <= sel_start {
                        continue;
                    }
                    let start_char = char_index_for_byte_offset(
                        &line.text,
                        sel_start - line.display_range.start,
                    );
                    let end_char = char_index_for_byte_offset(
                        &line.text,
                        sel_end - line.display_range.start,
                    );
                    if end_char <= start_char {
                        continue;
                    }
                    let char_width = state
                        .text_system
                        .measure_styled_mono(
                            "M",
                            line.font_size,
                            wgpui::text::FontStyle::default(),
                        )
                        .max(1.0);
                    let highlight_x = line.x + start_char as f32 * char_width;
                    let highlight_w = (end_char - start_char) as f32 * char_width;
                    let bounds =
                        Bounds::new(highlight_x, line.y, highlight_w, line.line_height);
                    scene.draw_quad(Quad::new(bounds).with_background(palette.selection_bg));
                }
            }
        }
    }

    // Render boot sections in centered card
    let boot_section_font_size = 13.0_f32;
    let boot_section_line_height = boot_section_font_size * 1.6;
    let boot_section_padding = 16.0_f32;
    let header_height = 44.0_f32;

    // Card dimensions
    let card_width = 400.0_f32;
    let card_corner_radius = 8.0_f32;

    // Get the first active boot section and its details
    let (boot_section, detail_count) = chat_layout
        .boot_sections
        .iter()
        .find(|s| !s.summary.is_empty())
        .map(|s| (Some(s), s.details.len()))
        .unwrap_or((None, 0));

    // Calculate card height based on expanded state
    let is_expanded = boot_section.map(|s| s.expanded).unwrap_or(false);
    let details_height = if is_expanded && detail_count > 0 {
        detail_count as f32 * boot_section_line_height + boot_section_padding
    } else {
        0.0
    };
    let card_height = header_height + details_height;

    // Center card horizontally, position 100px from top
    let main_center_x = sidebar_layout.main.origin.x + sidebar_layout.main.size.width / 2.0;
    let card_x = main_center_x - card_width / 2.0;
    let card_y = sidebar_layout.main.origin.y + 100.0;

    let card_bounds = Bounds::new(card_x, card_y, card_width, card_height);

    // Store card bounds for click detection
    if let Some(sections) = &mut state.chat.boot_sections {
        sections.card_bounds = Some(card_bounds);
    }

    // Draw card background with white border (10% opacity)
    let border_color = Hsla::new(0.0, 0.0, 1.0, 0.1); // White at 10% opacity
    scene.draw_quad(
        Quad::new(card_bounds)
            .with_background(Hsla::new(0.0, 0.0, 0.0, 0.3)) // Slight dark bg
            .with_border(border_color, 1.0)
            .with_corner_radius(card_corner_radius),
    );

    // Render boot section content inside card
    if let Some(boot_section) = boot_section {
        let header_y = card_y + header_height / 2.0 - boot_section_font_size / 2.0;

        // Status indicator on left (green dot for success, etc)
        let status_icon = match boot_section.status {
            SectionStatus::Pending => "",
            SectionStatus::InProgress => "◌", // Empty circle
            SectionStatus::Success => "●",    // Filled circle
            SectionStatus::Error => "✕",
        };
        if !status_icon.is_empty() {
            let status_color = match boot_section.status {
                SectionStatus::Pending => palette.text_muted,
                SectionStatus::InProgress => Hsla::new(45.0 / 360.0, 0.8, 0.5, 1.0),
                SectionStatus::Success => Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0),
                SectionStatus::Error => Hsla::new(0.0, 0.7, 0.55, 1.0),
            };
            let status_run = state.text_system.layout_styled_mono(
                status_icon,
                Point::new(card_x + boot_section_padding, header_y),
                boot_section_font_size,
                status_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(status_run);
        }

        // Summary text
        let summary_x = card_x + boot_section_padding + boot_section_font_size * 1.5;
        let summary_run = state.text_system.layout_styled_mono(
            &boot_section.summary,
            Point::new(summary_x, header_y),
            boot_section_font_size,
            palette.text_secondary,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(summary_run);

        // No arrow - card is always expanded

        // Render detail lines when expanded
        if boot_section.expanded && !boot_section.details.is_empty() {
            let detail_x = card_x + boot_section_padding;
            let mut detail_y = card_y + header_height;

            for detail in &boot_section.details {
                let detail_run = state.text_system.layout_styled_mono(
                    detail,
                    Point::new(detail_x, detail_y),
                    boot_section_font_size - 1.0,
                    palette.text_muted,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(detail_run);
                detail_y += boot_section_line_height;
            }
        }
    }

    // Render inline issue selector (if present and awaiting selection)
    if let Some(selector) = &mut state.chat.inline_issue_selector {
        if selector.await_selection && !selector.suggestions.is_empty() {
            let selector_font_size = 13.0_f32;
            let selector_line_height = selector_font_size * 1.6;
            let button_padding_x = 12.0_f32;
            let button_padding_y = 6.0_f32;
            let button_gap = 8.0_f32;
            let button_corner_radius = 4.0_f32;

            // Calculate y position after boot sections
            let mut selector_y = viewport_top - state.chat.scroll_offset;
            for boot_section in &chat_layout.boot_sections {
                selector_y += boot_section.height + 8.0;
            }

            // Header - minimal
            let header_text = "Select:";
            let header_run = state.text_system.layout_styled_mono(
                header_text,
                Point::new(content_x, selector_y),
                selector_font_size,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(header_run);
            selector_y += selector_line_height;

            // Clear previous bounds
            selector.suggestion_bounds.clear();

            // Render each suggestion as a button
            for (idx, suggestion) in selector.suggestions.iter().enumerate() {
                if idx >= 9 {
                    break; // Max 9 suggestions (keys 1-9)
                }

                let button_text = format!(
                    "{}. #{} {}",
                    idx + 1,
                    suggestion.number,
                    suggestion.title
                );

                // Measure text width
                let text_width = state
                    .text_system
                    .measure_styled_mono(
                        &button_text,
                        selector_font_size,
                        wgpui::text::FontStyle::default(),
                    )
                    .max(100.0);

                let button_width = text_width + button_padding_x * 2.0;
                let button_height = selector_line_height + button_padding_y * 2.0;

                let button_bounds = Bounds::new(content_x, selector_y, button_width, button_height);

                // Store bounds for click detection
                selector.suggestion_bounds.push(button_bounds);

                // Hover highlighting
                let is_hovered = selector.hovered_index == Some(idx);
                let bg_color = if is_hovered {
                    Hsla::new(0.0, 0.0, 0.3, 1.0)
                } else {
                    Hsla::new(0.0, 0.0, 0.15, 1.0)
                };
                let border_color = if is_hovered {
                    Hsla::new(210.0 / 360.0, 0.6, 0.5, 1.0)
                } else {
                    Hsla::new(0.0, 0.0, 0.4, 1.0)
                };

                // Draw button background
                scene.draw_quad(
                    Quad::new(button_bounds)
                        .with_background(bg_color)
                        .with_border(border_color, 1.0)
                        .with_corner_radius(button_corner_radius),
                );

                // Draw button text
                let text_run = state.text_system.layout_styled_mono(
                    &button_text,
                    Point::new(content_x + button_padding_x, selector_y + button_padding_y),
                    selector_font_size,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(text_run);

                selector_y += button_height + button_gap;
            }

            // Skip button
            let skip_text = "S. Skip";
            let skip_text_width = state
                .text_system
                .measure_styled_mono(
                    skip_text,
                    selector_font_size,
                    wgpui::text::FontStyle::default(),
                )
                .max(100.0);
            let skip_button_width = skip_text_width + button_padding_x * 2.0;
            let skip_button_height = selector_line_height + button_padding_y * 2.0;
            let skip_bounds = Bounds::new(content_x, selector_y, skip_button_width, skip_button_height);

            selector.skip_button_bounds = Some(skip_bounds);

            // Check if skip button is hovered
            let skip_hovered = selector.hovered_index == Some(usize::MAX);
            let skip_bg = if skip_hovered {
                Hsla::new(0.0, 0.0, 0.3, 1.0)
            } else {
                Hsla::new(0.0, 0.0, 0.15, 1.0)
            };
            let skip_border = if skip_hovered {
                Hsla::new(45.0 / 360.0, 0.6, 0.5, 1.0)
            } else {
                Hsla::new(0.0, 0.0, 0.4, 1.0)
            };

            scene.draw_quad(
                Quad::new(skip_bounds)
                    .with_background(skip_bg)
                    .with_border(skip_border, 1.0)
                    .with_corner_radius(button_corner_radius),
            );

            let skip_run = state.text_system.layout_styled_mono(
                skip_text,
                Point::new(content_x + button_padding_x, selector_y + button_padding_y),
                selector_font_size,
                palette.text_muted,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(skip_run);
        }
    }

    let mut y = viewport_top - state.chat.scroll_offset;
    // Advance y past boot sections
    for boot_section in &chat_layout.boot_sections {
        y += boot_section.height + 8.0; // boot_section_gap
    }
    // Advance y past inline issue selector (if present)
    if let Some(selector) = &state.chat.inline_issue_selector {
        if selector.await_selection && !selector.suggestions.is_empty() {
            let selector_font_size = 13.0_f32;
            let selector_line_height = selector_font_size * 1.6;
            let button_padding_y = 6.0_f32;
            let button_gap = 8.0_f32;
            let button_height = selector_line_height + button_padding_y * 2.0;
            // Header height (just one line now)
            let header_height = selector_line_height;
            // Button heights (one per suggestion, max 9)
            let num_buttons = selector.suggestions.len().min(9);
            let buttons_height = num_buttons as f32 * (button_height + button_gap);
            // Skip button height + gap
            let skip_height = button_height + button_gap;
            // Total selector height
            let total_selector_height = header_height + buttons_height + skip_height + 8.0;
            y += total_selector_height;
        }
    }
    let mut inline_tools_render_idx = 0;
    let mut dspy_stages_render_idx = 0;
    for (i, msg) in state.chat.messages.iter().enumerate() {
        let layout = &chat_layout.message_layouts[i];
        let msg_height = layout.height;

        if y + msg_height < viewport_top || y > viewport_bottom {
            y += msg_height;
            // Account for DSPy stages even when skipping off-screen messages
            while dspy_stages_render_idx < chat_layout.dspy_stages.len()
                && chat_layout.dspy_stages[dspy_stages_render_idx].message_index == i
            {
                y += chat_layout.dspy_stages[dspy_stages_render_idx].height + TOOL_PANEL_GAP;
                dspy_stages_render_idx += 1;
            }
            // Account for inline tools even when skipping off-screen messages
            if inline_tools_render_idx < chat_layout.inline_tools.len()
                && chat_layout.inline_tools[inline_tools_render_idx].message_index == i
            {
                y += chat_layout.inline_tools[inline_tools_render_idx].height + TOOL_PANEL_GAP;
                inline_tools_render_idx += 1;
            }
            continue;
        }

        match msg.role {
            MessageRole::User => {
                for line in &layout.lines {
                    if line.y < viewport_bottom && line.y + line.line_height > viewport_top {
                        let text_run = state.text_system.layout_styled_mono(
                            &line.text,
                            Point::new(line.x, line.y),
                            line.font_size,
                            palette.user_text,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(text_run);
                    }
                }
            }
            MessageRole::Assistant => {
                if let Some(doc) = &msg.document {
                    let content_visible = y + msg_height > viewport_top && y < viewport_bottom;
                    if content_visible {
                        state.chat.markdown_renderer.render(
                            doc,
                            Point::new(content_x, y),
                            available_width,
                            &mut state.text_system,
                            scene,
                        );
                    }
                } else {
                    for line in &layout.lines {
                        if line.y < viewport_bottom && line.y + line.line_height > viewport_top {
                            let text_run = state.text_system.layout_styled_mono(
                                &line.text,
                                Point::new(line.x, line.y),
                                line.font_size,
                                palette.assistant_text,
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(text_run);
                        }
                    }
                }

                // Render metadata under assistant messages
                if let Some(meta) = &msg.metadata {
                    let meta_y = y + msg_height - chat_layout.chat_line_height * 0.5;
                    if meta_y > viewport_top && meta_y < viewport_bottom {
                        let mut parts = Vec::new();
                        if let Some(model) = &meta.model {
                            parts.push(model.clone());
                        }
                        if let Some(input) = meta.input_tokens {
                            if let Some(output) = meta.output_tokens {
                                parts.push(format!("{}+{} tokens", input, output));
                            }
                        }
                        if let Some(ms) = meta.duration_ms {
                            if ms >= 1000 {
                                parts.push(format!("{:.1}s", ms as f64 / 1000.0));
                            } else {
                                parts.push(format!("{}ms", ms));
                            }
                        }
                        if let Some(cost) = meta.cost_msats {
                            if cost > 0 {
                                parts.push(format!("{} msats", cost));
                            }
                        }
                        if !parts.is_empty() {
                            let meta_text = parts.join(" · ");
                            let meta_color = Hsla::new(0.0, 0.0, 0.35, 1.0); // dark gray
                            let meta_run = state.text_system.layout_styled_mono(
                                &meta_text,
                                Point::new(content_x, meta_y),
                                11.0,
                                meta_color,
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(meta_run);
                        }
                    }
                }
            }
            MessageRole::AssistantThought => {
                for line in &layout.lines {
                    if line.y < viewport_bottom && line.y + line.line_height > viewport_top {
                        let text_run = state.text_system.layout_styled_mono(
                            &line.text,
                            Point::new(line.x, line.y),
                            line.font_size,
                            palette.thinking_text,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(text_run);
                    }
                }
            }
        }
        y += msg_height;

        // Account for DSPy stages after this message
        while dspy_stages_render_idx < chat_layout.dspy_stages.len()
            && chat_layout.dspy_stages[dspy_stages_render_idx].message_index == i
        {
            y += chat_layout.dspy_stages[dspy_stages_render_idx].height + TOOL_PANEL_GAP;
            dspy_stages_render_idx += 1;
        }

        // Account for inline tools after this message
        if inline_tools_render_idx < chat_layout.inline_tools.len()
            && chat_layout.inline_tools[inline_tools_render_idx].message_index == i
        {
            y += chat_layout.inline_tools[inline_tools_render_idx].height + TOOL_PANEL_GAP;
            inline_tools_render_idx += 1;
        }
    }

    // Render streaming thought (reasoning) content
    let streaming_thought_height = chat_layout.streaming_thought_height;
    if !state.chat.streaming_thought.source().is_empty() {
        let doc = state.chat.streaming_thought.document();
        let content_visible = y + streaming_thought_height > viewport_top && y < viewport_bottom;
        if content_visible {
            state.chat.markdown_renderer.render(
                doc,
                Point::new(content_x, y),
                available_width,
                &mut state.text_system,
                scene,
            );
        }
        y += streaming_thought_height;
    }

    if !state.chat.streaming_markdown.source().is_empty() {
        let doc = state.chat.streaming_markdown.document();
        let content_visible = y + streaming_height > viewport_top && y < viewport_bottom;
        if content_visible {
            state.chat.markdown_renderer.render(
                doc,
                Point::new(content_x, y),
                available_width,
                &mut state.text_system,
                scene,
            );
        }
    } else if state.chat.is_thinking {
        if y < viewport_bottom && y + chat_line_height > viewport_top {
            let text_run = state.text_system.layout_styled_mono(
                "...",
                Point::new(content_x, y),
                chat_font_size,
                palette.thinking_text,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(text_run);
        }
    }

    render_tools(
        state,
        scene,
        palette,
        &chat_layout,
        viewport_top,
        viewport_bottom,
        scale_factor,
    );

    if chat_clip_active {
        scene.pop_clip();
    }
}
