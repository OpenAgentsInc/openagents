impl AppState {
    pub(crate) fn build_chat_layout(
        &mut self,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
    ) -> ChatLayout {
        let viewport_top = OUTPUT_PADDING;
        // Calculate input width for wrapping
        let max_input_width = 768.0_f32;
        let available_input_width = sidebar_layout.main.size.width - INPUT_PADDING * 2.0;
        let input_width = available_input_width.min(max_input_width);
        // Set max width for wrapping, then calculate dynamic height
        self.input.set_max_width(input_width);
        let input_height = self.input.current_height().max(40.0);
        let viewport_bottom =
            logical_height - input_height - INPUT_PADDING * 2.0 - STATUS_BAR_HEIGHT - 16.0;
        let viewport_height = (viewport_bottom - viewport_top).max(0.0);

        // Apply max width 768px and center content (matching input container)
        let max_content_width = 768.0_f32;
        let full_available_width = sidebar_layout.main.size.width - OUTPUT_PADDING * 2.0;
        let available_width = full_available_width.min(max_content_width);
        let content_x = sidebar_layout.main.origin.x
            + (sidebar_layout.main.size.width - available_width) / 2.0;

        let chat_font_size = self.settings.coder_settings.font_size;
        let chat_line_height = (chat_font_size * 1.4).round();
        let char_width = chat_font_size * 0.6;
        let max_chars = (available_width / char_width).max(1.0) as usize;

        let mut message_layouts = Vec::with_capacity(self.chat.messages.len());
        let mut inline_tools_layouts: Vec<InlineToolsLayout> = Vec::new();
        let mut dspy_stage_layouts: Vec<DspyStageLayout> = Vec::new();
        let mut total_content_height = 0.0_f32;

        // Group tools by message_index
        let mut tools_by_message: HashMap<usize, Vec<usize>> = HashMap::new();
        for (tool_idx, tool) in self.tools.tool_history.iter().enumerate() {
            tools_by_message
                .entry(tool.message_index)
                .or_default()
                .push(tool_idx);
        }

        // Group DSPy stages by message_index
        let mut dspy_by_message: HashMap<usize, Vec<usize>> = HashMap::new();
        for (stage_idx, stage) in self.tools.dspy_stages.iter().enumerate() {
            dspy_by_message
                .entry(stage.message_index)
                .or_default()
                .push(stage_idx);
        }

        for index in 0..self.chat.messages.len() {
            let (role, content, document) = {
                let msg = &self.chat.messages[index];
                (msg.role, msg.content.clone(), msg.document.clone())
            };
            let layout = match role {
                MessageRole::User => self.layout_user_message(
                    index,
                    &content,
                    content_x,
                    chat_font_size,
                    chat_line_height,
                    max_chars,
                ),
                MessageRole::Assistant => self.layout_assistant_message(
                    index,
                    &content,
                    document.as_ref(),
                    content_x,
                    available_width,
                    chat_line_height,
                    max_chars,
                ),
            };
            total_content_height += layout.height;
            message_layouts.push(layout);

            // Add DSPy stage cards for this message (before tools)
            if let Some(stage_indices) = dspy_by_message.get(&index) {
                for &stage_idx in stage_indices {
                    let stage_height = self.measure_dspy_stage_height(stage_idx, available_width);
                    dspy_stage_layouts.push(DspyStageLayout {
                        message_index: index,
                        y_offset: total_content_height,
                        height: stage_height,
                        stage_index: stage_idx,
                    });
                    total_content_height += stage_height + TOOL_PANEL_GAP;
                }
            }

            // Add inline tools for this message
            if let Some(tool_indices) = tools_by_message.get(&index) {
                if !tool_indices.is_empty() {
                    let inline_layout = self.build_inline_tools_layout(
                        index,
                        tool_indices,
                        content_x,
                        available_width,
                        total_content_height,
                    );
                    total_content_height += inline_layout.height + TOOL_PANEL_GAP;
                    inline_tools_layouts.push(inline_layout);
                }
            }
        }

        let streaming_height = if !self.chat.streaming_markdown.source().is_empty() {
            let doc = self.chat.streaming_markdown.document();
            let size = self
                .chat
                .markdown_renderer
                .measure(doc, available_width, &mut self.text_system);
            size.height + chat_line_height
        } else if self.chat.is_thinking {
            chat_line_height
        } else {
            0.0
        };
        total_content_height += streaming_height;

        // Add inline tools for streaming/current message (last message index or beyond)
        let streaming_msg_index = self.chat.messages.len().saturating_sub(1);
        if let Some(tool_indices) = tools_by_message.get(&streaming_msg_index) {
            // Check if we already added tools for this message above
            let already_added = inline_tools_layouts
                .iter()
                .any(|l| l.message_index == streaming_msg_index);
            if !already_added && !tool_indices.is_empty() {
                let inline_layout = self.build_inline_tools_layout(
                    streaming_msg_index,
                    tool_indices,
                    content_x,
                    available_width,
                    total_content_height,
                );
                total_content_height += inline_layout.height + TOOL_PANEL_GAP;
                inline_tools_layouts.push(inline_layout);
            }
        }

        let max_scroll = (total_content_height - viewport_height).max(0.0);
        self.chat.scroll_offset = self.chat.scroll_offset.clamp(0.0, max_scroll);
        let was_near_bottom = self.chat.scroll_offset >= max_scroll - chat_line_height * 2.0;
        if self.settings.coder_settings.auto_scroll && self.tools.has_running() && was_near_bottom {
            self.chat.scroll_offset = max_scroll;
        }

        if let Some(selection) = self.chat.chat_selection {
            if selection.anchor.message_index >= message_layouts.len()
                || selection.focus.message_index >= message_layouts.len()
            {
                self.chat.chat_selection = None;
            }
        }

        // Apply scroll offset to message Y positions
        let scroll_adjust = viewport_top - self.chat.scroll_offset;
        let mut y = scroll_adjust;
        let mut inline_tools_idx = 0;
        let mut dspy_stages_idx = 0;
        for (msg_idx, layout) in message_layouts.iter_mut().enumerate() {
            for line in &mut layout.lines {
                line.y += y;
            }
            y += layout.height;

            // Adjust DSPy stage Y positions for this message
            while dspy_stages_idx < dspy_stage_layouts.len()
                && dspy_stage_layouts[dspy_stages_idx].message_index == msg_idx
            {
                let dsl = &mut dspy_stage_layouts[dspy_stages_idx];
                dsl.y_offset += scroll_adjust;
                y += dsl.height + TOOL_PANEL_GAP;
                dspy_stages_idx += 1;
            }

            // Adjust inline tools Y positions for this message
            if inline_tools_idx < inline_tools_layouts.len()
                && inline_tools_layouts[inline_tools_idx].message_index == msg_idx
            {
                let itl = &mut inline_tools_layouts[inline_tools_idx];
                itl.y_offset += scroll_adjust;
                for block in &mut itl.blocks {
                    block.card_bounds.origin.y += scroll_adjust;
                    if let Some(ref mut db) = block.detail_bounds {
                        db.origin.y += scroll_adjust;
                    }
                }
                y += itl.height + TOOL_PANEL_GAP;
                inline_tools_idx += 1;
            }
        }

        // Handle any remaining DSPy stages (for streaming message)
        while dspy_stages_idx < dspy_stage_layouts.len() {
            let dsl = &mut dspy_stage_layouts[dspy_stages_idx];
            dsl.y_offset += scroll_adjust;
            dspy_stages_idx += 1;
        }

        // Handle any remaining inline tools (for streaming message)
        while inline_tools_idx < inline_tools_layouts.len() {
            let itl = &mut inline_tools_layouts[inline_tools_idx];
            itl.y_offset += scroll_adjust;
            for block in &mut itl.blocks {
                block.card_bounds.origin.y += scroll_adjust;
                if let Some(ref mut db) = block.detail_bounds {
                    db.origin.y += scroll_adjust;
                }
            }
            inline_tools_idx += 1;
        }

        ChatLayout {
            viewport_top,
            viewport_bottom,
            content_x,
            available_width,
            chat_font_size,
            chat_line_height,
            message_layouts,
            streaming_height,
            inline_tools: inline_tools_layouts,
            dspy_stages: dspy_stage_layouts,
        }
    }

    fn build_inline_tools_layout(
        &self,
        message_index: usize,
        tool_indices: &[usize],
        content_x: f32,
        available_width: f32,
        y_offset: f32,
    ) -> InlineToolsLayout {
        let panel_x = content_x;
        let panel_width = available_width;

        let mut blocks = Vec::new();
        let mut block_y = y_offset + TOOL_PANEL_GAP;
        let mut total_height = TOOL_PANEL_GAP;

        for (i, &tool_idx) in tool_indices.iter().enumerate() {
            let tool = &self.tools.tool_history[tool_idx];
            let card_height = tool.card.size_hint().1.unwrap_or(22.0);
            let card_bounds = Bounds::new(panel_x, block_y, panel_width, card_height);
            block_y += card_height;
            total_height += card_height;

            let detail_height = tool.detail.height();
            let detail_bounds = if detail_height > 0.0 {
                block_y += TOOL_PANEL_GAP;
                total_height += TOOL_PANEL_GAP;
                let db = Bounds::new(panel_x, block_y, panel_width, detail_height);
                block_y += detail_height;
                total_height += detail_height;
                Some(db)
            } else {
                None
            };

            blocks.push(ToolPanelBlock {
                index: tool_idx,
                card_bounds,
                detail_bounds,
            });

            if i + 1 < tool_indices.len() {
                block_y += TOOL_PANEL_GAP;
                total_height += TOOL_PANEL_GAP;
            }
        }

        InlineToolsLayout {
            message_index,
            y_offset,
            height: total_height,
            blocks,
        }
    }

    fn measure_dspy_stage_height(&self, stage_idx: usize, _available_width: f32) -> f32 {
        let stage_viz = &self.tools.dspy_stages[stage_idx];
        match &stage_viz.stage {
            DspyStage::EnvironmentAssessment { .. } => 160.0,
            DspyStage::Planning { implementation_steps, .. } => {
                80.0 + (implementation_steps.len() as f32 * 20.0).min(200.0)
            }
            DspyStage::TodoList { tasks } => 60.0 + (tasks.len() as f32 * 24.0).min(240.0),
            DspyStage::ExecutingTask { .. } => 60.0,
            DspyStage::TaskComplete { .. } => 40.0,
            DspyStage::Complete { .. } => 80.0,
        }
    }

    fn layout_user_message(
        &mut self,
        message_index: usize,
        content: &str,
        content_x: f32,
        chat_font_size: f32,
        chat_line_height: f32,
        max_chars: usize,
    ) -> MessageLayout {
        let content_with_prefix = format!("> {}", content);
        let wrapped_lines = wrap_text(&content_with_prefix, max_chars);
        let line_count = wrapped_lines.len();
        let mut builder = MessageLayoutBuilder::new(message_index);
        let mut y = chat_line_height * 0.5;
        for line in wrapped_lines {
            builder.push_line(line, content_x, y, chat_line_height, chat_font_size);
            y += chat_line_height;
        }
        let height = chat_line_height * 0.5 + line_count as f32 * chat_line_height
            + chat_line_height * 0.5;
        builder.build(height)
    }

    fn layout_assistant_message(
        &mut self,
        message_index: usize,
        content: &str,
        document: Option<&MarkdownDocument>,
        content_x: f32,
        available_width: f32,
        chat_line_height: f32,
        max_chars: usize,
    ) -> MessageLayout {
        if let Some(doc) = document {
            let config = crate::app::build_markdown_config(&self.settings.coder_settings);
            let mut builder = MessageLayoutBuilder::new(message_index);
            let height = layout_markdown_document(
                doc,
                Point::new(content_x, 0.0),
                available_width,
                &mut self.text_system,
                &config,
                &mut builder,
            );
            builder.build(height + chat_line_height)
        } else {
            let wrapped_lines = wrap_text(content, max_chars);
            let line_count = wrapped_lines.len();
            let mut builder = MessageLayoutBuilder::new(message_index);
            let mut y = 0.0;
            for line in wrapped_lines {
                builder.push_line(
                    line,
                    content_x,
                    y,
                    chat_line_height,
                    self.settings.coder_settings.font_size,
                );
                y += chat_line_height;
            }
            let height = line_count as f32 * chat_line_height;
            builder.build(height)
        }
    }

