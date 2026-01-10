use std::collections::HashMap;

use wgpui::components::PaintContext;
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::organisms::{
    ChildTool, DiffLine, DiffLineKind, DiffToolCall, EventData, EventInspector, InspectorView,
    PermissionDialog, SearchMatch, SearchToolCall, TagData, TerminalToolCall, ToolCallCard,
};
use wgpui::markdown::{MarkdownBlock, MarkdownConfig, MarkdownDocument, StyledLine};
use wgpui::{copy_to_clipboard, Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::app::AppState;
use crate::app::catalog::{
    describe_mcp_config, AgentSource, HookScriptSource, McpServerSource, SkillSource,
};
use crate::app::chat::{
    ChatLayout, ChatLineLayout, ChatSelection, ChatSelectionPoint, InlineToolsLayout, MessageLayout,
    MessageLayoutBuilder, MessageRole,
};
use crate::app::config::{SettingsItem, SettingsTab};
use crate::app::events::{keybinding_labels, ModalState};
use crate::app::tools::{DspyStageLayout, ToolPanelBlock};
use crate::app::ui::{palette_for, split_into_words_for_layout, wrap_text, UiPalette};
use crate::app::{
    format_relative_time, hook_event_label, settings_rows, truncate_preview, CoderMode,
    HookModalView, HookSetting, ModelOption, SettingsInputMode, SettingsSnapshot,
};
use crate::keybindings::Action as KeyAction;
use crate::autopilot_loop::DspyStage;

pub(crate) const INPUT_HEIGHT: f32 = 40.0;
pub(crate) const INPUT_PADDING: f32 = 12.0;
pub(crate) const OUTPUT_PADDING: f32 = 12.0;
pub(crate) const STATUS_BAR_HEIGHT: f32 = 20.0;
pub(crate) const STATUS_BAR_FONT_SIZE: f32 = 13.0;
/// Height of input area (input + padding + status bar) for modal positioning
pub(crate) const INPUT_AREA_HEIGHT: f32 = INPUT_HEIGHT + INPUT_PADDING + STATUS_BAR_HEIGHT;

pub(crate) const SESSION_MODAL_WIDTH: f32 = 760.0;
pub(crate) const SESSION_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const SESSION_CARD_HEIGHT: f32 = 100.0;
pub(crate) const SESSION_CARD_GAP: f32 = 12.0;
pub(crate) const SESSION_MODAL_PADDING: f32 = 16.0;
pub(crate) const SKILL_CARD_HEIGHT: f32 = 110.0;
pub(crate) const SETTINGS_MODAL_WIDTH: f32 = 760.0;
pub(crate) const SETTINGS_MODAL_HEIGHT: f32 = 480.0;
pub(crate) const SETTINGS_ROW_HEIGHT: f32 = 24.0;
pub(crate) const SETTINGS_TAB_HEIGHT: f32 = 22.0;
pub(crate) const HELP_MODAL_WIDTH: f32 = 760.0;
pub(crate) const HELP_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const HOOK_MODAL_WIDTH: f32 = 860.0;
pub(crate) const HOOK_MODAL_HEIGHT: f32 = 520.0;
pub(crate) const HOOK_EVENT_ROW_HEIGHT: f32 = 20.0;
pub(crate) const TOOL_PANEL_GAP: f32 = 8.0;

const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_MIN_MAIN: f32 = 320.0;

#[derive(Clone, Debug)]
struct LinePrefix {
    text: String,
    x: f32,
    content_x: f32,
    font_size: f32,
}

pub(crate) struct SidebarLayout {
    pub(crate) left: Option<Bounds>,
    pub(crate) right: Option<Bounds>,
    pub(crate) main: Bounds,
}

pub(crate) struct SessionListLayout {
    pub(crate) modal_bounds: Bounds,
    pub(crate) card_bounds: Vec<(usize, Bounds)>,
    pub(crate) checkpoint_bounds: Option<Bounds>,
}

pub(crate) struct AgentListLayout {
    pub(crate) card_bounds: Vec<(usize, Bounds)>,
}

pub(crate) struct SkillListLayout {
    pub(crate) card_bounds: Vec<(usize, Bounds)>,
}

pub(crate) struct HookEventLayout {
    pub(crate) list_bounds: Bounds,
    pub(crate) inspector_bounds: Bounds,
    pub(crate) row_bounds: Vec<(usize, Bounds)>,
}

pub(crate) fn modal_y_in_content(logical_height: f32, modal_height: f32) -> f32 {
    let content_height = logical_height - INPUT_AREA_HEIGHT;
    (content_height - modal_height) / 2.0
}

pub(crate) fn sidebar_layout(
    logical_width: f32,
    logical_height: f32,
    left_open: bool,
    right_open: bool,
) -> SidebarLayout {
    let mut left_width = if left_open { SIDEBAR_WIDTH } else { 0.0 };
    let mut right_width = if right_open { SIDEBAR_WIDTH } else { 0.0 };
    let available_main = logical_width - left_width - right_width;
    if available_main < SIDEBAR_MIN_MAIN {
        let overflow = SIDEBAR_MIN_MAIN - available_main;
        if left_width > 0.0 && right_width > 0.0 {
            let reduce = overflow / 2.0;
            left_width = (left_width - reduce).max(120.0);
            right_width = (right_width - reduce).max(120.0);
        } else if left_width > 0.0 {
            left_width = (left_width - overflow).max(120.0);
        } else if right_width > 0.0 {
            right_width = (right_width - overflow).max(120.0);
        }
    }
    let main_width = (logical_width - left_width - right_width).max(1.0);
    let main = Bounds::new(left_width, 0.0, main_width, logical_height);
    let left = if left_width > 0.0 {
        Some(Bounds::new(0.0, 0.0, left_width, logical_height))
    } else {
        None
    };
    let right = if right_width > 0.0 {
        Some(Bounds::new(
            logical_width - right_width,
            0.0,
            right_width,
            logical_height,
        ))
    } else {
        None
    };

    SidebarLayout { left, right, main }
}

pub(crate) fn new_session_button_bounds(sidebar_bounds: Bounds) -> Bounds {
    Bounds::new(
        sidebar_bounds.origin.x + 12.0,
        sidebar_bounds.origin.y + 12.0,
        sidebar_bounds.size.width - 24.0,
        32.0,
    )
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_duration_ms(ms: u64) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1_000;
        format!("{}m{}s", mins, secs)
    }
}

fn byte_offset_for_char_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

fn char_index_for_byte_offset(text: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(text.len());
    text[..clamped].chars().count()
}

fn prefix_text_for_line(prefix: &LinePrefix, text_system: &mut TextSystem) -> (String, f32) {
    let font_style = wgpui::text::FontStyle::default();
    let prefix_width = text_system.measure_styled_mono(&prefix.text, prefix.font_size, font_style);
    let space_width = text_system
        .measure_styled_mono(" ", prefix.font_size, font_style)
        .max(1.0);
    let gap_px = (prefix.content_x - prefix.x - prefix_width).max(space_width);
    let space_count = (gap_px / space_width).round().max(1.0) as usize;
    let mut text = prefix.text.clone();
    text.push_str(&" ".repeat(space_count));
    let total_width = prefix_width + space_width * space_count as f32;
    (text, total_width)
}

fn line_text_from_styled(lines: &[StyledLine]) -> String {
    let mut out = String::new();
    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            out.push_str(&span.text);
        }
    }
    out
}

fn layout_styled_lines(
    lines: &[StyledLine],
    origin: Point,
    max_width: f32,
    base_indent: u32,
    text_system: &mut TextSystem,
    builder: &mut MessageLayoutBuilder,
    prefix: &mut Option<LinePrefix>,
) -> f32 {
    let mut y = origin.y;
    let mut first_visual_line = true;

    for line in lines {
        y += line.margin_top;
        let indent = (base_indent + line.indent) as f32 * wgpui::theme::spacing::LG;
        let line_start_x = origin.x + indent;
        let right_edge = origin.x + max_width;

        let base_font_size = line
            .spans
            .first()
            .map(|s| s.style.font_size)
            .unwrap_or(wgpui::theme::font_size::BASE);
        let line_height = base_font_size * line.line_height;

        let mut current_x = line_start_x;
        let mut line_x = line_start_x;
        let mut current_line_text = String::new();

        if first_visual_line {
            if let Some(prefix_line) = prefix.take() {
                let (prefix_text, prefix_width) = prefix_text_for_line(&prefix_line, text_system);
                line_x = prefix_line.x;
                current_x = prefix_line.x + prefix_width;
                current_line_text.push_str(&prefix_text);
            }
        }

        for span in &line.spans {
            let font_style = wgpui::text::FontStyle {
                bold: span.style.bold,
                italic: span.style.italic,
            };
            let words = split_into_words_for_layout(&span.text);

            for word in words {
                if word.is_empty() {
                    continue;
                }

                let word_width =
                    text_system.measure_styled_mono(word, span.style.font_size, font_style);
                if current_x + word_width > right_edge && current_x > line_start_x {
                    builder.push_line(
                        current_line_text,
                        line_x,
                        y,
                        line_height,
                        base_font_size,
                    );
                    y += line_height;
                    current_line_text = String::new();
                    current_x = line_start_x;
                    line_x = line_start_x;
                }

                current_line_text.push_str(word);
                current_x += word_width;
            }
        }

        builder.push_line(
            current_line_text,
            line_x,
            y,
            line_height,
            base_font_size,
        );
        y += line_height;
        first_visual_line = false;
    }

    y - origin.y
}

fn layout_markdown_block(
    block: &MarkdownBlock,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    config: &MarkdownConfig,
    builder: &mut MessageLayoutBuilder,
    prefix: &mut Option<LinePrefix>,
) -> f32 {
    match block {
        MarkdownBlock::Paragraph(lines) => {
            layout_styled_lines(lines, origin, max_width, 0, text_system, builder, prefix)
        }
        MarkdownBlock::Header { level, lines } => {
            let margin_top = match level {
                1 => wgpui::theme::spacing::XL,
                2 => wgpui::theme::spacing::LG,
                _ => wgpui::theme::spacing::MD,
            };
            margin_top
                + layout_styled_lines(
                    lines,
                    Point::new(origin.x, origin.y + margin_top),
                    max_width,
                    0,
                    text_system,
                    builder,
                    prefix,
                )
        }
        MarkdownBlock::CodeBlock { lines, .. } => {
            let margin = wgpui::theme::spacing::SM;
            let padding = wgpui::theme::spacing::SM;
            let header_height = wgpui::theme::font_size::XS + wgpui::theme::spacing::XS;

            let content_origin = Point::new(
                origin.x + padding,
                origin.y + margin + header_height + padding,
            );

            let content_height = layout_styled_lines(
                lines,
                content_origin,
                max_width - padding * 2.0,
                0,
                text_system,
                builder,
                prefix,
            );

            content_height + padding * 2.0 + header_height + margin * 2.0
        }
        MarkdownBlock::Blockquote(blocks) => {
            let bar_width = 4.0;
            let gap = wgpui::theme::spacing::MD;
            let indent = bar_width + gap;
            let margin = wgpui::theme::spacing::SM;
            let start_y = origin.y + margin;
            let mut y = start_y;

            for block in blocks {
                y += layout_markdown_block(
                    block,
                    Point::new(origin.x + indent, y),
                    max_width - indent,
                    text_system,
                    config,
                    builder,
                    prefix,
                );
            }

            y - start_y + margin * 2.0
        }
        MarkdownBlock::UnorderedList(items) => {
            let indent = wgpui::theme::spacing::XL;
            let bullet_x = origin.x + wgpui::theme::spacing::SM;
            let margin = wgpui::theme::spacing::XS;
            let mut y = origin.y + margin;

            for item in items {
                let mut item_prefix = Some(LinePrefix {
                    text: "\u{2022}".to_string(),
                    x: bullet_x,
                    content_x: origin.x + indent,
                    font_size: config.base_font_size,
                });
                for block in item {
                    y += layout_markdown_block(
                        block,
                        Point::new(origin.x + indent, y),
                        max_width - indent,
                        text_system,
                        config,
                        builder,
                        &mut item_prefix,
                    );
                }
            }

            y - origin.y + margin
        }
        MarkdownBlock::OrderedList { start, items } => {
            let indent = wgpui::theme::spacing::XL * 2.0;
            let margin = wgpui::theme::spacing::XS;
            let mut y = origin.y + margin;

            for (idx, item) in items.iter().enumerate() {
                let number = start + idx as u64;
                let mut item_prefix = Some(LinePrefix {
                    text: format!("{}.", number),
                    x: origin.x,
                    content_x: origin.x + indent,
                    font_size: config.base_font_size,
                });
                for block in item {
                    y += layout_markdown_block(
                        block,
                        Point::new(origin.x + indent, y),
                        max_width - indent,
                        text_system,
                        config,
                        builder,
                        &mut item_prefix,
                    );
                }
            }

            y - origin.y + margin
        }
        MarkdownBlock::HorizontalRule => {
            let margin = wgpui::theme::spacing::LG;
            margin * 2.0 + 1.0
        }
        MarkdownBlock::Table { headers, rows } => {
            if headers.is_empty() {
                return 0.0;
            }

            let cell_padding = wgpui::theme::spacing::SM;
            let mut y = origin.y + cell_padding;
            let header_text = headers
                .iter()
                .map(|cell| line_text_from_styled(cell))
                .collect::<Vec<_>>()
                .join(" | ");
            builder.push_line(
                header_text,
                origin.x + cell_padding,
                y,
                32.0,
                config.base_font_size,
            );
            y += 32.0 + 1.0;

            for row in rows {
                let row_text = row
                    .iter()
                    .map(|cell| line_text_from_styled(cell))
                    .collect::<Vec<_>>()
                    .join(" | ");
                builder.push_line(
                    row_text,
                    origin.x + cell_padding,
                    y + cell_padding,
                    28.0,
                    config.base_font_size,
                );
                y += 28.0;
            }

            y - origin.y
        }
    }
}

fn layout_markdown_document(
    document: &MarkdownDocument,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    config: &MarkdownConfig,
    builder: &mut MessageLayoutBuilder,
) -> f32 {
    let mut y = origin.y;

    for (i, block) in document.blocks.iter().enumerate() {
        if i > 0 {
            y += wgpui::theme::spacing::MD;
            builder.push_gap();
        }
        let mut prefix = None;
        y += layout_markdown_block(
            block,
            Point::new(origin.x, y),
            max_width,
            text_system,
            config,
            builder,
            &mut prefix,
        );
    }

    y - origin.y
}

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

    pub(crate) fn chat_selection_point_at(
        &mut self,
        layout: &ChatLayout,
        x: f32,
        y: f32,
    ) -> Option<ChatSelectionPoint> {
        if y < layout.viewport_top || y > layout.viewport_bottom {
            return None;
        }
        let mut lines = layout
            .message_layouts
            .iter()
            .flat_map(|layout| layout.lines.iter());

        let first_line = lines.next()?;
        let mut closest = first_line;
        if y < first_line.y {
            return Some(ChatSelectionPoint {
                message_index: first_line.message_index,
                offset: first_line.display_range.start,
            });
        }

        if y >= first_line.y && y <= first_line.y + first_line.line_height {
            return Some(self.chat_point_for_line(first_line, x));
        }

        for line in lines {
            if y >= line.y && y <= line.y + line.line_height {
                return Some(self.chat_point_for_line(line, x));
            }
            closest = line;
        }

        if y > closest.y + closest.line_height {
            return Some(ChatSelectionPoint {
                message_index: closest.message_index,
                offset: closest.display_range.end,
            });
        }

        Some(self.chat_point_for_line(closest, x))
    }

    fn chat_point_for_line(&mut self, line: &ChatLineLayout, x: f32) -> ChatSelectionPoint {
        let char_width = self
            .text_system
            .measure_styled_mono("M", line.font_size, wgpui::text::FontStyle::default())
            .max(1.0);
        let char_count = line.text.chars().count();
        let rel_x = (x - line.x).max(0.0);
        let mut char_index = (rel_x / char_width).floor() as usize;
        if char_index > char_count {
            char_index = char_count;
        }
        let byte_offset = byte_offset_for_char_index(&line.text, char_index);
        ChatSelectionPoint {
            message_index: line.message_index,
            offset: line.display_range.start + byte_offset,
        }
    }

    pub(crate) fn chat_selection_contains(&self, point: ChatSelectionPoint) -> bool {
        let Some(selection) = self.chat.chat_selection else {
            return false;
        };
        let (start, end) = selection.normalized();
        crate::app::selection_point_cmp(&point, &start).is_ge()
            && crate::app::selection_point_cmp(&point, &end).is_le()
    }

    fn chat_selection_text(&self, layout: &ChatLayout) -> Option<String> {
        let selection = self.chat.chat_selection?;
        if selection.is_empty() {
            return None;
        }
        let (start, end) = selection.normalized();
        let mut out = String::new();
        for idx in start.message_index..=end.message_index {
            let Some(message) = layout.message_layouts.get(idx) else {
                continue;
            };
            let text = &message.display_text;
            let start_offset = if idx == start.message_index {
                start.offset.min(text.len())
            } else {
                0
            };
            let end_offset = if idx == end.message_index {
                end.offset.min(text.len())
            } else {
                text.len()
            };
            if start_offset <= end_offset {
                if let Some(slice) = text.get(start_offset..end_offset) {
                    out.push_str(slice);
                }
            }
            if idx != end.message_index {
                out.push('\n');
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    fn select_all_chat(&mut self, layout: &ChatLayout) {
        if layout.message_layouts.is_empty() {
            return;
        }
        let last_idx = layout.message_layouts.len() - 1;
        let end_offset = layout.message_layouts[last_idx].display_text.len();
        self.chat.chat_selection = Some(ChatSelection {
            anchor: ChatSelectionPoint {
                message_index: 0,
                offset: 0,
            },
            focus: ChatSelectionPoint {
                message_index: last_idx,
                offset: end_offset,
            },
        });
    }

    pub(crate) fn open_chat_context_menu(
        &mut self,
        position: Point,
        target_message: Option<usize>,
        copy_enabled: bool,
    ) {
        let mod_key = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        let copy_item = wgpui::MenuItem::new("copy", "Copy")
            .shortcut(format!("{}+C", mod_key))
            .disabled(!copy_enabled);
        let items = vec![
            copy_item,
            wgpui::MenuItem::separator(),
            wgpui::MenuItem::new("select_all", "Select All").shortcut(format!("{}+A", mod_key)),
        ];
        self.chat.chat_context_menu = wgpui::ContextMenu::new().items(items);
        self.chat.chat_context_menu_target = target_message;
        self.chat.chat_context_menu.open(position);
    }

    pub(crate) fn handle_chat_menu_action(&mut self, action: &str, layout: &ChatLayout) {
        match action {
            "copy" => {
                if let Some(text) = self.chat_selection_text(layout) {
                    self.write_chat_clipboard(&text);
                } else if let Some(target) = self.chat.chat_context_menu_target {
                    if let Some(message) = layout.message_layouts.get(target) {
                        self.write_chat_clipboard(&message.display_text);
                    }
                }
            }
            "select_all" => {
                self.select_all_chat(layout);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_chat_shortcut(
        &mut self,
        key: &winit::keyboard::Key<winit::keyboard::SmolStr>,
        modifiers: wgpui::input::Modifiers,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
    ) -> bool {
        if self.input.is_focused() {
            return false;
        }
        let ctrl_or_meta = modifiers.ctrl || modifiers.meta;
        if !ctrl_or_meta {
            return false;
        }
        match key {
            winit::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                if self
                    .chat
                    .chat_selection
                    .as_ref()
                    .is_some_and(|sel| !sel.is_empty())
                {
                    let chat_layout = self.build_chat_layout(sidebar_layout, logical_height);
                    if let Some(text) = self.chat_selection_text(&chat_layout) {
                        self.write_chat_clipboard(&text);
                        return true;
                    }
                }
            }
            winit::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("a") => {
                let chat_layout = self.build_chat_layout(sidebar_layout, logical_height);
                self.select_all_chat(&chat_layout);
                return true;
            }
            _ => {}
        }
        false
    }

    fn write_chat_clipboard(&mut self, text: &str) {
        // Always use system clipboard command (wl-copy on Wayland) for reliability
        let _ = copy_to_clipboard(text);
    }
}

fn coder_mode_display(mode: CoderMode) -> &'static str {
    match mode {
        CoderMode::BypassPermissions => "Bypass",
        CoderMode::Plan => "Plan",
        CoderMode::Autopilot => "Autopilot",
    }
}

fn coder_mode_color(mode: CoderMode, _palette: &UiPalette) -> Hsla {
    match mode {
        CoderMode::BypassPermissions => Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
        CoderMode::Plan => Hsla::new(200.0, 0.8, 0.5, 1.0), // Blue
        CoderMode::Autopilot => Hsla::new(280.0, 0.6, 0.5, 1.0), // Purple
    }
}

pub(crate) fn render_app(state: &mut AppState) {
    let scale_factor = state.window.scale_factor() as f32;
    let logical_width = state.config.width as f32 / scale_factor;
    let logical_height = state.config.height as f32 / scale_factor;

    let output = match state.surface.get_current_texture() {
        Ok(t) => t,
        Err(wgpu::SurfaceError::Lost) => {
            state.surface.configure(&state.device, &state.config);
            return;
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            tracing::error!("Out of memory");
            return;
        }
        Err(_) => return,
    };
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut scene = Scene::new();
    let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
    let palette = palette_for(state.settings.coder_settings.theme);
    let sidebar_layout = sidebar_layout(
        logical_width,
        logical_height,
        state.left_sidebar_open,
        state.right_sidebar_open,
    );

    // Dark terminal background
    scene.draw_quad(Quad::new(bounds).with_background(palette.background));

    render_sidebars(state, &mut scene, &palette, &sidebar_layout);
    render_chat(
        state,
        &mut scene,
        &palette,
        &sidebar_layout,
        logical_height,
        scale_factor,
    );
    render_input(
        state,
        &mut scene,
        &palette,
        &sidebar_layout,
        logical_width,
        logical_height,
        scale_factor,
    );
    render_modals(
        state,
        &mut scene,
        &palette,
        &sidebar_layout,
        logical_width,
        logical_height,
        scale_factor,
    );
    render_overlays(state, &mut scene, bounds, scale_factor, &palette);

    // Render
    let mut encoder = state
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Coder Render"),
        });

    let physical_width = state.config.width as f32;
    let physical_height = state.config.height as f32;

    state
        .renderer
        .resize(&state.queue, Size::new(physical_width, physical_height), 1.0);

    if state.text_system.is_dirty() {
        state.renderer.update_atlas(
            &state.queue,
            state.text_system.atlas_data(),
            state.text_system.atlas_size(),
        );
        state.text_system.mark_clean();
    }

    state
        .renderer
        .prepare(&state.device, &state.queue, &scene, scale_factor);
    state.renderer.render(&mut encoder, &view);

    state.queue.submit(std::iter::once(encoder.finish()));
    output.present();
}

fn render_sidebars(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
) {
    // Sidebar background color #0a0a0a
    let sidebar_bg = Hsla::new(0.0, 0.0, 0.039, 1.0);

    if let Some(left_bounds) = sidebar_layout.left {
        scene.draw_quad(
            Quad::new(left_bounds)
                .with_background(sidebar_bg)
                .with_border(palette.panel_border, 1.0),
        );

        // New Session button
        let btn_bounds = new_session_button_bounds(left_bounds);
        let btn_bg = if state.new_session_button_hovered {
            Hsla::new(0.0, 0.0, 0.15, 1.0)
        } else {
            Hsla::new(0.0, 0.0, 0.1, 1.0)
        };
        scene.draw_quad(
            Quad::new(btn_bounds)
                .with_background(btn_bg)
                .with_corner_radius(4.0),
        );
        let btn_text_y = btn_bounds.origin.y + (btn_bounds.size.height - 12.0) / 2.0;
        let btn_run = state.text_system.layout_styled_mono(
            "+ New Session",
            Point::new(btn_bounds.origin.x + 12.0, btn_text_y),
            12.0,
            palette.text_primary,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(btn_run);
    }

    if let Some(right_bounds) = sidebar_layout.right {
        scene.draw_quad(
            Quad::new(right_bounds)
                .with_background(sidebar_bg)
                .with_border(palette.panel_border, 1.0),
        );

        // Usage display
        let padding = 12.0;
        let mut y = right_bounds.origin.y + padding;
        let x = right_bounds.origin.x + padding;
        let w = right_bounds.size.width - padding * 2.0;

        let label_color = Hsla::new(0.0, 0.0, 0.5, 1.0);
        let value_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
        let muted_color = Hsla::new(0.0, 0.0, 0.4, 1.0);
        let font_size = 10.0;
        let line_height = 14.0;

        // Header
        let header = state.text_system.layout_styled_mono(
            "SESSION USAGE",
            Point::new(x, y),
            font_size,
            label_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(header);
        y += line_height + 8.0;

        // Model
        let model_text = &state.session.session_info.model;
        if !model_text.is_empty() {
            let model_run = state.text_system.layout_styled_mono(
                model_text,
                Point::new(x, y),
                11.0,
                value_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(model_run);
            y += line_height + 8.0;
        }

        // Cost and turns
        let cost_text = format!("${:.4}", state.session.session_usage.total_cost_usd);
        let cost_run = state.text_system.layout_styled_mono(
            &cost_text,
            Point::new(x, y),
            11.0,
            value_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(cost_run);

        let turns_text = format!("{} turns", state.session.session_usage.num_turns);
        let turns_run = state.text_system.layout_styled_mono(
            &turns_text,
            Point::new(x + 70.0, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(turns_run);
        y += line_height + 8.0;

        // Tokens
        let in_text = format!(
            "{} in",
            format_tokens(state.session.session_usage.input_tokens)
        );
        let in_run = state.text_system.layout_styled_mono(
            &in_text,
            Point::new(x, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(in_run);

        let out_text = format!(
            "{} out",
            format_tokens(state.session.session_usage.output_tokens)
        );
        let out_run = state.text_system.layout_styled_mono(
            &out_text,
            Point::new(x + w / 2.0, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(out_run);
        y += line_height + 4.0;

        // Duration
        let dur_text = format_duration_ms(state.session.session_usage.duration_ms);
        let dur_run = state.text_system.layout_styled_mono(
            &dur_text,
            Point::new(x, y),
            font_size,
            muted_color,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(dur_run);
        y += line_height + 16.0;

        // Rate limits section
        let green_color = Hsla::new(0.389, 0.7, 0.5, 1.0);

        // Render each rate limit
        let rate_limits_to_render: Vec<_> = [
            state.session.rate_limits.primary.clone(),
            state.session.rate_limits.secondary.clone(),
        ]
        .into_iter()
        .flatten()
        .collect();

        if !rate_limits_to_render.is_empty() {
            let header = state.text_system.layout_styled_mono(
                "RATE LIMITS",
                Point::new(x, y),
                font_size,
                label_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(header);
            y += line_height + 4.0;

            for limit in rate_limits_to_render {
                // Limit name and percentage
                let limit_text = format!("{} {:.0}%", limit.name, limit.percent_used);
                let limit_run = state.text_system.layout_styled_mono(
                    &limit_text,
                    Point::new(x, y),
                    font_size,
                    muted_color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(limit_run);
                y += line_height;

                // Progress bar
                let bar_h = 4.0;
                scene.draw_quad(
                    Quad::new(Bounds::new(x, y, w, bar_h))
                        .with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)),
                );
                let bar_color = if limit.percent_used < 50.0 {
                    green_color
                } else if limit.percent_used < 75.0 {
                    Hsla::new(0.125, 0.8, 0.5, 1.0) // yellow
                } else {
                    Hsla::new(0.0, 0.8, 0.5, 1.0) // red
                };
                let fill_w = (w * limit.percent_used as f32 / 100.0).min(w);
                scene.draw_quad(
                    Quad::new(Bounds::new(x, y, fill_w, bar_h)).with_background(bar_color),
                );
                y += bar_h + 2.0;

                // Reset time
                if !limit.resets_at.is_empty() {
                    let reset_text = format!("resets {}", limit.resets_at);
                    let reset_run = state.text_system.layout_styled_mono(
                        &reset_text,
                        Point::new(x, y),
                        9.0,
                        muted_color,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(reset_run);
                    y += line_height + 4.0;
                }
            }
        }
    }
}

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
                            &mut scene,
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
                &mut scene,
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

fn render_tools(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    chat_layout: &ChatLayout,
    viewport_top: f32,
    viewport_bottom: f32,
    scale_factor: f32,
) {
    let content_x = chat_layout.content_x;
    let available_width = chat_layout.available_width;

    // Render inline tools (scrolls with messages, no panel background)
    {
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        for inline_layout in &chat_layout.inline_tools {
            for block in &inline_layout.blocks {
                // Check if the tool block is visible in the viewport
                let block_top = block.card_bounds.origin.y;
                let block_bottom = block
                    .detail_bounds
                    .as_ref()
                    .map(|db| db.origin.y + db.size.height)
                    .unwrap_or(block.card_bounds.origin.y + block.card_bounds.size.height);

                if block_bottom > viewport_top && block_top < viewport_bottom {
                    if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                        tool.card.paint(block.card_bounds, &mut paint_cx);
                        if tool.status == ToolStatus::Running {
                            let ratio = tool
                                .elapsed_secs
                                .map(|elapsed| (elapsed / 6.0).min(1.0).max(0.1) as f32)
                                .unwrap_or(0.2_f32);
                            let bar_height = 2.0;
                            let bar_bounds = Bounds::new(
                                block.card_bounds.origin.x,
                                block.card_bounds.origin.y
                                    + block.card_bounds.size.height
                                    - bar_height,
                                block.card_bounds.size.width,
                                bar_height,
                            );
                            paint_cx.scene.draw_quad(
                                Quad::new(bar_bounds).with_background(palette.tool_progress_bg),
                            );
                            paint_cx.scene.draw_quad(
                                Quad::new(Bounds::new(
                                    bar_bounds.origin.x,
                                    bar_bounds.origin.y,
                                    bar_bounds.size.width * ratio,
                                    bar_bounds.size.height,
                                ))
                                .with_background(palette.tool_progress_fg),
                            );
                        }
                        if let Some(detail_bounds) = block.detail_bounds {
                            tool.detail.paint(detail_bounds, &mut paint_cx);
                        }
                    }
                }
            }
        }
    }

    // Render DSPy stage cards
    {
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        for dspy_layout in &chat_layout.dspy_stages {
            let stage_top = dspy_layout.y_offset;
            let stage_bottom = stage_top + dspy_layout.height;

            // Check if the stage is visible in the viewport
            if stage_bottom > viewport_top && stage_top < viewport_bottom {
                if let Some(stage_viz) = state.tools.dspy_stages.get(dspy_layout.stage_index) {
                    render_dspy_stage_card(
                        &stage_viz.stage,
                        Bounds::new(content_x, stage_top, available_width, dspy_layout.height),
                        &mut paint_cx,
                        palette,
                    );
                }
            }
        }
    }
}

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

    // Right side: model, available open models, tools, session
    if !state.session.session_info.model.is_empty() {
        // Format: "haiku | gptoss | 18 tools | abc123"
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
        parts.push(model_short);
        // Add available open models (not Claude SDK since that's already shown)
        for provider in &state.autopilot.available_providers {
            if !matches!(provider, adjutant::dspy::lm_config::LmProvider::ClaudeSdk) {
                parts.push(provider.short_name().to_string());
            }
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

fn render_modals(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    _sidebar_layout: &SidebarLayout,
    logical_width: f32,
    logical_height: f32,
    scale_factor: f32,
) {
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
    }
}

fn render_overlays(
    state: &mut AppState,
    scene: &mut Scene,
    bounds: Bounds,
    scale_factor: f32,
    palette: &UiPalette,
) {
    // Kitchen sink storybook overlay (covers full screen)
    if state.show_kitchen_sink {
        // Render on layer 1 to be on top of all layer 0 content
        scene.set_layer(1);

        paint_kitchen_sink(
            bounds,
            scene,
            &mut state.text_system,
            scale_factor,
            state.kitchen_sink_scroll,
            palette,
        );
    }

    if state.command_palette.is_open() {
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        state.command_palette.paint(bounds, &mut paint_cx);
    }

    if state.chat.chat_context_menu.is_open() {
        scene.set_layer(1);
        let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
        state.chat.chat_context_menu.paint(bounds, &mut paint_cx);
    }

    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
        if dialog.is_open() {
            let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
            dialog.paint(bounds, &mut paint_cx);
        }
    }
}

fn render_dspy_stage_card(
    stage: &DspyStage,
    bounds: Bounds,
    cx: &mut PaintContext,
    palette: &UiPalette,
) {
    let padding = 12.0;
    let font_size = 13.0;
    let small_font_size = 11.0;
    let line_height = font_size * 1.4;
    let small_line_height = small_font_size * 1.4;

    // Background with accent border based on stage type
    let (header_text, accent_color, icon) = match stage {
        DspyStage::EnvironmentAssessment { .. } => (
            "Environment Assessment",
            Hsla::new(200.0 / 360.0, 0.7, 0.5, 1.0), // Blue
            "🔍",
        ),
        DspyStage::Planning { .. } => (
            "Planning",
            Hsla::new(280.0 / 360.0, 0.6, 0.5, 1.0), // Purple
            "📋",
        ),
        DspyStage::TodoList { .. } => (
            "Todo List",
            Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0), // Green
            "✅",
        ),
        DspyStage::ExecutingTask { .. } => (
            "Executing",
            Hsla::new(30.0 / 360.0, 0.8, 0.5, 1.0), // Orange
            "🔧",
        ),
        DspyStage::TaskComplete { .. } => (
            "Task Complete",
            Hsla::new(160.0 / 360.0, 0.6, 0.5, 1.0), // Teal
            "✅",
        ),
        DspyStage::Complete { .. } => (
            "Complete",
            Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0), // Green
            "🏁",
        ),
    };

    // Card background
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(palette.panel_bg)
            .with_border(accent_color, 2.0)
            .with_corner_radius(8.0),
    );

    // Header
    let header_y = bounds.origin.y + padding;
    let icon_run = cx.text.layout_styled_mono(
        icon,
        Point::new(bounds.origin.x + padding, header_y),
        font_size,
        accent_color,
        wgpui::text::FontStyle::default(),
    );
    cx.scene.draw_text(icon_run);

    let header_run = cx.text.layout_styled_mono(
        header_text,
        Point::new(bounds.origin.x + padding + 20.0, header_y),
        font_size,
        palette.text_primary,
        wgpui::text::FontStyle::default(),
    );
    cx.scene.draw_text(header_run);

    // Content
    let content_x = bounds.origin.x + padding;
    let mut y = header_y + line_height + 8.0;

    match stage {
        DspyStage::EnvironmentAssessment {
            project_summary,
            repo_context,
            constraints,
            ..
        } => {
            let items = [
                ("Project", project_summary),
                ("Repo", repo_context),
                ("Constraints", constraints),
            ];
            for (label, text) in items {
                let label_run = cx.text.layout_styled_mono(
                    label,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.text_dim,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(label_run);
                y += small_line_height;
                for line in wrap_text(text, 80) {
                    let run = cx.text.layout_styled_mono(
                        &line,
                        Point::new(content_x, y),
                        small_font_size,
                        palette.text_primary,
                        wgpui::text::FontStyle::default(),
                    );
                    cx.scene.draw_text(run);
                    y += small_line_height;
                }
                y += 4.0;
            }
        }
        DspyStage::Planning {
            plan_summary,
            implementation_steps,
            ..
        } => {
            let run = cx.text.layout_styled_mono(
                plan_summary,
                Point::new(content_x, y),
                small_font_size,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
            y += small_line_height + 8.0;
            for (i, step) in implementation_steps.iter().enumerate() {
                let line = format!("{}. {}", i + 1, step);
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.text_dim,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }
        }
        DspyStage::TodoList { tasks } => {
            for task in tasks {
                let status_symbol = match task.status {
                    crate::autopilot_loop::TodoStatus::Pending => "□",
                    crate::autopilot_loop::TodoStatus::InProgress => "◐",
                    crate::autopilot_loop::TodoStatus::Complete => "✓",
                    crate::autopilot_loop::TodoStatus::Failed => "✗",
                };
                let color = match task.status {
                    crate::autopilot_loop::TodoStatus::Pending => palette.text_dim,
                    crate::autopilot_loop::TodoStatus::InProgress => accent_color,
                    crate::autopilot_loop::TodoStatus::Complete => Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0),
                    crate::autopilot_loop::TodoStatus::Failed => Hsla::new(0.0, 0.6, 0.5, 1.0),
                };
                let line = format!("{} {}", status_symbol, task.title);
                let run = cx.text.layout_styled_mono(
                    &line,
                    Point::new(content_x, y),
                    small_font_size,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;
            }
        }
        DspyStage::ExecutingTask { task_title, .. } => {
            let status = format!("Working on: {}", task_title);
            let run = cx.text.layout_styled_mono(
                &status,
                Point::new(content_x, y),
                font_size,
                accent_color,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
        }
        DspyStage::TaskComplete { task_title, success, .. } => {
            let status = if *success {
                format!("Completed: {}", task_title)
            } else {
                format!("Failed: {}", task_title)
            };
            let color = if *success {
                Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0)
            } else {
                Hsla::new(0.0, 0.6, 0.5, 1.0)
            };
            let run = cx.text.layout_styled_mono(
                &status,
                Point::new(content_x, y),
                font_size,
                color,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
        }
        DspyStage::Complete {
            total_tasks,
            successful,
            failed,
        } => {
            let summary = format!(
                "Completed {} tasks: {} successful, {} failed",
                total_tasks, successful, failed
            );
            let color = if *failed == 0 {
                Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0)
            } else {
                Hsla::new(30.0 / 360.0, 0.7, 0.5, 1.0)
            };
            let run = cx.text.layout_styled_mono(
                &summary,
                Point::new(content_x, y),
                font_size,
                color,
                wgpui::text::FontStyle::default(),
            );
            cx.scene.draw_text(run);
        }
    }
}

fn paint_kitchen_sink(
    bounds: Bounds,
    scene: &mut Scene,
    text_system: &mut TextSystem,
    scale_factor: f32,
    scroll_offset: f32,
    palette: &UiPalette,
) {
    // Opaque background to cover content behind
    let overlay = Quad::new(bounds).with_background(Hsla::new(220.0, 0.15, 0.08, 1.0));
    scene.draw_quad(overlay);

    // Content area with padding
    let padding = 24.0;
    let content_x = bounds.origin.x + padding;
    let content_width = bounds.size.width - padding * 2.0;
    let card_width = (content_width - 16.0) / 2.0; // Two columns

    let mut y = bounds.origin.y + padding - scroll_offset;
    let font_style = wgpui::text::FontStyle::default();

    // Title
    let title_run = text_system.layout_styled_mono(
        "Kitchen Sink - Component Storybook",
        Point::new(content_x, y),
        18.0,
        Hsla::new(0.0, 0.0, 0.95, 1.0),
        font_style,
    );
    scene.draw_text(title_run);
    y += 28.0;

    let subtitle_run = text_system.layout_styled_mono(
        "Press Escape to close | Scroll to see more",
        Point::new(content_x, y),
        12.0,
        Hsla::new(0.0, 0.0, 0.5, 1.0),
        font_style,
    );
    scene.draw_text(subtitle_run);
    y += 32.0;

    // Section: Tool Types
    let section_run = text_system.layout_styled_mono(
        "TOOL TYPES (with Success status)",
        Point::new(content_x, y),
        14.0,
        Hsla::new(42.0 / 360.0, 0.8, 0.6, 1.0), // Yellow/gold
        font_style,
    );
    scene.draw_text(section_run);
    y += 24.0;

    let tool_types = [
        (ToolType::Read, "Read", "src/main.rs"),
        (ToolType::Write, "Write", "output.txt"),
        (ToolType::Edit, "Edit", "config.toml"),
        (ToolType::Bash, "Bash", "cargo build"),
        (ToolType::Glob, "Glob", "**/*.rs"),
        (ToolType::Grep, "Grep", "fn main"),
        (ToolType::Search, "Search", "error handling"),
        (ToolType::List, "List", "/home/user"),
        (ToolType::Task, "Task", "Analyze codebase"),
        (ToolType::WebFetch, "WebFetch", "https://example.com"),
    ];

    let mut paint_cx = PaintContext::new(scene, text_system, scale_factor);
    let mut col = 0;
    let mut row_y = y;

    for (tool_type, name, input) in &tool_types {
        let x = content_x + (col as f32 * (card_width + 16.0));
        let card_bounds = Bounds::new(x, row_y, card_width, 28.0);

        let mut card = ToolCallCard::new(*tool_type, *name)
            .status(ToolStatus::Success)
            .input(*input)
            .elapsed_secs(0.42);
        card.paint(card_bounds, &mut paint_cx);

        col += 1;
        if col >= 2 {
            col = 0;
            row_y += 36.0;
        }
    }

    if col != 0 {
        row_y += 36.0;
    }
    y = row_y + 16.0;

    // Section: Tool Statuses
    let section_run = paint_cx.text.layout_styled_mono(
        "TOOL STATUSES (Read tool)",
        Point::new(content_x, y),
        14.0,
        Hsla::new(200.0 / 360.0, 0.8, 0.6, 1.0), // Blue
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let statuses = [
        (ToolStatus::Pending, "Pending"),
        (ToolStatus::Running, "Running"),
        (ToolStatus::Success, "Success"),
        (ToolStatus::Error, "Error"),
        (ToolStatus::Cancelled, "Cancelled"),
    ];

    col = 0;
    row_y = y;

    for (status, label) in &statuses {
        let x = content_x + (col as f32 * (card_width + 16.0));
        let card_bounds = Bounds::new(x, row_y, card_width, 28.0);

        let elapsed = if matches!(status, ToolStatus::Success | ToolStatus::Error) {
            Some(1.23)
        } else {
            None
        };

        let mut card = ToolCallCard::new(ToolType::Read, format!("Read ({})", label))
            .status(*status)
            .input("example.rs");
        if let Some(e) = elapsed {
            card = card.elapsed_secs(e);
        }
        card.paint(card_bounds, &mut paint_cx);

        col += 1;
        if col >= 2 {
            col = 0;
            row_y += 36.0;
        }
    }

    if col != 0 {
        row_y += 36.0;
    }
    y = row_y + 16.0;

    // Section: Task with Children
    let section_run = paint_cx.text.layout_styled_mono(
        "TASK WITH CHILD TOOLS",
        Point::new(content_x, y),
        14.0,
        Hsla::new(320.0 / 360.0, 0.8, 0.6, 1.0), // Magenta
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let task_bounds = Bounds::new(content_x, y, card_width, 72.0);
    let mut task_card = ToolCallCard::new(ToolType::Task, "Task")
        .status(ToolStatus::Running)
        .input("Build dependency graph")
        .elapsed_secs(12.3);

    let child_tools = vec![
        ChildTool::new(ToolType::Read, "Read", "Cargo.toml", ToolStatus::Success),
        ChildTool::new(ToolType::Grep, "Grep", "mod.rs", ToolStatus::Success),
        ChildTool::new(ToolType::Search, "Search", "dependency", ToolStatus::Running),
    ];
    task_card = task_card.children(child_tools);
    task_card.paint(task_bounds, &mut paint_cx);

    y += 92.0;

    // Section: Diff Preview
    let section_run = paint_cx.text.layout_styled_mono(
        "DIFF PREVIEW",
        Point::new(content_x, y),
        14.0,
        Hsla::new(120.0 / 360.0, 0.8, 0.6, 1.0), // Green
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let diff_bounds = Bounds::new(content_x, y, content_width, 160.0);
    let diff_lines = vec![
        DiffLine::new(DiffLineKind::Context, " fn main() {"),
        DiffLine::new(DiffLineKind::Remove, "-    println!(\"Hello\");"),
        DiffLine::new(DiffLineKind::Add, "+    println!(\"Hello, world!\");"),
        DiffLine::new(DiffLineKind::Context, " }"),
    ];
    let mut diff = DiffToolCall::new(diff_lines);
    diff.paint(diff_bounds, &mut paint_cx);

    y += 180.0;

    // Section: Search Results
    let section_run = paint_cx.text.layout_styled_mono(
        "SEARCH RESULTS",
        Point::new(content_x, y),
        14.0,
        Hsla::new(40.0 / 360.0, 0.8, 0.6, 1.0), // Orange
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let matches = vec![
        SearchMatch::new("src/main.rs", 42, "fn handle_error() {"),
        SearchMatch::new("src/lib.rs", 101, "pub enum ErrorKind {"),
        SearchMatch::new("src/utils.rs", 7, "error handling utilities"),
    ];
    let search_bounds = Bounds::new(content_x, y, content_width, 140.0);
    let mut search = SearchToolCall::new("error".to_string(), matches);
    search.paint(search_bounds, &mut paint_cx);

    y += 160.0;

    // Section: Terminal Output
    let section_run = paint_cx.text.layout_styled_mono(
        "TERMINAL OUTPUT",
        Point::new(content_x, y),
        14.0,
        Hsla::new(200.0 / 360.0, 0.8, 0.6, 1.0), // Blue
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let output = "Compiling...\nFinished dev [unoptimized + debuginfo] target(s) in 2.13s";
    let terminal_bounds = Bounds::new(content_x, y, content_width, 90.0);
    let mut terminal = TerminalToolCall::new(output.to_string());
    terminal.paint(terminal_bounds, &mut paint_cx);

    y += 110.0;

    // Section: Event Inspector
    let section_run = paint_cx.text.layout_styled_mono(
        "EVENT INSPECTOR",
        Point::new(content_x, y),
        14.0,
        Hsla::new(280.0 / 360.0, 0.8, 0.6, 1.0), // Purple
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let event = EventData::new("event-1", "hooks", 61001)
        .content("Example hook event".to_string())
        .created_at(0)
        .tags(vec![TagData::new("tool", vec!["Read".to_string()])])
        .sig("")
        .verified(false);
    let mut inspector = EventInspector::new(event);
    inspector = inspector.view(InspectorView::Summary);
    let inspector_bounds = Bounds::new(content_x, y, content_width, 200.0);
    inspector.paint(inspector_bounds, &mut paint_cx);

    y += 220.0;

    // Section: Permission Dialog
    let section_run = paint_cx.text.layout_styled_mono(
        "PERMISSION DIALOG",
        Point::new(content_x, y),
        14.0,
        Hsla::new(0.0, 0.8, 0.6, 1.0), // Red
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let mut dialog = PermissionDialog::new();
    dialog.open();
    dialog.set_title("Allow tool?");
    dialog.set_message("Tool \"Bash\" wants to run: rm -rf /");
    let dialog_bounds = Bounds::new(content_x, y, 400.0, 140.0);
    dialog.paint(dialog_bounds, &mut paint_cx);

    y += 160.0;

    // Footer
    let footer_run = paint_cx.text.layout_styled_mono(
        "End of Kitchen Sink",
        Point::new(content_x, y),
        12.0,
        Hsla::new(0.0, 0.0, 0.4, 1.0),
        font_style,
    );
    paint_cx.scene.draw_text(footer_run);
}

pub(crate) fn session_list_layout(
    logical_width: f32,
    logical_height: f32,
    session_count: usize,
    selected: usize,
    checkpoint_height: f32,
) -> SessionListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

    let content_top = modal_y + SESSION_MODAL_PADDING + 46.0;
    let footer_y = modal_y + modal_height - 24.0;
    let checkpoint_height = checkpoint_height.max(0.0);
    let checkpoint_bounds = if checkpoint_height > 0.0 {
        let y = footer_y - 12.0 - checkpoint_height;
        Some(Bounds::new(
            modal_x + SESSION_MODAL_PADDING,
            y,
            modal_width - SESSION_MODAL_PADDING * 2.0,
            checkpoint_height,
        ))
    } else {
        None
    };

    let card_area_bottom = checkpoint_bounds
        .as_ref()
        .map(|bounds| bounds.origin.y - 16.0)
        .unwrap_or(footer_y - 16.0);
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SESSION_CARD_HEIGHT + SESSION_CARD_GAP)) as usize
    };

    let visible_count = session_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(session_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > session_count {
            start = session_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SESSION_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SESSION_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    SessionListLayout {
        modal_bounds,
        card_bounds,
        checkpoint_bounds,
    }
}

pub(crate) fn agent_modal_content_top(modal_y: f32, state: &AppState) -> f32 {
    let mut y = modal_y + 16.0;
    y += 20.0;
    y += 18.0;
    if state.catalogs.active_agent.is_some() {
        y += 18.0;
    }
    y += 18.0;
    if state.catalogs.agent_user_path.is_some() {
        y += 18.0;
    }
    if state.catalogs.agent_load_error.is_some() {
        y += 18.0;
    }
    y + 20.0
}

pub(crate) fn skill_modal_content_top(modal_y: f32, state: &AppState) -> f32 {
    let mut y = modal_y + 16.0;
    y += 20.0;
    y += 18.0;
    y += 18.0;
    if state.catalogs.skill_user_path.is_some() {
        y += 18.0;
    }
    if state.catalogs.skill_load_error.is_some() {
        y += 18.0;
    }
    y + 20.0
}

pub(crate) fn agent_list_layout(
    logical_width: f32,
    logical_height: f32,
    agent_count: usize,
    selected: usize,
    content_top: f32,
) -> AgentListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let footer_y = modal_y + modal_height - 24.0;
    let card_area_bottom = footer_y - 16.0;
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SESSION_CARD_HEIGHT + SESSION_CARD_GAP))
            as usize
    };

    let visible_count = agent_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(agent_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > agent_count {
            start = agent_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SESSION_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SESSION_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    AgentListLayout { card_bounds }
}

pub(crate) fn skill_list_layout(
    logical_width: f32,
    logical_height: f32,
    skill_count: usize,
    selected: usize,
    content_top: f32,
) -> SkillListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let footer_y = modal_y + modal_height - 24.0;
    let card_area_bottom = footer_y - 16.0;
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SKILL_CARD_HEIGHT + SESSION_CARD_GAP)) as usize
    };

    let visible_count = skill_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(skill_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > skill_count {
            start = skill_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SKILL_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SKILL_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    SkillListLayout { card_bounds }
}

pub(crate) fn hook_event_layout(
    logical_width: f32,
    logical_height: f32,
    event_count: usize,
    selected: usize,
) -> HookEventLayout {
    let modal_width = HOOK_MODAL_WIDTH;
    let modal_height = HOOK_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);

    let content_top = modal_y + 64.0;
    let content_bottom = modal_y + modal_height - 32.0;
    let content_height = (content_bottom - content_top).max(0.0);
    let list_width = 260.0;
    let list_bounds = Bounds::new(modal_x + 16.0, content_top, list_width, content_height);
    let inspector_bounds = Bounds::new(
        list_bounds.origin.x + list_width + 16.0,
        content_top,
        modal_width - list_width - 48.0,
        content_height,
    );

    let max_rows = (content_height / HOOK_EVENT_ROW_HEIGHT).floor().max(0.0) as usize;
    let visible_count = event_count.min(max_rows.max(1));
    let mut row_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(event_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > event_count {
            start = event_count.saturating_sub(visible_count);
        }
        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * HOOK_EVENT_ROW_HEIGHT;
            let bounds = Bounds::new(list_bounds.origin.x, y, list_bounds.size.width, HOOK_EVENT_ROW_HEIGHT);
            row_bounds.push((index, bounds));
        }
    }

    HookEventLayout {
        list_bounds,
        inspector_bounds,
        row_bounds,
    }
}
