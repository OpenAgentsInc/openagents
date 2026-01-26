use tokio::sync::mpsc;
use wgpui::Bounds;
use wgpui::ContextMenu;
use wgpui::components::molecules::SectionStatus;
use wgpui::markdown::{MarkdownRenderer as MdRenderer, StreamingMarkdown};

use super::{ChatMessage, ChatSelection, MessageRole};
use crate::app::config::CoderSettings;
use crate::app::events::{QueryControl, ResponseEvent};
use crate::app::session::SessionState;
use crate::app::truncate_preview;
use crate::app::ui::ThemeSetting;
use crate::autopilot_loop::IssueSuggestionDisplay;

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
    /// Initialize section (Hardware, Compute, Network, Identity, Workspace, Summary)
    pub(crate) initialize: BootSection,
    /// Issue suggestion section
    pub(crate) suggest_issues: BootSection,
    /// Bounds of the boot card (for click detection)
    pub(crate) card_bounds: Option<Bounds>,
    /// Streaming text for issue analysis (rendered in suggest_issues card)
    pub(crate) streaming_text: String,
    /// Scroll offset for streaming text viewport
    pub(crate) streaming_scroll_offset: f32,
}

impl BootSections {
    pub(crate) fn new() -> Self {
        tracing::info!("Creating BootSections for chat display");
        Self {
            initialize: BootSection::new(1, "..."),
            suggest_issues: {
                let mut section = BootSection::new(2, "");
                section.active = true;
                section.expanded = true;
                section
            },
            card_bounds: None,
            streaming_text: String::new(),
            streaming_scroll_offset: 0.0,
        }
    }
}

/// State for inline issue selector displayed in chat.
///
/// This replaces the full-screen bootloader modal with an inline
/// card that supports both clickable buttons and keyboard hotkeys.
#[derive(Clone)]
pub(crate) struct InlineIssueSelector {
    /// Issue suggestions to display
    pub(crate) suggestions: Vec<IssueSuggestionDisplay>,
    /// Number of issues filtered out (stale/blocked)
    pub(crate) filtered_count: usize,
    /// Confidence score from the LLM
    pub(crate) confidence: f32,
    /// Whether awaiting user selection
    pub(crate) await_selection: bool,
    /// Index of currently hovered suggestion (for hover highlighting)
    pub(crate) hovered_index: Option<usize>,
    /// Computed bounds for each suggestion button (for click detection)
    pub(crate) suggestion_bounds: Vec<Bounds>,
    /// Computed bounds for the skip button
    pub(crate) skip_button_bounds: Option<Bounds>,
}

impl InlineIssueSelector {
    pub(crate) fn new(
        suggestions: Vec<IssueSuggestionDisplay>,
        filtered_count: usize,
        confidence: f32,
        await_selection: bool,
    ) -> Self {
        Self {
            suggestions,
            filtered_count,
            confidence,
            await_selection,
            hovered_index: None,
            suggestion_bounds: Vec::new(),
            skip_button_bounds: None,
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
    /// Inline issue selector (replaces full-screen bootloader modal)
    pub(crate) inline_issue_selector: Option<InlineIssueSelector>,
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
            inline_issue_selector: None,
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
