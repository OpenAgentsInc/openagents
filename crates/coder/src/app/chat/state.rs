use tokio::sync::mpsc;
use wgpui::ContextMenu;
use wgpui::markdown::{MarkdownRenderer as MdRenderer, StreamingMarkdown};

use super::{ChatMessage, ChatSelection};
use crate::app::config::CoderSettings;
use crate::app::events::{QueryControl, ResponseEvent};

pub(crate) struct ChatState {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) streaming_markdown: StreamingMarkdown,
    pub(crate) markdown_renderer: MdRenderer,
    pub(crate) is_thinking: bool,
    pub(crate) chat_selection: Option<ChatSelection>,
    pub(crate) chat_selection_dragging: bool,
    pub(crate) chat_context_menu: ContextMenu,
    pub(crate) chat_context_menu_target: Option<usize>,
    pub(crate) response_rx: Option<mpsc::UnboundedReceiver<ResponseEvent>>,
    pub(crate) query_control_tx: Option<mpsc::UnboundedSender<QueryControl>>,
    pub(crate) scroll_offset: f32,
}

impl ChatState {
    pub(crate) fn new(settings: &CoderSettings) -> Self {
        let mut streaming_markdown = StreamingMarkdown::new();
        streaming_markdown.set_markdown_config(super::super::build_markdown_config(settings));
        let markdown_renderer = super::super::build_markdown_renderer(settings);
        Self {
            messages: Vec::new(),
            streaming_markdown,
            markdown_renderer,
            is_thinking: false,
            chat_selection: None,
            chat_selection_dragging: false,
            chat_context_menu: ContextMenu::new(),
            chat_context_menu_target: None,
            response_rx: None,
            query_control_tx: None,
            scroll_offset: 0.0,
        }
    }
}
