use super::super::tools::{DspyStageLayout, ToolPanelBlock};

#[derive(Clone, Debug)]
pub(crate) struct ChatLineLayout {
    pub(crate) message_index: usize,
    pub(crate) text: String,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) line_height: f32,
    pub(crate) font_size: f32,
    pub(crate) display_range: std::ops::Range<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct MessageLayout {
    pub(crate) height: f32,
    pub(crate) display_text: String,
    pub(crate) lines: Vec<ChatLineLayout>,
}

/// Layout for tools shown inline after a specific message.
pub(crate) struct InlineToolsLayout {
    pub(crate) message_index: usize,
    /// Y position in content coordinates (before scroll adjustment).
    pub(crate) y_offset: f32,
    pub(crate) height: f32,
    pub(crate) blocks: Vec<ToolPanelBlock>,
}

/// Layout for DSPy stage cards displayed inline in chat.
pub(crate) struct ChatLayout {
    pub(crate) viewport_top: f32,
    pub(crate) viewport_bottom: f32,
    pub(crate) content_x: f32,
    pub(crate) available_width: f32,
    pub(crate) chat_font_size: f32,
    pub(crate) chat_line_height: f32,
    pub(crate) message_layouts: Vec<MessageLayout>,
    pub(crate) streaming_height: f32,
    /// Inline tool layouts positioned after their associated messages.
    pub(crate) inline_tools: Vec<InlineToolsLayout>,
    /// DSPy stage layouts positioned inline in chat.
    pub(crate) dspy_stages: Vec<DspyStageLayout>,
}

pub(crate) struct MessageLayoutBuilder {
    pub(crate) message_index: usize,
    pub(crate) display_text: String,
    pub(crate) lines: Vec<ChatLineLayout>,
}

impl MessageLayoutBuilder {
    pub(crate) fn new(message_index: usize) -> Self {
        Self {
            message_index,
            display_text: String::new(),
            lines: Vec::new(),
        }
    }

    pub(crate) fn push_line(
        &mut self,
        text: String,
        x: f32,
        y: f32,
        line_height: f32,
        font_size: f32,
    ) {
        if !self.display_text.is_empty() {
            self.display_text.push('\n');
        }
        let start = self.display_text.len();
        self.display_text.push_str(&text);
        let end = self.display_text.len();
        self.lines.push(ChatLineLayout {
            message_index: self.message_index,
            text,
            x,
            y,
            line_height,
            font_size,
            display_range: start..end,
        });
    }

    pub(crate) fn push_gap(&mut self) {
        if !self.display_text.is_empty() {
            self.display_text.push('\n');
        }
    }

    pub(crate) fn build(self, height: f32) -> MessageLayout {
        MessageLayout {
            height,
            display_text: self.display_text,
            lines: self.lines,
        }
    }
}
