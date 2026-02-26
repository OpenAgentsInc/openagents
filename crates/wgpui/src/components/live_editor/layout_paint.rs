impl LiveEditor {
    /// Render a line with markdown formatting
    /// `is_continuation` indicates this is a wrapped continuation (not the start of the line)
    #[expect(clippy::too_many_arguments)]
    fn render_formatted_line(
        &self,
        line: &str,
        block_type: BlockType,
        x: f32,
        y: f32,
        _line_height: f32,
        is_continuation: bool,
        cx: &mut PaintContext,
    ) {
        let mut current_x = x;

        match block_type {
            BlockType::Header(level) => {
                // Render header with larger font
                let content = strip_header_prefix(line);
                let font_size = self.style.font_size * header_font_scale(level);
                let spans = parse_inline(content);

                for span in spans {
                    let mut style = FontStyle::default();
                    if span.bold {
                        style.bold = true;
                    }
                    if span.italic {
                        style.italic = true;
                    }

                    let text_run = cx.text.layout_styled_mono(
                        &span.text,
                        Point::new(current_x, y),
                        font_size,
                        self.style.text_color,
                        style,
                    );
                    current_x += span.text.chars().count() as f32 * (font_size * 0.6);
                    cx.scene.draw_text(text_run);
                }
            }

            BlockType::CodeBlock | BlockType::CodeFence => {
                // Render code with monospace, slightly dimmed
                let code_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
                let text_run = cx.text.layout_styled_mono(
                    line,
                    Point::new(x, y),
                    self.style.font_size,
                    code_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(text_run);
            }

            BlockType::UnorderedList => {
                // Only render bullet on first segment, not continuations
                let content = if is_continuation {
                    // Continuation: indent to align with content after bullet
                    current_x += self.mono_char_width * 2.0;
                    line
                } else {
                    // First segment: render bullet point then content
                    let content = strip_list_prefix(line);
                    let bullet = "\u{2022} "; // bullet character
                    let bullet_run = cx.text.layout_styled_mono(
                        bullet,
                        Point::new(current_x, y),
                        self.style.font_size,
                        self.style.text_color,
                        FontStyle::default(),
                    );
                    cx.scene.draw_text(bullet_run);
                    current_x += self.mono_char_width * 2.0;
                    content
                };

                // Render content with inline formatting
                self.render_inline_formatted(content, current_x, y, cx);
            }

            BlockType::OrderedList => {
                // Only render number on first segment, not continuations
                if is_continuation {
                    // Continuation: indent to align with content after number
                    // Use a fixed indent (4 chars: "N. " padding)
                    current_x += self.mono_char_width * 4.0;
                    self.render_inline_formatted(line, current_x, y, cx);
                } else {
                    // First segment: render number then content
                    let content = strip_list_prefix(line);
                    // Extract the number from original line
                    let num: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
                    let prefix = format!("{}. ", num);
                    let prefix_run = cx.text.layout_styled_mono(
                        &prefix,
                        Point::new(current_x, y),
                        self.style.font_size,
                        self.style.text_color,
                        FontStyle::default(),
                    );
                    cx.scene.draw_text(prefix_run);
                    current_x += self.mono_char_width * prefix.len() as f32;

                    // Render content with inline formatting
                    self.render_inline_formatted(content, current_x, y, cx);
                }
            }

            BlockType::Blockquote => {
                // Render blockquote bar and content
                let content = strip_blockquote_prefix(line);
                let bar_color = Hsla::new(210.0, 0.5, 0.5, 0.7);

                // Draw vertical bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        x,
                        y,
                        3.0,
                        self.style.font_size * self.style.line_height,
                    ))
                    .with_background(bar_color),
                );

                // Render content with italic style
                let text_run = cx.text.layout_styled_mono(
                    content,
                    Point::new(x + 12.0, y),
                    self.style.font_size,
                    Hsla::new(0.0, 0.0, 0.7, 1.0),
                    FontStyle::italic(),
                );
                cx.scene.draw_text(text_run);
            }

            BlockType::HorizontalRule => {
                // Draw a horizontal line
                let rule_y = y + (self.style.font_size * self.style.line_height) / 2.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(x, rule_y, 200.0, 1.0))
                        .with_background(Hsla::new(0.0, 0.0, 0.3, 1.0)),
                );
            }

            BlockType::Empty => {
                // Nothing to render
            }

            BlockType::Paragraph => {
                // Render paragraph with inline formatting
                self.render_inline_formatted(line, x, y, cx);
            }
        }
    }

    /// Render text with inline formatting (bold, italic, code, etc.)
    fn render_inline_formatted(&self, text: &str, x: f32, y: f32, cx: &mut PaintContext) {
        let spans = parse_inline(text);
        let mut current_x = x;

        for span in spans {
            if span.text.is_empty() {
                continue;
            }

            if span.code {
                // Inline code with background
                let bg_padding = 2.0;
                let code_width = span.text.chars().count() as f32 * self.mono_char_width;

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        current_x - bg_padding,
                        y,
                        code_width + bg_padding * 2.0,
                        self.style.font_size * self.style.line_height,
                    ))
                    .with_background(inline_code_background())
                    .with_corner_radius(3.0),
                );

                let text_run = cx.text.layout_styled_mono(
                    &span.text,
                    Point::new(current_x, y),
                    self.style.font_size,
                    Hsla::new(30.0, 0.8, 0.7, 1.0), // Orange-ish for code
                    FontStyle::default(),
                );
                cx.scene.draw_text(text_run);
                current_x += code_width;
            } else {
                // Regular text with bold/italic
                let mut style = FontStyle::default();
                if span.bold {
                    style.bold = true;
                }
                if span.italic {
                    style.italic = true;
                }

                let text_run = cx.text.layout_styled_mono(
                    &span.text,
                    Point::new(current_x, y),
                    self.style.font_size,
                    self.style.text_color,
                    style,
                );
                cx.scene.draw_text(text_run);
                current_x += span.text.chars().count() as f32 * self.mono_char_width;
            }
        }
    }

    pub(super) fn paint_component(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Update cached mono char width
        self.mono_char_width =
            cx.text
                .measure_styled_mono("M", self.style.font_size, FontStyle::default());

        // Background with configurable opacity
        if self.background_opacity > 0.0 {
            let bg = self.style.background;
            let bg_with_opacity = Hsla::new(bg.h, bg.s, bg.l, bg.a * self.background_opacity);
            cx.scene
                .draw_quad(Quad::new(bounds).with_background(bg_with_opacity));
        }

        let line_height = self.style.font_size * self.style.line_height;
        let status_bar_height = 24.0;
        let visible_height = bounds.size.height - self.style.padding * 2.0 - status_bar_height;

        // Center content with max width 768px
        let max_content_width = self.scaled_max_content_width();
        let content_width = bounds.size.width.min(max_content_width);
        let content_x = bounds.origin.x + (bounds.size.width - content_width) / 2.0;
        let text_x = content_x + self.style.padding;

        // Calculate available width for text (content width minus padding on both sides)
        let available_text_width = content_width - self.style.padding * 2.0;
        let max_chars = if self.style.wrap_text {
            self.max_chars_per_line(available_text_width, self.mono_char_width)
        } else {
            usize::MAX
        };

        // Parse block types for all lines
        let mut block_parser = BlockParser::new();
        let block_types: Vec<BlockType> = self
            .lines
            .iter()
            .enumerate()
            .map(|(i, line)| block_parser.detect_block_type_at(line, i))
            .collect();

        // Compute wrapped lines info: (logical_line_idx, segment_start_col, segment_text, visual_row)
        let mut wrapped_info: Vec<(usize, usize, String, usize)> = Vec::new();
        let mut visual_row = 0;
        let mut cursor_visual_row = 0;
        let mut cursor_visual_col = 0;

        for (line_idx, line) in self.lines.iter().enumerate() {
            // Add title margin (extra row) after line 0
            if line_idx == 1 {
                visual_row += 1;
            }

            let segments = if self.style.wrap_text {
                self.wrap_line(line, max_chars)
            } else {
                vec![(0, line.clone())]
            };

            for (seg_idx, (start_col, segment)) in segments.iter().enumerate() {
                // Track cursor position in visual coordinates
                if line_idx == self.cursor.line && self.cursor.column >= *start_col {
                    let segment_end = start_col + segment.chars().count();
                    if self.cursor.column <= segment_end || seg_idx == segments.len() - 1 {
                        cursor_visual_row = visual_row;
                        cursor_visual_col = self.cursor.column - start_col;
                    }
                }

                wrapped_info.push((line_idx, *start_col, segment.clone(), visual_row));
                visual_row += 1;
            }
        }

        let total_visual_rows = visual_row;

        // Calculate visible row range
        let first_visible_row = (self.scroll_offset / line_height).floor() as usize;
        let visible_rows = (visible_height / line_height).ceil() as usize + 1;
        let last_visible_row = (first_visible_row + visible_rows).min(total_visual_rows);

        // Render visible wrapped segments
        for (line_idx, start_col, segment, vis_row) in wrapped_info.iter() {
            if *vis_row < first_visible_row || *vis_row >= last_visible_row {
                continue;
            }

            let y = bounds.origin.y + self.style.padding + (*vis_row as f32 * line_height)
                - self.scroll_offset;

            // Skip if outside visible area
            if y + line_height < bounds.origin.y || y > bounds.origin.y + bounds.size.height {
                continue;
            }

            let block_type = block_types
                .get(*line_idx)
                .copied()
                .unwrap_or(BlockType::Paragraph);
            let is_cursor_line = *line_idx == self.cursor.line;

            if segment.is_empty() {
                // Empty segment, nothing to render
            } else if is_cursor_line {
                // Cursor line: render raw markdown but keep font size for headers
                let font_size = match block_type {
                    BlockType::Header(level) => self.style.font_size * header_font_scale(level),
                    _ => self.style.font_size,
                };
                let text_run = cx.text.layout_styled_mono(
                    segment,
                    Point::new(text_x, y),
                    font_size,
                    self.style.text_color,
                    FontStyle::default(),
                );
                cx.scene.draw_text(text_run);
            } else {
                // Non-cursor line: render formatted markdown
                // is_continuation is true when start_col > 0 (wrapped segment, not start of line)
                let is_continuation = *start_col > 0;
                self.render_formatted_line(
                    segment,
                    block_type,
                    text_x,
                    y,
                    line_height,
                    is_continuation,
                    cx,
                );
            }

            // Selection highlight for this segment
            if let Some(sel) = &self.selection
                && !sel.is_empty()
            {
                let sel_start = sel.start();
                let sel_end = sel.end();

                if *line_idx >= sel_start.line && *line_idx <= sel_end.line {
                    let segment_end_col = start_col + segment.chars().count();

                    // Calculate selection range within this segment
                    let line_sel_start = if *line_idx == sel_start.line {
                        sel_start.column
                    } else {
                        0
                    };
                    let line_sel_end = if *line_idx == sel_end.line {
                        sel_end.column
                    } else {
                        self.line_len(*line_idx)
                    };

                    // Intersect with segment range
                    let seg_sel_start = line_sel_start.max(*start_col).saturating_sub(*start_col);
                    let seg_sel_end = line_sel_end.min(segment_end_col).saturating_sub(*start_col);

                    if seg_sel_start < seg_sel_end {
                        let char_width = match block_type {
                            BlockType::Header(level) => {
                                let scale = header_font_scale(level);
                                cx.text.measure_styled_mono(
                                    "M",
                                    self.style.font_size * scale,
                                    FontStyle::default(),
                                )
                            }
                            _ => self.mono_char_width,
                        };
                        let sel_x = text_x + seg_sel_start as f32 * char_width;
                        let sel_width = (seg_sel_end - seg_sel_start) as f32 * char_width;

                        cx.scene.draw_quad(
                            Quad::new(Bounds::new(sel_x, y, sel_width, line_height))
                                .with_background(self.style.selection_color),
                        );
                    }
                }
            }
        }

        // Cursor with blinking (500ms on, 500ms off)
        if self.focused {
            let elapsed = self.cursor_blink_start.elapsed().as_millis();
            let cursor_visible = (elapsed / 500).is_multiple_of(2);

            if cursor_visible {
                let cursor_y =
                    bounds.origin.y + self.style.padding + (cursor_visual_row as f32 * line_height)
                        - self.scroll_offset;

                // Get cursor char width - scale for headers
                let cursor_block_type = block_types
                    .get(self.cursor.line)
                    .copied()
                    .unwrap_or(BlockType::Paragraph);
                let cursor_char_width = match cursor_block_type {
                    BlockType::Header(level) => {
                        let scale = header_font_scale(level);
                        cx.text.measure_styled_mono(
                            "M",
                            self.style.font_size * scale,
                            FontStyle::default(),
                        )
                    }
                    _ => self.mono_char_width,
                };

                let cursor_x = text_x + cursor_visual_col as f32 * cursor_char_width;
                // Shift cursor up slightly to align with text
                let cursor_offset_y = -2.0;

                // Block cursor in vim normal/visual mode, line cursor otherwise
                let (cursor_width, cursor_color) = if self.vim_enabled {
                    match self.vim.mode {
                        VimMode::Normal
                        | VimMode::Visual
                        | VimMode::VisualLine
                        | VimMode::VisualBlock => {
                            // Block cursor with semi-transparent background
                            (cursor_char_width, self.style.cursor_color.with_alpha(0.7))
                        }
                        VimMode::Insert | VimMode::Replace => {
                            // Line cursor
                            (2.0, self.style.cursor_color)
                        }
                    }
                } else {
                    // Standard line cursor when vim disabled
                    (2.0, self.style.cursor_color)
                };

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        cursor_x,
                        cursor_y + cursor_offset_y,
                        cursor_width,
                        line_height,
                    ))
                    .with_background(cursor_color),
                );
            }
        }

        // Scrollbar
        let total_content_height = total_visual_rows as f32 * line_height;
        if total_content_height > visible_height {
            let scrollbar_width = 8.0;
            let scrollbar_x = bounds.origin.x + bounds.size.width - scrollbar_width - 2.0;

            // Track
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    scrollbar_x,
                    bounds.origin.y + self.style.padding,
                    scrollbar_width,
                    visible_height,
                ))
                .with_background(Hsla::new(0.0, 0.0, 0.3, 0.2)),
            );

            // Thumb
            let thumb_ratio = visible_height / total_content_height;
            let thumb_height = (visible_height * thumb_ratio).max(20.0);
            let scroll_ratio = self.scroll_offset / (total_content_height - visible_height);
            let thumb_y = bounds.origin.y
                + self.style.padding
                + scroll_ratio * (visible_height - thumb_height);

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    scrollbar_x,
                    thumb_y,
                    scrollbar_width,
                    thumb_height,
                ))
                .with_background(Hsla::new(0.0, 0.0, 0.5, 0.5))
                .with_corner_radius(4.0),
            );
        }

        // Status bar at bottom
        let status_bar_y = bounds.origin.y + bounds.size.height - status_bar_height;
        let status_y = status_bar_y + 4.0;

        // Vim mode indicator (left side)
        if let Some(vim_mode) = self.vim_mode() {
            let mode_text = vim_mode.label();
            let mode_color = match vim_mode {
                VimMode::Normal => Hsla::new(210.0, 0.7, 0.6, 1.0), // Blue
                VimMode::Insert => Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                VimMode::Replace => Hsla::new(30.0, 0.7, 0.6, 1.0), // Orange
                VimMode::Visual | VimMode::VisualLine | VimMode::VisualBlock => {
                    Hsla::new(280.0, 0.6, 0.6, 1.0)
                } // Purple
            };

            let mode_x = bounds.origin.x + 12.0;
            let mode_run = cx.text.layout_styled_mono(
                mode_text,
                Point::new(mode_x, status_y),
                STATUS_BAR_FONT_SIZE,
                mode_color,
                FontStyle::default(),
            );
            cx.scene.draw_text(mode_run);
        }

        // Status message (center-left, after vim mode)
        if let Some((message, color)) = &self.status_message {
            let status_msg_x = bounds.origin.x + 100.0; // After vim mode indicator
            let status_msg_run = cx.text.layout_styled_mono(
                message,
                Point::new(status_msg_x, status_y),
                STATUS_BAR_FONT_SIZE,
                *color,
                FontStyle::default(),
            );
            cx.scene.draw_text(status_msg_run);
        }

        // Line:Col indicator (right side)
        let status_text = format!(
            "Ln {}, Col {}",
            self.cursor.line + 1,
            self.cursor.column + 1
        );
        let status_x = bounds.origin.x + bounds.size.width - 120.0;
        let status_run = cx.text.layout_styled_mono(
            &status_text,
            Point::new(status_x, status_y),
            STATUS_BAR_FONT_SIZE,
            Hsla::new(0.0, 0.0, 0.5, 1.0),
            FontStyle::default(),
        );
        cx.scene.draw_text(status_run);
    }
}
