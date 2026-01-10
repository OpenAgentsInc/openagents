use tokio::sync::mpsc;
use wgpui::ContextMenu;
use wgpui::markdown::{MarkdownRenderer as MdRenderer, StreamingMarkdown};

use super::{ChatMessage, ChatSelection, MessageRole};
use crate::app::config::CoderSettings;
use crate::app::events::{QueryControl, ResponseEvent};
use crate::app::session::SessionState;
use crate::app::truncate_preview;

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

    pub(crate) fn push_system_message(&mut self, message: String) {
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: message,
            document: None,
            uuid: None,
            metadata: None,
        });
    }

    pub(crate) fn attach_user_message_id(&mut self, uuid: String, session: &mut SessionState) {
        if let Some(message) = self
            .messages
            .iter_mut()
            .rev()
            .find(|msg| matches!(msg.role, MessageRole::User) && msg.uuid.is_none())
        {
            message.uuid = Some(uuid);
            session.refresh_checkpoint_restore(&self.messages);
        }
    }

    pub(crate) fn request_rewind_files(&mut self, user_message_id: String) {
        if let Some(tx) = &self.query_control_tx {
            let _ = tx.send(QueryControl::RewindFiles {
                user_message_id: user_message_id.clone(),
            });
            self.push_system_message(format!(
                "Requested checkpoint restore for message {}.",
                truncate_preview(&user_message_id, 12)
            ));
        } else {
            self.push_system_message("No active request to rewind.".to_string());
        }
    }

    pub(crate) fn interrupt_query(&mut self) {
        if let Some(tx) = &self.query_control_tx {
            let _ = tx.send(QueryControl::Interrupt);
        } else {
            self.push_system_message("No active request to interrupt.".to_string());
        }
    }

    #[allow(dead_code)]
    pub(crate) fn abort_query(&mut self) {
        if let Some(tx) = &self.query_control_tx {
            let _ = tx.send(QueryControl::Abort);
        } else {
            self.push_system_message("No active request to cancel.".to_string());
        }
    }
}
