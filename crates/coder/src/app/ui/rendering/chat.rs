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

    let mut y = viewport_top - state.chat.scroll_offset;
    let mut inline_tools_render_idx = 0;
    for (i, msg) in state.chat.messages.iter().enumerate() {
        let layout = &chat_layout.message_layouts[i];
        let msg_height = layout.height;

        if y + msg_height < viewport_top || y > viewport_bottom {
            y += msg_height;
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
        }
        y += msg_height;

        // Account for inline tools after this message
        if inline_tools_render_idx < chat_layout.inline_tools.len()
            && chat_layout.inline_tools[inline_tools_render_idx].message_index == i
        {
            y += chat_layout.inline_tools[inline_tools_render_idx].height + TOOL_PANEL_GAP;
            inline_tools_render_idx += 1;
        }
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
