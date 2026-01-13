use tokio::sync::mpsc;
use wgpui::ContextMenu;
use wgpui::components::molecules::SectionStatus;
use wgpui::markdown::{MarkdownRenderer as MdRenderer, StreamingMarkdown};

use super::{ChatMessage, ChatSelection, MessageRole};
use crate::app::config::CoderSettings;
use crate::app::events::{QueryControl, ResponseEvent};
use crate::app::session::SessionState;
use crate::app::truncate_preview;
use crate::app::ui::ThemeSetting;

/// State for a collapsible boot section displayed in chat
pub(crate) struct BootSection {
    /// Unique identifier for this section
    pub(crate) id: usize,
    /// Summary line shown in header
    pub(crate) summary: String,
    /// Detail lines shown when expanded
    pub(crate) details: Vec<String>,
    /// Current status (Pending, InProgress, Success, Error)
    pub(crate) status: SectionStatus,
    /// Whether section is expanded
    pub(crate) expanded: bool,
    /// Whether section is still receiving updates
    pub(crate) active: bool,
}

impl BootSection {
    fn new(id: usize, summary: &str) -> Self {
        Self {
            id,
            summary: summary.to_string(),
            details: Vec::new(),
            status: SectionStatus::Pending,
            expanded: true,
            active: true,
        }
    }
}

/// Boot sections displayed at top of chat during startup
pub(crate) struct BootSections {
    /// Environment check section (Hardware, Compute, Network, Identity, Workspace, Summary)
    pub(crate) environment: BootSection,
    /// Issue verification section
    pub(crate) issues: BootSection,
}

impl BootSections {
    pub(crate) fn new() -> Self {
        Self {
            environment: BootSection::new(1, "Checking environment..."),
            issues: {
                let mut section = BootSection::new(2, "Evaluating blocked issues...");
                section.active = false; // Will activate after environment completes
                section
            },
        }
    }
}

pub(crate) struct ChatState {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) streaming_markdown: StreamingMarkdown,
    pub(crate) streaming_thought: StreamingMarkdown,
    pub(crate) markdown_renderer: MdRenderer,
    pub(crate) is_thinking: bool,
    pub(crate) chat_selection: Option<ChatSelection>,
    pub(crate) chat_selection_dragging: bool,
    pub(crate) chat_context_menu: ContextMenu,
    pub(crate) chat_context_menu_target: Option<usize>,
    pub(crate) response_rx: Option<mpsc::UnboundedReceiver<ResponseEvent>>,
    pub(crate) query_control_tx: Option<mpsc::UnboundedSender<QueryControl>>,
    pub(crate) scroll_offset: f32,
    /// Boot sections displayed at top of chat during startup
    pub(crate) boot_sections: Option<BootSections>,
}

impl ChatState {
    pub(crate) fn new(settings: &CoderSettings, theme: ThemeSetting) -> Self {
        let mut streaming_markdown = StreamingMarkdown::new();
        streaming_markdown
            .set_markdown_config(super::super::build_markdown_config(settings, theme));
        let mut streaming_thought = StreamingMarkdown::new();
        streaming_thought.set_markdown_config(super::super::build_markdown_config(settings, theme));
        let markdown_renderer = super::super::build_markdown_renderer(settings, theme);
        Self {
            messages: Vec::new(),
            streaming_markdown,
            streaming_thought,
            markdown_renderer,
            is_thinking: false,
            chat_selection: None,
            chat_selection_dragging: false,
            chat_context_menu: ContextMenu::new(),
            chat_context_menu_target: None,
            response_rx: None,
            query_control_tx: None,
            scroll_offset: 0.0,
            boot_sections: Some(BootSections::new()),
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
