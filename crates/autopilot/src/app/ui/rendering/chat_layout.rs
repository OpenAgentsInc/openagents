impl AppState {
    pub(crate) fn build_chat_layout(
        &mut self,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
    ) -> ChatLayout {
        let viewport_top = TOPBAR_HEIGHT + OUTPUT_PADDING;
        let input_layout = self.build_input_layout(sidebar_layout, logical_height);
        let viewport_bottom = input_layout.area_bounds.origin.y - 16.0;
        let viewport_height = (viewport_bottom - viewport_top).max(0.0);

        let full_available_width = sidebar_layout.main.size.width - CONTENT_PADDING_X * 2.0;
        let available_width = full_available_width.max(0.0);
        let content_x = sidebar_layout.main.origin.x + CONTENT_PADDING_X;

        let chat_font_size = self.settings.coder_settings.font_size;
        let chat_line_height = (chat_font_size * 1.4).round();
        let char_width = chat_font_size * 0.6;
        let max_chars = (available_width / char_width).max(1.0) as usize;

        let mut message_layouts = Vec::with_capacity(self.chat.messages.len());
        let mut inline_tools_layouts: Vec<InlineToolsLayout> = Vec::new();
        let mut dspy_stage_layouts: Vec<DspyStageLayout> = Vec::new();
        let mut total_content_height = 0.0_f32;

        // Calculate boot section layouts (displayed at top of chat)
        let boot_section_font_size = 13.0_f32;
        let boot_section_header_height = boot_section_font_size * 1.6;
        let boot_section_line_height = boot_section_font_size * 1.4;
        let boot_section_gap = 8.0_f32;

        let mut boot_section_layouts: Vec<BootSectionLayout> = Vec::new();
        if let Some(sections) = &self.chat.boot_sections {
            // Initialize section
            let init = &sections.initialize;
            if init.active || !init.details.is_empty() || init.status != SectionStatus::Pending {
                let init_height = if init.expanded && !init.details.is_empty() {
                    boot_section_header_height
                        + 4.0
                        + (init.details.len() as f32 * boot_section_line_height)
                } else {
                    boot_section_header_height
                };
                boot_section_layouts.push(BootSectionLayout {
                    y_offset: total_content_height,
                    height: init_height,
                    summary: init.summary.clone(),
                    details: init.details.clone(),
                    status: init.status,
                    expanded: init.expanded,
                    section_id: init.id,
                });
                total_content_height += init_height + boot_section_gap;
            }

            // Suggest issues section
            let suggest = &sections.suggest_issues;
            if suggest.active || !suggest.details.is_empty() || suggest.status != SectionStatus::Pending
            {
                let suggest_height = if suggest.expanded && !suggest.details.is_empty() {
                    boot_section_header_height
                        + 4.0
                        + (suggest.details.len() as f32 * boot_section_line_height)
                } else {
                    boot_section_header_height
                };
                boot_section_layouts.push(BootSectionLayout {
                    y_offset: total_content_height,
                    height: suggest_height,
                    summary: suggest.summary.clone(),
                    details: suggest.details.clone(),
                    status: suggest.status,
                    expanded: suggest.expanded,
                    section_id: suggest.id,
                });
                total_content_height += suggest_height + boot_section_gap;
            }
        }

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
                MessageRole::Assistant | MessageRole::AssistantThought => self
                    .layout_assistant_message(
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

        // Calculate streaming thought height (reasoning/thinking content)
        let streaming_thought_height = if !self.chat.streaming_thought.source().is_empty() {
            let doc = self.chat.streaming_thought.document();
            let size = self
                .chat
                .markdown_renderer
                .measure(doc, available_width, &mut self.text_system);
            size.height + chat_line_height
        } else {
            0.0
        };
        total_content_height += streaming_thought_height;

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

        // Add DSPy stages and inline tools for streaming/current message
        // During streaming, these are associated with messages.len() (the next message index)
        let streaming_msg_index = self.chat.messages.len();

        // DSPy stages for streaming message (positioned after streaming content)
        if let Some(stage_indices) = dspy_by_message.get(&streaming_msg_index) {
            for &stage_idx in stage_indices {
                let stage_height = self.measure_dspy_stage_height(stage_idx, available_width);
                dspy_stage_layouts.push(DspyStageLayout {
                    message_index: streaming_msg_index,
                    y_offset: total_content_height,
                    height: stage_height,
                    stage_index: stage_idx,
                });
                total_content_height += stage_height + TOOL_PANEL_GAP;
            }
        }

        // Inline tools for streaming message - positioned after streaming content
        if let Some(tool_indices) = tools_by_message.get(&streaming_msg_index) {
            if !tool_indices.is_empty() {
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

        // Apply scroll offset to boot section Y positions
        let scroll_adjust = viewport_top - self.chat.scroll_offset;
        for bsl in &mut boot_section_layouts {
            bsl.y_offset += scroll_adjust;
        }

        // Apply scroll offset to message Y positions
        let mut y = scroll_adjust;
        // Start y after boot sections
        for bsl in &boot_section_layouts {
            y += bsl.height + boot_section_gap;
        }
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
            boot_sections: boot_section_layouts,
            message_layouts,
            streaming_thought_height,
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

    fn measure_dspy_stage_height(&self, stage_idx: usize, available_width: f32) -> f32 {
        let stage_viz = &self.tools.dspy_stages[stage_idx];
        let stage = &stage_viz.stage;

        // Match dspy.rs rendering constants
        let padding = 12.0;
        let font_size = 13.0;
        let small_font_size = 11.0;
        let line_height = font_size * 1.4;
        let small_line_height = small_font_size * 1.4;
        let content_width = (available_width - padding * 2.0).max(0.0);
        let wrap_chars_small =
            ((content_width / (small_font_size * 0.6)).floor().max(16.0)) as usize;
        let wrap_chars_large =
            ((content_width / (font_size * 0.6)).floor().max(12.0)) as usize;

        let header_height = padding + line_height + 8.0;

        let content_height = match stage {
            DspyStage::EnvironmentAssessment {
                system_info,
                workspace,
                active_directive,
                open_issues,
                priority_action,
                urgency,
                reasoning,
                ..
            } => {
                let priority_line = format!("{} ({})", priority_action, urgency);

                let items: Vec<(&str, String)> = vec![
                    ("System", truncate_preview(system_info, 120)),
                    ("Workspace", truncate_preview(workspace, 120)),
                    ("Directive", truncate_preview(active_directive.as_deref().unwrap_or("None"), 120)),
                    ("Open issues", open_issues.to_string()),
                    ("Priority", priority_line),
                    ("Reasoning", truncate_preview(reasoning, 140)),
                ];

                let mut h = 0.0;
                for (label, text) in items {
                    if label == "Reasoning" && text.is_empty() {
                        continue;
                    }
                    let line = format!("{}: {}", label, text);
                    let wrapped = wrap_text(&line, wrap_chars_small);
                    h += wrapped.len() as f32 * small_line_height + 4.0;
                }
                h
            }
            DspyStage::Planning {
                analysis,
                implementation_steps,
                test_strategy,
                ..
            } => {
                let analysis_line = format!(
                    "Analysis: {}",
                    strip_markdown_markers(&truncate_preview(analysis, 160))
                );
                let wrapped_analysis = wrap_text(&analysis_line, wrap_chars_small);
                let test_line = format!(
                    "Test: {}",
                    strip_markdown_markers(&truncate_preview(test_strategy, 160))
                );
                let wrapped_test = wrap_text(&test_line, wrap_chars_small);
                let step_lines: usize = implementation_steps
                    .iter()
                    .map(|step| {
                        let clean_step =
                            strip_markdown_markers(&truncate_preview(step, 80));
                        wrap_text(&clean_step, wrap_chars_small).len().max(1)
                    })
                    .sum();

                wrapped_analysis.len() as f32 * small_line_height + 4.0
                    + small_line_height + 4.0  // complexity line
                    + wrapped_test.len() as f32 * small_line_height + 6.0
                    + step_lines as f32 * small_line_height
            }
            DspyStage::TodoList { tasks } => {
                let task_lines: usize = tasks
                    .iter()
                    .map(|task| {
                        let status_symbol = match task.status {
                            crate::autopilot_loop::TodoStatus::Pending => "□",
                            crate::autopilot_loop::TodoStatus::InProgress => "◐",
                            crate::autopilot_loop::TodoStatus::Complete => "✓",
                            crate::autopilot_loop::TodoStatus::Failed => "✗",
                        };
                        let clean_desc = strip_markdown_markers(&truncate_preview(&task.description, 80));
                        let line = format!("{} {}", status_symbol, clean_desc);
                        wrap_text(&line, wrap_chars_small).len().max(1)
                    })
                    .sum();
                task_lines as f32 * small_line_height
            }
            DspyStage::ExecutingTask {
                task_index,
                total_tasks,
                task_description,
            } => {
                let clean_desc =
                    strip_markdown_markers(&truncate_preview(task_description, 60));
                let line = format!("Task {}/{}: {}", task_index, total_tasks, clean_desc);
                wrap_text(&line, wrap_chars_large).len().max(1) as f32 * line_height
            }
            DspyStage::TaskComplete { task_index, success } => {
                let line = if *success {
                    format!("Task {} completed", task_index)
                } else {
                    format!("Task {} failed", task_index)
                };
                wrap_text(&line, wrap_chars_large).len().max(1) as f32 * line_height
            }
            DspyStage::Complete {
                total_tasks,
                successful,
                failed,
            } => {
                let line = format!(
                    "Completed {} tasks: {} successful, {} failed",
                    total_tasks, successful, failed
                );
                wrap_text(&line, wrap_chars_large).len().max(1) as f32 * line_height
            }
            DspyStage::IssueSuggestions {
                suggestions,
                filtered_count,
                confidence,
                await_selection,
            } => {
                let status = if *await_selection {
                    "Awaiting selection..."
                } else {
                    "Auto-selecting..."
                };
                let status_line = format!("Confidence: {:.0}% · {}", confidence * 100.0, status);
                let status_lines = wrap_text(&status_line, wrap_chars_small).len().max(1);
                let mut h = status_lines as f32 * small_line_height + 4.0;

                for suggestion in suggestions {
                    let title_line = format!(
                        "[#{}] {} ({})",
                        suggestion.number,
                        truncate_preview(&suggestion.title, 50),
                        suggestion.priority
                    );
                    let rationale_line =
                        format!("\"{}\"", truncate_preview(&suggestion.rationale, 60));
                    let complexity_line = format!("Complexity: {}", suggestion.complexity);

                    h += wrap_text(&title_line, wrap_chars_small).len().max(1) as f32
                        * small_line_height;
                    h += wrap_text(&rationale_line, wrap_chars_small).len().max(1) as f32
                        * small_line_height;
                    h += wrap_text(&complexity_line, wrap_chars_small).len().max(1) as f32
                        * small_line_height;
                    h += 4.0;
                }
                if *filtered_count > 0 {
                    let filtered_line =
                        format!("[{} issues filtered as stale/blocked]", filtered_count);
                    h += wrap_text(&filtered_line, wrap_chars_small).len().max(1) as f32
                        * small_line_height;
                }
                h
            }
            DspyStage::IssueSelected {
                number,
                title,
                selection_method,
            } => {
                let line = format!(
                    "Selected issue #{}: {} ({})",
                    number,
                    truncate_preview(title, 50),
                    selection_method
                );
                wrap_text(&line, wrap_chars_large).len().max(1) as f32 * line_height
            }
            DspyStage::UnblockSuggestion {
                issue_number,
                title,
                blocked_reason,
                unblock_rationale,
                unblock_strategy,
                estimated_effort,
                other_blocked_count,
            } => {
                let title_line =
                    format!("#{} {}", issue_number, truncate_preview(title, 50));
                let blocked_line =
                    format!("Blocked: \"{}\"", truncate_preview(blocked_reason, 60));
                let why_line = format!("Why: {}", truncate_preview(unblock_rationale, 60));
                let strategy_line =
                    format!("Strategy: {}", truncate_preview(unblock_strategy, 55));
                let effort_line =
                    format!("Effort: {} | {} other blocked", estimated_effort, other_blocked_count);

                let title_lines = wrap_text(&title_line, wrap_chars_large).len().max(1);
                let blocked_lines = wrap_text(&blocked_line, wrap_chars_small).len().max(1);
                let why_lines = wrap_text(&why_line, wrap_chars_small).len().max(1);
                let strategy_lines = wrap_text(&strategy_line, wrap_chars_small).len().max(1);
                let effort_lines = wrap_text(&effort_line, wrap_chars_small).len().max(1);

                title_lines as f32 * line_height
                    + 4.0
                    + blocked_lines as f32 * small_line_height
                    + 4.0
                    + why_lines as f32 * small_line_height
                    + strategy_lines as f32 * small_line_height
                    + effort_lines as f32 * small_line_height
            }
        };

        header_height + content_height + padding
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
            let config = crate::app::build_markdown_config(
                &self.settings.coder_settings,
                self.resolved_theme(),
            );
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
}
