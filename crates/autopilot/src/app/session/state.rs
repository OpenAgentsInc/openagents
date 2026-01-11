use std::fs;
use std::io;

use tokio::sync::mpsc;
use wgpui::components::atoms::SessionStatus;
use wgpui::components::molecules::{
    CheckpointRestore, SessionAction, SessionCard, SessionInfo as SessionCardInfo,
};

use super::{CheckpointEntry, RateLimits, SessionEntry, SessionInfo, SessionUsageStats};
use crate::app::chat::{ChatMessage, ChatState, MessageRole};
use crate::app::config::session_messages_dir;
use crate::app::events::ModalState;
use crate::app::tools::ToolsState;
use crate::app::{ModelOption, SessionCardEvent};

pub(crate) struct SessionState {
    pub(crate) session_info: SessionInfo,
    pub(crate) session_usage: SessionUsageStats,
    pub(crate) rate_limits: RateLimits,
    pub(crate) rate_limit_rx: Option<mpsc::UnboundedReceiver<RateLimits>>,
    pub(crate) session_index: Vec<SessionEntry>,
    pub(crate) pending_resume_session: Option<String>,
    pub(crate) pending_fork_session: bool,
    pub(crate) session_cards: Vec<SessionCard>,
    pub(crate) session_action_tx: Option<mpsc::UnboundedSender<SessionCardEvent>>,
    pub(crate) session_action_rx: Option<mpsc::UnboundedReceiver<SessionCardEvent>>,
    pub(crate) checkpoint_restore: CheckpointRestore,
    pub(crate) checkpoint_entries: Vec<CheckpointEntry>,
    pub(crate) checkpoint_action_tx: Option<mpsc::UnboundedSender<usize>>,
    pub(crate) checkpoint_action_rx: Option<mpsc::UnboundedReceiver<usize>>,
}

impl SessionState {
    pub(crate) fn new(
        selected_model: ModelOption,
        coder_mode_label: String,
        session_index: Vec<SessionEntry>,
        rate_limit_rx: Option<mpsc::UnboundedReceiver<RateLimits>>,
    ) -> Self {
        Self {
            session_info: SessionInfo {
                model: selected_model.model_id().to_string(),
                permission_mode: coder_mode_label,
                ..Default::default()
            },
            session_usage: SessionUsageStats::default(),
            rate_limits: RateLimits::default(),
            rate_limit_rx,
            session_index,
            pending_resume_session: None,
            pending_fork_session: false,
            session_cards: Vec::new(),
            session_action_tx: None,
            session_action_rx: None,
            checkpoint_restore: CheckpointRestore::new(),
            checkpoint_entries: Vec::new(),
            checkpoint_action_tx: None,
            checkpoint_action_rx: None,
        }
    }

    pub(crate) fn refresh_session_cards(&mut self, is_thinking: bool) {
        let action_tx = self.session_action_tx.clone();
        self.session_cards = self
            .session_index
            .iter()
            .map(|entry| {
                let is_active = entry.id == self.session_info.session_id;
                let status = if is_active {
                    if is_thinking {
                        SessionStatus::Running
                    } else {
                        SessionStatus::Paused
                    }
                } else {
                    SessionStatus::Completed
                };
                let title = if entry.last_message.trim().is_empty() {
                    format!(
                        "Session {}",
                        super::super::truncate_preview(&entry.id, 8)
                    )
                } else {
                    super::super::truncate_preview(&entry.last_message, 64)
                };
                let duration = entry.updated_at.saturating_sub(entry.created_at);
                let timestamp = super::super::format_relative_time(entry.updated_at);
                let model = entry.model.replace("-2025", "");
                let info = SessionCardInfo::new(entry.id.clone(), title)
                    .status(status)
                    .duration(duration)
                    .task_count(entry.message_count as u32)
                    .timestamp(timestamp)
                    .model(model);
                let mut card = SessionCard::new(info).show_actions(true);
                if let Some(tx) = action_tx.clone() {
                    card = card.on_action(move |action, session_id| {
                        let _ = tx.send(SessionCardEvent { action, session_id });
                    });
                }
                card
            })
            .collect();
    }

    pub(crate) fn refresh_checkpoint_restore(&mut self, messages: &[ChatMessage]) {
        let entries = super::super::build_checkpoint_entries(messages);
        let labels = entries.iter().map(|entry| entry.label.clone()).collect();
        let action_tx = self.checkpoint_action_tx.clone();
        let mut restore = CheckpointRestore::new().checkpoints(labels);
        if let Some(tx) = action_tx {
            let tx = tx.clone();
            restore = restore.on_restore(move |index, _label| {
                let _ = tx.send(index);
            });
        }
        self.checkpoint_entries = entries;
        self.checkpoint_restore = restore;
    }

    pub(crate) fn record_session(
        &mut self,
        settings: &crate::app::config::CoderSettings,
        messages: &[ChatMessage],
        is_thinking: bool,
    ) {
        if !settings.session_auto_save {
            return;
        }
        let session_id = self.session_info.session_id.trim();
        if session_id.is_empty() {
            return;
        }

        let now = super::super::now_timestamp();
        let last_message = messages
            .iter()
            .rev()
            .find(|msg| !msg.content.trim().is_empty())
            .map(|msg| super::super::truncate_preview(&msg.content, 140))
            .unwrap_or_default();

        if let Some(entry) = self.session_index.iter_mut().find(|entry| entry.id == session_id) {
            entry.updated_at = now;
            entry.last_message = last_message;
            entry.message_count = messages.len();
            entry.model = self.session_info.model.clone();
        } else {
            self.session_index.push(SessionEntry {
                id: session_id.to_string(),
                created_at: now,
                updated_at: now,
                last_message,
                message_count: messages.len(),
                model: self.session_info.model.clone(),
            });
        }

        let removed_sessions = super::apply_session_history_limit(
            &mut self.session_index,
            settings.session_history_limit,
        );

        if let Err(err) = super::save_session_index(&self.session_index) {
            tracing::error!("Failed to save session index: {}", err);
        }
        if let Err(err) = super::write_session_messages(session_id, messages) {
            tracing::error!("Failed to write session messages: {}", err);
        }
        if let Some(entry) = self.session_index.iter().find(|entry| entry.id == session_id) {
            if let Err(err) = super::write_session_metadata(session_id, entry) {
                tracing::error!("Failed to write session metadata: {}", err);
            }
        }
        for removed_id in removed_sessions {
            let _ = fs::remove_dir_all(session_messages_dir(&removed_id));
        }
        self.refresh_session_cards(is_thinking);
        self.refresh_checkpoint_restore(messages);
    }

    pub(crate) fn restore_session(
        &mut self,
        session_id: &str,
        chat: &mut ChatState,
        tools: &mut ToolsState,
    ) -> io::Result<()> {
        chat.messages = super::read_session_messages(session_id)?;
        chat.streaming_markdown.reset();
        chat.scroll_offset = 0.0;
        tools.current_tool_name = None;
        tools.current_tool_input.clear();
        self.refresh_checkpoint_restore(&chat.messages);
        Ok(())
    }

    pub(crate) fn apply_history_limit(&mut self, limit: usize, chat_is_thinking: bool) {
        let removed = super::apply_session_history_limit(&mut self.session_index, limit);
        if !removed.is_empty() {
            let _ = super::save_session_index(&self.session_index);
            for removed_id in removed {
                let _ = fs::remove_dir_all(session_messages_dir(&removed_id));
            }
            self.refresh_session_cards(chat_is_thinking);
        }
    }

    pub(crate) fn handle_session_card_action(
        &mut self,
        action: SessionAction,
        session_id: String,
        chat: &mut ChatState,
        tools: &mut ToolsState,
        modal_state: &mut ModalState,
    ) {
        match action {
            SessionAction::Select | SessionAction::Resume => {
                self.begin_session_resume(session_id, chat, tools);
                *modal_state = ModalState::None;
            }
            SessionAction::Fork => {
                self.begin_session_fork_from(session_id, chat, tools);
                *modal_state = ModalState::None;
            }
            SessionAction::Delete => {
                chat.push_system_message("Session delete not implemented yet.".to_string());
            }
        }
    }

    pub(crate) fn handle_checkpoint_restore(&mut self, index: usize, chat: &mut ChatState) {
        if let Some(entry) = self.checkpoint_entries.get(index) {
            chat.request_rewind_files(entry.user_message_id.clone());
        }
    }

    pub(crate) fn begin_session_fork_from(
        &mut self,
        session_id: String,
        chat: &mut ChatState,
        tools: &mut ToolsState,
    ) {
        let session_id = session_id.trim().to_string();
        if session_id.is_empty() {
            chat.push_system_message("Session id is required to fork.".to_string());
            return;
        }
        self.pending_resume_session = Some(session_id.clone());
        self.pending_fork_session = true;
        self.session_info.session_id = session_id.clone();
        if let Some(entry) = self.session_index.iter().find(|entry| entry.id == session_id) {
            self.session_info.model = entry.model.clone();
        }
        match self.restore_session(&session_id, chat, tools) {
            Ok(()) => chat.push_system_message(format!(
                "Loaded cached history for session {}.",
                session_id
            )),
            Err(_) => {
                chat.messages.clear();
                chat.push_system_message(format!(
                    "No local history for session {} yet.",
                    session_id
                ));
            }
        }
        chat.push_system_message(format!(
            "Next message will fork session {}.",
            session_id
        ));
        self.refresh_session_cards(chat.is_thinking);
    }

    pub(crate) fn begin_session_resume(
        &mut self,
        session_id: String,
        chat: &mut ChatState,
        tools: &mut ToolsState,
    ) {
        let session_id = session_id.trim().to_string();
        if session_id.is_empty() {
            chat.push_system_message("Session id is required to resume.".to_string());
            return;
        }
        self.pending_resume_session = Some(session_id.clone());
        self.pending_fork_session = false;
        self.session_info.session_id = session_id.clone();
        if let Some(entry) = self.session_index.iter().find(|entry| entry.id == session_id) {
            self.session_info.model = entry.model.clone();
        }
        match self.restore_session(&session_id, chat, tools) {
            Ok(()) => chat.push_system_message(format!(
                "Loaded cached history for session {}.",
                session_id
            )),
            Err(_) => {
                chat.messages.clear();
                chat.push_system_message(format!(
                    "No local history for session {} yet.",
                    session_id
                ));
            }
        }
        self.refresh_session_cards(chat.is_thinking);
    }

    pub(crate) fn begin_session_fork(&mut self, chat: &mut ChatState) {
        if self.session_info.session_id.trim().is_empty() {
            chat.push_system_message("No active session to fork.".to_string());
            return;
        }
        self.pending_resume_session = Some(self.session_info.session_id.clone());
        self.pending_fork_session = true;
        chat.push_system_message("Next message will fork the current session.".to_string());
    }

    pub(crate) fn clear_conversation(&mut self, chat: &mut ChatState, tools: &mut ToolsState) {
        if chat.is_thinking {
            chat.push_system_message("Cannot clear while a response is in progress.".to_string());
            return;
        }
        chat.messages.clear();
        chat.streaming_markdown.reset();
        chat.scroll_offset = 0.0;
        tools.current_tool_name = None;
        tools.current_tool_input.clear();
        tools.current_tool_use_id = None;
        tools.tool_history.clear();
        self.session_info.session_id.clear();
        self.session_info.tool_count = 0;
        self.session_info.tools.clear();
        self.pending_resume_session = None;
        self.pending_fork_session = false;
        self.checkpoint_entries.clear();
        self.checkpoint_restore = CheckpointRestore::new();
        self.refresh_session_cards(chat.is_thinking);
    }

    pub(crate) fn start_new_session(&mut self, chat: &mut ChatState, tools: &mut ToolsState) {
        if chat.is_thinking {
            chat.push_system_message("Cannot start new session while processing.".to_string());
            return;
        }
        chat.messages.clear();
        chat.streaming_markdown.reset();
        chat.scroll_offset = 0.0;
        tools.current_tool_name = None;
        tools.current_tool_input.clear();
        tools.current_tool_use_id = None;
        tools.tool_history.clear();
        self.session_usage = SessionUsageStats::default();
        self.session_info.session_id.clear();
        self.session_info.tool_count = 0;
        self.session_info.tools.clear();
        self.pending_resume_session = None;
        self.pending_fork_session = false;
        self.checkpoint_entries.clear();
        self.checkpoint_restore = CheckpointRestore::new();
        self.refresh_session_cards(chat.is_thinking);
        chat.push_system_message("Started new session.".to_string());
    }

    pub(crate) fn undo_last_exchange(&mut self, chat: &mut ChatState) {
        if chat.is_thinking {
            chat.push_system_message("Cannot undo while a response is in progress.".to_string());
            return;
        }

        let mut removed = 0;
        while matches!(
            chat.messages.last(),
            Some(ChatMessage {
                role: MessageRole::Assistant | MessageRole::AssistantThought,
                ..
            })
        ) {
            chat.messages.pop();
            removed += 1;
        }
        if matches!(
            chat.messages.last(),
            Some(ChatMessage {
                role: MessageRole::User,
                ..
            })
        ) {
            chat.messages.pop();
            removed += 1;
        }

        if removed == 0 {
            chat.push_system_message("Nothing to undo.".to_string());
        } else {
            self.refresh_checkpoint_restore(&chat.messages);
        }
    }
}
