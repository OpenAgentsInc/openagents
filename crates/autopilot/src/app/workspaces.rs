use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;
use web_time::Instant;

use crate::app::codex_app_server as app_server;
use crate::app::config::workspaces_file;

const THREAD_NAME_MAX_LEN: usize = 38;
const FOCUS_REFRESH_COOLDOWN_SECS: u64 = 2;
const MAX_ITEMS_PER_THREAD: usize = 400;
const MAX_ITEM_TEXT: usize = 20000;
const MAX_TOOL_TITLE_TEXT: usize = 200;
const MAX_TOOL_DETAIL_TEXT: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) codex_bin: Option<String>,
    #[serde(default)]
    pub(crate) settings: WorkspaceSettings,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) codex_bin: Option<String>,
    pub(crate) connected: bool,
    pub(crate) settings: WorkspaceSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceSettings {
    #[serde(default)]
    pub(crate) sidebar_collapsed: bool,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceThreadSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) preview: String,
    pub(crate) created_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) enum ConversationRole {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub(crate) enum ReviewState {
    Started,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceAccessMode {
    ReadOnly,
    Current,
    FullAccess,
}

impl WorkspaceAccessMode {
    pub(crate) fn all() -> [WorkspaceAccessMode; 3] {
        [
            WorkspaceAccessMode::ReadOnly,
            WorkspaceAccessMode::Current,
            WorkspaceAccessMode::FullAccess,
        ]
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            WorkspaceAccessMode::ReadOnly => "Read only",
            WorkspaceAccessMode::Current => "Current",
            WorkspaceAccessMode::FullAccess => "Full access",
        }
    }

    pub(crate) fn approval_policy(self) -> app_server::AskForApproval {
        match self {
            WorkspaceAccessMode::FullAccess => app_server::AskForApproval::Never,
            WorkspaceAccessMode::ReadOnly | WorkspaceAccessMode::Current => {
                app_server::AskForApproval::OnRequest
            }
        }
    }

    pub(crate) fn sandbox_mode(self) -> app_server::SandboxMode {
        match self {
            WorkspaceAccessMode::FullAccess => app_server::SandboxMode::DangerFullAccess,
            WorkspaceAccessMode::ReadOnly => app_server::SandboxMode::ReadOnly,
            WorkspaceAccessMode::Current => app_server::SandboxMode::WorkspaceWrite,
        }
    }

    pub(crate) fn sandbox_policy(self, workspace_path: &str) -> app_server::SandboxPolicy {
        match self {
            WorkspaceAccessMode::FullAccess => app_server::SandboxPolicy::DangerFullAccess,
            WorkspaceAccessMode::ReadOnly => app_server::SandboxPolicy::ReadOnly,
            WorkspaceAccessMode::Current => app_server::SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![workspace_path.to_string()],
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
        }
    }

    pub(crate) fn auto_approves(self) -> bool {
        matches!(self, WorkspaceAccessMode::FullAccess)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceApprovalRequest {
    pub(crate) id: app_server::AppServerRequestId,
    pub(crate) method: String,
    pub(crate) params: Value,
}

impl WorkspaceApprovalRequest {
    pub(crate) fn id_label(&self) -> String {
        match &self.id {
            app_server::AppServerRequestId::String(value) => value.clone(),
            app_server::AppServerRequestId::Integer(value) => value.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceComposerState {
    pub(crate) models: Vec<app_server::ModelInfo>,
    pub(crate) selected_model_id: Option<String>,
    pub(crate) selected_effort: Option<app_server::ReasoningEffort>,
    pub(crate) access_mode: WorkspaceAccessMode,
    pub(crate) skills: Vec<app_server::SkillMetadata>,
    pub(crate) models_loaded: bool,
    pub(crate) skills_loaded: bool,
    pub(crate) models_pending: bool,
    pub(crate) skills_pending: bool,
    pub(crate) models_error: Option<String>,
    pub(crate) skills_error: Option<String>,
}

impl Default for WorkspaceComposerState {
    fn default() -> Self {
        Self {
            models: Vec::new(),
            selected_model_id: None,
            selected_effort: None,
            access_mode: WorkspaceAccessMode::FullAccess,
            skills: Vec::new(),
            models_loaded: false,
            skills_loaded: false,
            models_pending: false,
            skills_pending: false,
            models_error: None,
            skills_error: None,
        }
    }
}

impl WorkspaceComposerState {
    pub(crate) fn selected_model(&self) -> Option<&app_server::ModelInfo> {
        let selected_id = self.selected_model_id.as_deref()?;
        self.models.iter().find(|model| model.id == selected_id)
    }

    pub(crate) fn reasoning_options(&self) -> Vec<app_server::ReasoningEffort> {
        self.selected_model()
            .map(|model| {
                model
                    .supported_reasoning_efforts
                    .iter()
                    .map(|option| option.reasoning_effort)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn set_models(&mut self, models: Vec<app_server::ModelInfo>) {
        self.models = models;
        self.models_loaded = true;
        self.models_pending = false;
        self.models_error = None;

        if self.models.is_empty() {
            self.selected_model_id = None;
            self.selected_effort = None;
            return;
        }

        let selected = self
            .selected_model_id
            .as_ref()
            .and_then(|id| self.models.iter().find(|model| &model.id == id))
            .cloned();

        let chosen = selected
            .or_else(|| {
                self.models
                    .iter()
                    .find(|model| model.model == "gpt-5.2-codex")
                    .cloned()
            })
            .or_else(|| self.models.iter().find(|model| model.is_default).cloned())
            .or_else(|| self.models.first().cloned());

        if let Some(model) = chosen {
            self.selected_model_id = Some(model.id.clone());
            self.selected_effort = Some(model.default_reasoning_effort);
        }
    }

    pub(crate) fn set_skills(&mut self, skills: Vec<app_server::SkillMetadata>) {
        self.skills = skills;
        self.skills_loaded = true;
        self.skills_pending = false;
        self.skills_error = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComposerMenuKind {
    Model,
    Effort,
    Access,
    Skill,
}

#[derive(Clone, Debug)]
pub(crate) struct ComposerLabels {
    pub(crate) model: String,
    pub(crate) effort: String,
    pub(crate) access: String,
    pub(crate) skill: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ConversationChange {
    pub(crate) path: String,
    pub(crate) kind: Option<String>,
    pub(crate) diff: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ToolItemData {
    pub(crate) tool_name: String,
    pub(crate) tool_type: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) status: Option<String>,
    pub(crate) output: String,
    pub(crate) input_value: Option<Value>,
    pub(crate) output_value: Option<Value>,
    pub(crate) changes: Vec<ConversationChange>,
}

#[derive(Clone, Debug)]
pub(crate) enum ConversationItem {
    Message {
        id: String,
        role: ConversationRole,
        text: String,
    },
    Reasoning {
        id: String,
        summary: String,
        content: String,
    },
    Tool {
        id: String,
        data: ToolItemData,
    },
    Review {
        id: String,
        state: ReviewState,
        text: String,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadStatus {
    pub(crate) is_processing: bool,
    pub(crate) has_unread: bool,
    pub(crate) is_reviewing: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspacePlanSnapshot {
    pub(crate) turn_id: String,
    pub(crate) explanation: Option<String>,
    pub(crate) steps: Vec<WorkspacePlanStep>,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspacePlanStep {
    pub(crate) step: String,
    pub(crate) status: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadTimeline {
    pub(crate) items: Vec<ConversationItem>,
    index_by_id: HashMap<String, usize>,
}

impl ThreadTimeline {
    pub(crate) fn upsert(&mut self, item: ConversationItem) {
        let mut item = item;
        truncate_item(&mut item);
        let id = item.id().to_string();
        if let Some(index) = self.index_by_id.get(&id).copied() {
            if let Some(existing) = self.items.get_mut(index) {
                existing.merge(item);
                truncate_item(existing);
            }
            self.trim_to_limit();
            return;
        }
        self.index_by_id.insert(id, self.items.len());
        self.items.push(item);
        self.trim_to_limit();
    }

    pub(crate) fn append_agent_delta(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Message { text, .. }) = self.items.get_mut(index) {
                text.push_str(delta);
                *text = truncate_text(text, MAX_ITEM_TEXT);
            }
            self.trim_to_limit();
            return;
        }
        let item = ConversationItem::Message {
            id: item_id.to_string(),
            role: ConversationRole::Assistant,
            text: delta.to_string(),
        };
        self.index_by_id
            .insert(item_id.to_string(), self.items.len());
        self.items.push(item);
        self.trim_to_limit();
    }

    pub(crate) fn append_reasoning_summary(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Reasoning { summary, .. }) = self.items.get_mut(index) {
                summary.push_str(delta);
                *summary = truncate_text(summary, MAX_ITEM_TEXT);
            }
            self.trim_to_limit();
            return;
        }
        let item = ConversationItem::Reasoning {
            id: item_id.to_string(),
            summary: delta.to_string(),
            content: String::new(),
        };
        self.index_by_id
            .insert(item_id.to_string(), self.items.len());
        self.items.push(item);
        self.trim_to_limit();
    }

    pub(crate) fn append_reasoning_content(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Reasoning { content, .. }) = self.items.get_mut(index) {
                content.push_str(delta);
                *content = truncate_text(content, MAX_ITEM_TEXT);
            }
            self.trim_to_limit();
            return;
        }
        let item = ConversationItem::Reasoning {
            id: item_id.to_string(),
            summary: String::new(),
            content: delta.to_string(),
        };
        self.index_by_id
            .insert(item_id.to_string(), self.items.len());
        self.items.push(item);
        self.trim_to_limit();
    }

    pub(crate) fn append_tool_output(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Tool { data, .. }) = self.items.get_mut(index) {
                data.output.push_str(delta);
                data.output = truncate_text(&data.output, MAX_ITEM_TEXT);
            }
        }
        self.trim_to_limit();
    }

    fn trim_to_limit(&mut self) {
        if self.items.len() <= MAX_ITEMS_PER_THREAD {
            return;
        }
        let trim_count = self.items.len().saturating_sub(MAX_ITEMS_PER_THREAD);
        self.items.drain(0..trim_count);
        self.index_by_id.clear();
        for (index, item) in self.items.iter().enumerate() {
            self.index_by_id.insert(item.id().to_string(), index);
        }
    }
}

impl ConversationItem {
    pub(crate) fn id(&self) -> &str {
        match self {
            ConversationItem::Message { id, .. } => id,
            ConversationItem::Reasoning { id, .. } => id,
            ConversationItem::Tool { id, .. } => id,
            ConversationItem::Review { id, .. } => id,
        }
    }

    fn merge(&mut self, incoming: ConversationItem) {
        match (self, incoming) {
            (
                ConversationItem::Message { role, text, .. },
                ConversationItem::Message {
                    role: new_role,
                    text: new_text,
                    ..
                },
            ) => {
                *role = new_role;
                if !new_text.is_empty() && new_text.len() >= text.len() {
                    *text = new_text;
                }
            }
            (
                ConversationItem::Reasoning {
                    summary, content, ..
                },
                ConversationItem::Reasoning {
                    summary: new_summary,
                    content: new_content,
                    ..
                },
            ) => {
                if !new_summary.is_empty() && new_summary.len() >= summary.len() {
                    *summary = new_summary;
                }
                if !new_content.is_empty() && new_content.len() >= content.len() {
                    *content = new_content;
                }
            }
            (
                ConversationItem::Tool { data, .. },
                ConversationItem::Tool { data: new_data, .. },
            ) => {
                data.merge(new_data);
            }
            (
                ConversationItem::Review { state, text, .. },
                ConversationItem::Review {
                    state: new_state,
                    text: new_text,
                    ..
                },
            ) => {
                *state = new_state;
                if !new_text.is_empty() {
                    *text = new_text;
                }
            }
            (slot, incoming) => {
                *slot = incoming;
            }
        }
    }
}

impl ToolItemData {
    fn merge(&mut self, incoming: ToolItemData) {
        if !incoming.tool_name.is_empty() {
            self.tool_name = incoming.tool_name;
        }
        if !incoming.tool_type.is_empty() {
            self.tool_type = incoming.tool_type;
        }
        if !incoming.title.is_empty() && self.title.is_empty() {
            self.title = incoming.title;
        }
        if !incoming.detail.is_empty() && self.detail.is_empty() {
            self.detail = incoming.detail;
        }
        if let Some(status) = incoming.status {
            if !status.trim().is_empty() {
                self.status = Some(status);
            }
        }
        if !incoming.output.is_empty() && incoming.output.len() >= self.output.len() {
            self.output = incoming.output;
        }
        if let Some(input_value) = incoming.input_value {
            if self.input_value.is_none() {
                self.input_value = Some(input_value);
            }
        }
        if let Some(output_value) = incoming.output_value {
            if self.output_value.is_none() {
                self.output_value = Some(output_value);
            }
        }
        if !incoming.changes.is_empty() && self.changes.is_empty() {
            self.changes = incoming.changes;
        }
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    let slice_len = max_len.saturating_sub(3);
    let truncated: String = text.chars().take(slice_len).collect();
    format!("{}...", truncated)
}

fn truncate_item(item: &mut ConversationItem) {
    match item {
        ConversationItem::Message { text, .. } => {
            *text = truncate_text(text, MAX_ITEM_TEXT);
        }
        ConversationItem::Reasoning { summary, content, .. } => {
            *summary = truncate_text(summary, MAX_ITEM_TEXT);
            *content = truncate_text(content, MAX_ITEM_TEXT);
        }
        ConversationItem::Tool { data, .. } => {
            data.title = truncate_text(&data.title, MAX_TOOL_TITLE_TEXT);
            data.detail = truncate_text(&data.detail, MAX_TOOL_DETAIL_TEXT);
            data.output = truncate_text(&data.output, MAX_ITEM_TEXT);
            for change in &mut data.changes {
                if let Some(diff) = change.diff.as_mut() {
                    *diff = truncate_text(diff, MAX_ITEM_TEXT);
                }
            }
        }
        ConversationItem::Review { text, .. } => {
            *text = truncate_text(text, MAX_ITEM_TEXT);
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkspaceEvent {
    WorkspacesLoaded {
        workspaces: Vec<WorkspaceInfo>,
    },
    WorkspaceAdded {
        workspace: WorkspaceInfo,
    },
    WorkspaceAddFailed {
        workspace_id: String,
        error: String,
    },
    WorkspaceConnected {
        workspace_id: String,
    },
    WorkspaceSettingsUpdated {
        workspace_id: String,
        settings: WorkspaceSettings,
    },
    WorkspaceSettingsUpdateFailed {
        workspace_id: String,
        error: String,
    },
    WorkspaceConnectFailed {
        workspace_id: String,
        error: String,
    },
    ThreadsListed {
        workspace_id: String,
        threads: Vec<WorkspaceThreadSummary>,
    },
    ThreadsListFailed {
        workspace_id: String,
        error: String,
    },
    ThreadStarted {
        workspace_id: String,
        thread_id: String,
    },
    ThreadStartFailed {
        workspace_id: String,
        error: String,
    },
    ThreadResumed {
        workspace_id: String,
        thread_id: String,
        items: Vec<ConversationItem>,
        reviewing: bool,
        preview: Option<String>,
    },
    ThreadResumeFailed {
        workspace_id: String,
        thread_id: String,
        error: String,
    },
    ThreadArchived {
        workspace_id: String,
        thread_id: String,
    },
    ThreadArchiveFailed {
        workspace_id: String,
        thread_id: String,
        error: String,
    },
    UserMessageQueued {
        workspace_id: String,
        thread_id: String,
        text: String,
    },
    UserMessageFailed {
        workspace_id: String,
        error: String,
    },
    ReviewQueued {
        workspace_id: String,
        thread_id: String,
        label: String,
    },
    ReviewFailed {
        workspace_id: String,
        error: String,
    },
    ModelsListed {
        workspace_id: String,
        models: Vec<app_server::ModelInfo>,
    },
    ModelListFailed {
        workspace_id: String,
        error: String,
    },
    SkillsListed {
        workspace_id: String,
        skills: Vec<app_server::SkillMetadata>,
        error: Option<String>,
    },
    SkillsListFailed {
        workspace_id: String,
        error: String,
    },
    AppServerNotification {
        workspace_id: String,
        notification: app_server::AppServerNotification,
    },
    AppServerRequest {
        workspace_id: String,
        request: app_server::AppServerRequest,
    },
}

#[derive(Debug)]
pub(crate) enum WorkspaceCommand {
    Reload,
    Add {
        path: PathBuf,
        codex_bin: Option<String>,
    },
    Connect {
        workspace_id: String,
    },
    UpdateSettings {
        workspace_id: String,
        settings: WorkspaceSettings,
    },
    ListThreads {
        workspace_id: String,
    },
    StartThread {
        workspace_id: String,
        model: Option<String>,
        access_mode: WorkspaceAccessMode,
    },
    ResumeThread {
        workspace_id: String,
        thread_id: String,
    },
    ArchiveThread {
        workspace_id: String,
        thread_id: String,
    },
    SendUserMessage {
        workspace_id: String,
        thread_id: Option<String>,
        text: String,
        model: Option<String>,
        effort: Option<app_server::ReasoningEffort>,
        access_mode: WorkspaceAccessMode,
    },
    StartReview {
        workspace_id: String,
        thread_id: Option<String>,
        target: app_server::ReviewTarget,
        delivery: Option<app_server::ReviewDelivery>,
        label: String,
        access_mode: WorkspaceAccessMode,
    },
    ListModels {
        workspace_id: String,
    },
    ListSkills {
        workspace_id: String,
        force_reload: bool,
    },
    RespondToRequest {
        workspace_id: String,
        request_id: app_server::AppServerRequestId,
        response: app_server::ApprovalResponse,
    },
}

pub(crate) struct WorkspaceRuntime {
    cmd_tx: mpsc::Sender<WorkspaceCommand>,
    pub(crate) event_rx: mpsc::Receiver<WorkspaceEvent>,
}

impl WorkspaceRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<WorkspaceCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<WorkspaceEvent>(256);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_workspace_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn reload(&self) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::Reload);
    }

    pub(crate) fn add_workspace(&self, path: PathBuf, codex_bin: Option<String>) {
        let _ = self
            .cmd_tx
            .try_send(WorkspaceCommand::Add { path, codex_bin });
    }

    pub(crate) fn connect_workspace(&self, workspace_id: String) {
        let _ = self
            .cmd_tx
            .try_send(WorkspaceCommand::Connect { workspace_id });
    }

    pub(crate) fn update_workspace_settings(
        &self,
        workspace_id: String,
        settings: WorkspaceSettings,
    ) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::UpdateSettings {
            workspace_id,
            settings,
        });
    }

    pub(crate) fn list_threads(&self, workspace_id: String) {
        let _ = self
            .cmd_tx
            .try_send(WorkspaceCommand::ListThreads { workspace_id });
    }

    pub(crate) fn start_thread(
        &self,
        workspace_id: String,
        model: Option<String>,
        access_mode: WorkspaceAccessMode,
    ) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::StartThread {
            workspace_id,
            model,
            access_mode,
        });
    }

    pub(crate) fn resume_thread(&self, workspace_id: String, thread_id: String) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::ResumeThread {
            workspace_id,
            thread_id,
        });
    }

    pub(crate) fn archive_thread(&self, workspace_id: String, thread_id: String) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::ArchiveThread {
            workspace_id,
            thread_id,
        });
    }

    pub(crate) fn send_user_message(
        &self,
        workspace_id: String,
        thread_id: Option<String>,
        text: String,
        model: Option<String>,
        effort: Option<app_server::ReasoningEffort>,
        access_mode: WorkspaceAccessMode,
    ) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::SendUserMessage {
            workspace_id,
            thread_id,
            text,
            model,
            effort,
            access_mode,
        });
    }

    pub(crate) fn start_review(
        &self,
        workspace_id: String,
        thread_id: Option<String>,
        target: app_server::ReviewTarget,
        delivery: Option<app_server::ReviewDelivery>,
        label: String,
        access_mode: WorkspaceAccessMode,
    ) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::StartReview {
            workspace_id,
            thread_id,
            target,
            delivery,
            label,
            access_mode,
        });
    }

    pub(crate) fn list_models(&self, workspace_id: String) {
        let _ = self
            .cmd_tx
            .try_send(WorkspaceCommand::ListModels { workspace_id });
    }

    pub(crate) fn list_skills(&self, workspace_id: String, force_reload: bool) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::ListSkills {
            workspace_id,
            force_reload,
        });
    }

    pub(crate) fn respond_to_request(
        &self,
        workspace_id: String,
        request_id: app_server::AppServerRequestId,
        response: app_server::ApprovalResponse,
    ) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::RespondToRequest {
            workspace_id,
            request_id,
            response,
        });
    }
}

impl Default for WorkspaceRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct WorkspaceState {
    pub(crate) runtime: WorkspaceRuntime,
    pub(crate) workspaces: Vec<WorkspaceInfo>,
    pub(crate) threads_by_workspace: HashMap<String, Vec<WorkspaceThreadSummary>>,
    pub(crate) active_workspace_id: Option<String>,
    pub(crate) active_thread_by_workspace: HashMap<String, String>,
    pub(crate) thread_status_by_id: HashMap<String, ThreadStatus>,
    pub(crate) composer_by_workspace: HashMap<String, WorkspaceComposerState>,
    pub(crate) approvals_by_workspace: HashMap<String, Vec<WorkspaceApprovalRequest>>,
    pub(crate) composer_menu: Option<ComposerMenuKind>,
    pub(crate) timelines_by_thread: HashMap<String, ThreadTimeline>,
    pub(crate) plans_by_thread: HashMap<String, WorkspacePlanSnapshot>,
    pub(crate) expanded_workspaces: HashSet<String>,
    pub(crate) loaded_threads: HashSet<String>,
    pub(crate) timeline_dirty: bool,
    pub(crate) status_message: Option<String>,
    pub(crate) home_active: bool,
    last_focus_refresh: Option<Instant>,
    initial_restore_pending: bool,
}

impl WorkspaceState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: WorkspaceRuntime::new(),
            workspaces: Vec::new(),
            threads_by_workspace: HashMap::new(),
            active_workspace_id: None,
            active_thread_by_workspace: HashMap::new(),
            thread_status_by_id: HashMap::new(),
            composer_by_workspace: HashMap::new(),
            approvals_by_workspace: HashMap::new(),
            composer_menu: None,
            timelines_by_thread: HashMap::new(),
            plans_by_thread: HashMap::new(),
            expanded_workspaces: HashSet::new(),
            loaded_threads: HashSet::new(),
            timeline_dirty: false,
            status_message: None,
            home_active: false,
            last_focus_refresh: None,
            initial_restore_pending: true,
        }
    }

    pub(crate) fn apply_loaded(&mut self, workspaces: Vec<WorkspaceInfo>) {
        let active_id = self.active_workspace_id.clone();
        self.workspaces = workspaces;
        self.composer_by_workspace
            .retain(|id, _| self.workspaces.iter().any(|ws| ws.id == *id));
        self.approvals_by_workspace
            .retain(|id, _| self.workspaces.iter().any(|ws| ws.id == *id));
        for workspace in &self.workspaces {
            self.composer_by_workspace
                .entry(workspace.id.clone())
                .or_default();
        }
        if let Some(active_id) = active_id {
            if self.workspaces.iter().any(|ws| ws.id == active_id) {
                self.active_workspace_id = Some(active_id);
            } else {
                self.active_workspace_id = self.workspaces.first().map(|ws| ws.id.clone());
            }
        } else {
            self.active_workspace_id = self.workspaces.first().map(|ws| ws.id.clone());
        }
        self.timeline_dirty = true;
    }

    pub(crate) fn apply_workspace_added(&mut self, workspace: WorkspaceInfo) {
        if self.workspaces.iter().any(|ws| ws.id == workspace.id) {
            return;
        }
        self.active_workspace_id = Some(workspace.id.clone());
        self.composer_by_workspace
            .entry(workspace.id.clone())
            .or_default();
        self.workspaces.push(workspace);
        self.timeline_dirty = true;
    }

    pub(crate) fn apply_workspace_connected(&mut self, workspace_id: &str) {
        if let Some(workspace) = self
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == workspace_id)
        {
            workspace.connected = true;
        }
        self.composer_by_workspace
            .entry(workspace_id.to_string())
            .or_default();
    }

    pub(crate) fn apply_workspace_settings(&mut self, workspace_id: &str, settings: WorkspaceSettings) {
        if let Some(workspace) = self
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == workspace_id)
        {
            workspace.settings = settings;
        }
        self.timeline_dirty = true;
    }

    pub(crate) fn apply_threads(
        &mut self,
        workspace_id: String,
        threads: Vec<WorkspaceThreadSummary>,
    ) {
        for thread in &threads {
            self.thread_status_by_id
                .entry(thread.id.clone())
                .or_default();
        }
        if let Some(first) = threads.first() {
            self.active_thread_by_workspace
                .entry(workspace_id.clone())
                .or_insert_with(|| first.id.clone());
        }
        self.threads_by_workspace.insert(workspace_id, threads);
        self.timeline_dirty = true;
    }

    pub(crate) fn apply_thread_started(&mut self, workspace_id: &str, thread_id: &str) {
        self.ensure_thread(workspace_id, thread_id);
        self.set_active_thread(workspace_id, thread_id);
        self.timeline_dirty = true;
    }

    pub(crate) fn apply_thread_resumed(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        items: Vec<ConversationItem>,
        reviewing: bool,
        preview: Option<String>,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        if let Some(preview) = preview {
            self.update_thread_preview(workspace_id, thread_id, &preview);
        }
        let timeline = self.timeline_for_thread_mut(thread_id);
        for item in items {
            timeline.upsert(item);
        }
        if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
            status.is_reviewing = reviewing;
        }
        self.set_thread_loaded(thread_id);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn apply_thread_archived(&mut self, workspace_id: &str, thread_id: &str) {
        if let Some(list) = self.threads_by_workspace.get_mut(workspace_id) {
            list.retain(|thread| thread.id != thread_id);
        }
        self.thread_status_by_id.remove(thread_id);
        self.timelines_by_thread.remove(thread_id);
        self.plans_by_thread.remove(thread_id);
        self.loaded_threads.remove(thread_id);
        if let Some(active) = self.active_thread_by_workspace.get(workspace_id) {
            if active == thread_id {
                self.active_thread_by_workspace.remove(workspace_id);
                if let Some(first) = self
                    .threads_by_workspace
                    .get(workspace_id)
                    .and_then(|threads| threads.first())
                {
                    self.active_thread_by_workspace
                        .insert(workspace_id.to_string(), first.id.clone());
                }
            }
        }
        self.timeline_dirty = true;
    }

    pub(crate) fn connect_all_if_needed(&mut self) {
        if !self.initial_restore_pending {
            return;
        }
        self.initial_restore_pending = false;
        for workspace in &self.workspaces {
            if !workspace.connected {
                self.runtime.connect_workspace(workspace.id.clone());
            }
        }
    }

    pub(crate) fn set_active_workspace(&mut self, workspace_id: String) {
        if self.active_workspace_id.as_deref() == Some(&workspace_id) {
            return;
        }
        self.active_workspace_id = Some(workspace_id);
        self.home_active = false;
        self.composer_menu = None;
        self.timeline_dirty = true;
    }

    pub(crate) fn set_home_active(&mut self, active: bool) {
        self.home_active = active;
        self.timeline_dirty = true;
    }

    pub(crate) fn composer_state(&self, workspace_id: &str) -> Option<&WorkspaceComposerState> {
        self.composer_by_workspace.get(workspace_id)
    }

    pub(crate) fn composer_state_mut(&mut self, workspace_id: &str) -> &mut WorkspaceComposerState {
        self.composer_by_workspace
            .entry(workspace_id.to_string())
            .or_default()
    }

    pub(crate) fn active_composer(&self) -> Option<&WorkspaceComposerState> {
        let workspace_id = self.active_workspace_id.as_ref()?;
        self.composer_by_workspace.get(workspace_id)
    }

    pub(crate) fn active_composer_mut(&mut self) -> Option<&mut WorkspaceComposerState> {
        let workspace_id = self.active_workspace_id.clone()?;
        Some(self.composer_by_workspace.entry(workspace_id).or_default())
    }

    pub(crate) fn composer_labels(&self) -> ComposerLabels {
        let Some(workspace_id) = self.active_workspace_id.as_ref() else {
            return ComposerLabels {
                model: "No workspace".to_string(),
                effort: "Effort".to_string(),
                access: "Access".to_string(),
                skill: "Skill".to_string(),
            };
        };
        let composer = self.composer_by_workspace.get(workspace_id);
        let (model_label, effort_label) = if let Some(composer) = composer {
            if composer.models.is_empty() {
                let label = if composer.models_pending {
                    "Models...".to_string()
                } else if composer.models_error.is_some() {
                    "Models failed".to_string()
                } else {
                    "No models".to_string()
                };
                (label, "Effort".to_string())
            } else {
                let model_name = composer
                    .selected_model()
                    .map(|model| {
                        if !model.display_name.trim().is_empty() {
                            model.display_name.clone()
                        } else if !model.model.trim().is_empty() {
                            model.model.clone()
                        } else {
                            model.id.clone()
                        }
                    })
                    .unwrap_or_else(|| "Model".to_string());
                let effort_name = composer
                    .selected_effort
                    .map(reasoning_effort_label)
                    .unwrap_or("default");
                (
                    format!("Model: {}", model_name),
                    format!("Effort: {}", effort_name),
                )
            }
        } else {
            ("Model".to_string(), "Effort".to_string())
        };

        let access_label = composer
            .map(|composer| format!("Access: {}", composer.access_mode.label()))
            .unwrap_or_else(|| "Access".to_string());
        let skill_label = "Skill".to_string();

        ComposerLabels {
            model: model_label,
            effort: effort_label,
            access: access_label,
            skill: skill_label,
        }
    }

    pub(crate) fn set_models_for_workspace(
        &mut self,
        workspace_id: &str,
        models: Vec<app_server::ModelInfo>,
    ) {
        let composer = self.composer_state_mut(workspace_id);
        composer.set_models(models);
    }

    pub(crate) fn set_skills_for_workspace(
        &mut self,
        workspace_id: &str,
        skills: Vec<app_server::SkillMetadata>,
    ) {
        let composer = self.composer_state_mut(workspace_id);
        composer.set_skills(skills);
    }

    pub(crate) fn set_models_error(&mut self, workspace_id: &str, error: String) {
        let composer = self.composer_state_mut(workspace_id);
        composer.models_error = Some(error);
        composer.models_pending = false;
    }

    pub(crate) fn set_skills_error(&mut self, workspace_id: &str, error: String) {
        let composer = self.composer_state_mut(workspace_id);
        composer.skills_error = Some(error);
        composer.skills_pending = false;
    }

    pub(crate) fn request_composer_data(&mut self, workspace_id: &str) {
        let connected = self
            .workspaces
            .iter()
            .find(|workspace| workspace.id == workspace_id)
            .map(|workspace| workspace.connected)
            .unwrap_or(false);
        if !connected {
            return;
        }
        let (fetch_models, fetch_skills) = {
            let composer = self.composer_state_mut(workspace_id);
            let mut fetch_models = false;
            let mut fetch_skills = false;
            if !composer.models_loaded && !composer.models_pending {
                composer.models_pending = true;
                fetch_models = true;
            }
            if !composer.skills_loaded && !composer.skills_pending {
                composer.skills_pending = true;
                fetch_skills = true;
            }
            (fetch_models, fetch_skills)
        };
        if fetch_models {
            self.runtime.list_models(workspace_id.to_string());
        }
        if fetch_skills {
            self.runtime.list_skills(workspace_id.to_string(), false);
        }
    }

    pub(crate) fn approvals_for_workspace(
        &self,
        workspace_id: &str,
    ) -> &[WorkspaceApprovalRequest] {
        self.approvals_by_workspace
            .get(workspace_id)
            .map(|items| items.as_slice())
            .unwrap_or_default()
    }

    pub(crate) fn approvals_for_active(&self) -> &[WorkspaceApprovalRequest] {
        let Some(workspace_id) = self.active_workspace_id.as_ref() else {
            return &[];
        };
        self.approvals_for_workspace(workspace_id)
    }

    pub(crate) fn add_approval(&mut self, workspace_id: &str, request: WorkspaceApprovalRequest) {
        self.approvals_by_workspace
            .entry(workspace_id.to_string())
            .or_default()
            .push(request);
        self.timeline_dirty = true;
    }

    pub(crate) fn remove_approval(&mut self, workspace_id: &str, request_id: &str) {
        if let Some(list) = self.approvals_by_workspace.get_mut(workspace_id) {
            list.retain(|item| item.id_label() != request_id);
        }
        self.timeline_dirty = true;
    }

    pub(crate) fn active_thread_status(&self) -> Option<&ThreadStatus> {
        let thread_id = self.active_thread_id()?;
        self.thread_status_by_id.get(&thread_id)
    }

    pub(crate) fn active_thread_is_reviewing(&self) -> bool {
        self.active_thread_status()
            .map(|status| status.is_reviewing)
            .unwrap_or(false)
    }

    pub(crate) fn is_workspace_expanded(&self, workspace_id: &str) -> bool {
        self.expanded_workspaces.contains(workspace_id)
    }

    pub(crate) fn toggle_workspace_expanded(&mut self, workspace_id: &str) {
        if self.expanded_workspaces.contains(workspace_id) {
            self.expanded_workspaces.remove(workspace_id);
        } else {
            self.expanded_workspaces.insert(workspace_id.to_string());
        }
        self.timeline_dirty = true;
    }

    pub(crate) fn set_thread_loaded(&mut self, thread_id: &str) {
        self.loaded_threads.insert(thread_id.to_string());
    }

    pub(crate) fn is_thread_loaded(&self, thread_id: &str) -> bool {
        self.loaded_threads.contains(thread_id)
    }

    pub(crate) fn set_plan_for_thread(
        &mut self,
        thread_id: &str,
        plan: WorkspacePlanSnapshot,
    ) {
        self.plans_by_thread
            .insert(thread_id.to_string(), plan);
        self.timeline_dirty = true;
    }

    pub(crate) fn plan_for_thread(&self, thread_id: &str) -> Option<&WorkspacePlanSnapshot> {
        self.plans_by_thread.get(thread_id)
    }

    pub(crate) fn set_composer_menu(&mut self, menu: Option<ComposerMenuKind>) {
        self.composer_menu = menu;
    }

    pub(crate) fn update_thread_preview(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        preview: &str,
    ) {
        if let Some(threads) = self.threads_by_workspace.get_mut(workspace_id) {
            for thread in threads.iter_mut() {
                if thread.id == thread_id {
                    let fallback =
                        format!("Agent {}", thread.id.chars().take(4).collect::<String>());
                    thread.preview = preview.to_string();
                    thread.name = format_thread_name(preview, &fallback);
                    break;
                }
            }
        }
    }

    pub(crate) fn active_thread_id(&self) -> Option<String> {
        let workspace_id = self.active_workspace_id.as_ref()?;
        self.active_thread_id_for(workspace_id)
    }

    pub(crate) fn active_thread_id_for(&self, workspace_id: &str) -> Option<String> {
        self.active_thread_by_workspace
            .get(workspace_id)
            .cloned()
            .or_else(|| {
                self.threads_by_workspace
                    .get(workspace_id)
                    .and_then(|threads| threads.first().map(|thread| thread.id.clone()))
            })
    }

    pub(crate) fn set_active_thread(&mut self, workspace_id: &str, thread_id: &str) {
        self.active_thread_by_workspace
            .insert(workspace_id.to_string(), thread_id.to_string());
        if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
            status.has_unread = false;
        }
        self.timeline_dirty = true;
    }

    pub(crate) fn ensure_thread(&mut self, workspace_id: &str, thread_id: &str) {
        let threads = self
            .threads_by_workspace
            .entry(workspace_id.to_string())
            .or_default();
        if !threads.iter().any(|thread| thread.id == thread_id) {
            let name = format!("Agent {}", threads.len() + 1);
            threads.push(WorkspaceThreadSummary {
                id: thread_id.to_string(),
                name,
                preview: String::new(),
                created_at: 0,
            });
        }
        self.thread_status_by_id
            .entry(thread_id.to_string())
            .or_default();
        if !self.active_thread_by_workspace.contains_key(workspace_id) {
            self.active_thread_by_workspace
                .insert(workspace_id.to_string(), thread_id.to_string());
        }
    }

    pub(crate) fn timeline_for_thread_mut(&mut self, thread_id: &str) -> &mut ThreadTimeline {
        self.timelines_by_thread
            .entry(thread_id.to_string())
            .or_default()
    }

    pub(crate) fn active_timeline(&self) -> Option<&ThreadTimeline> {
        let thread_id = self.active_thread_id()?;
        self.timelines_by_thread.get(&thread_id)
    }

    pub(crate) fn mark_processing(&mut self, workspace_id: &str, thread_id: &str, active: bool) {
        self.ensure_thread(workspace_id, thread_id);
        if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
            status.is_processing = active;
        }
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn mark_reviewing(&mut self, workspace_id: &str, thread_id: &str, active: bool) {
        self.ensure_thread(workspace_id, thread_id);
        if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
            status.is_reviewing = active;
        }
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn mark_unread(&mut self, workspace_id: &str, thread_id: &str) {
        self.ensure_thread(workspace_id, thread_id);
        if !self.is_active_thread(workspace_id, thread_id) {
            if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
                status.has_unread = true;
            }
        }
    }

    pub(crate) fn apply_item_update(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item: ConversationItem,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.upsert(item);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_agent_delta(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_agent_delta(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_reasoning_summary(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_reasoning_summary(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_reasoning_content(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_reasoning_content(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_tool_output(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_tool_output(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    fn is_active_thread(&self, workspace_id: &str, thread_id: &str) -> bool {
        self.active_thread_by_workspace
            .get(workspace_id)
            .map(|active| active == thread_id)
            .unwrap_or(false)
    }

    fn mark_dirty_for(&mut self, workspace_id: &str, thread_id: &str) {
        if self.is_active_thread(workspace_id, thread_id) {
            self.timeline_dirty = true;
        } else {
            self.mark_unread(workspace_id, thread_id);
        }
    }

    pub(crate) fn refresh_on_focus(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_focus_refresh {
            if now.duration_since(last).as_secs() < FOCUS_REFRESH_COOLDOWN_SECS {
                return;
            }
        }
        self.last_focus_refresh = Some(now);
        self.runtime.reload();
        for workspace in &self.workspaces {
            if workspace.connected {
                self.runtime.list_threads(workspace.id.clone());
            }
        }
    }

    pub(crate) fn list_workspace_summary(&self) -> String {
        if self.workspaces.is_empty() {
            return "No workspaces saved. Use /workspace add to add one.".to_string();
        }
        let mut lines = Vec::new();
        for workspace in &self.workspaces {
            let status = if workspace.connected {
                "connected"
            } else {
                "offline"
            };
            lines.push(format!(
                "- {} ({})\n  id: {}\n  path: {}",
                workspace.name, status, workspace.id, workspace.path
            ));
        }
        lines.join("\n")
    }

    pub(crate) fn connect_by_hint(&self, hint: &str) -> Option<String> {
        let trimmed = hint.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(workspace) = self.workspaces.iter().find(|ws| ws.id == trimmed) {
            return Some(workspace.id.clone());
        }
        if let Some(workspace) = self.workspaces.iter().find(|ws| ws.id.starts_with(trimmed)) {
            return Some(workspace.id.clone());
        }
        if let Some(workspace) = self
            .workspaces
            .iter()
            .find(|ws| ws.name.to_lowercase() == trimmed.to_lowercase())
        {
            return Some(workspace.id.clone());
        }
        None
    }
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}

struct WorkspaceSession {
    entry: WorkspaceEntry,
    client: app_server::AppServerClient,
    notification_task: tokio::task::JoinHandle<()>,
    request_task: tokio::task::JoinHandle<()>,
}

async fn run_workspace_loop(
    mut cmd_rx: mpsc::Receiver<WorkspaceCommand>,
    event_tx: mpsc::Sender<WorkspaceEvent>,
) {
    let storage_path = workspaces_file();
    let mut workspaces = load_workspace_entries(&storage_path);
    let mut sessions: HashMap<String, WorkspaceSession> = HashMap::new();

    let loaded = workspaces
        .values()
        .map(|entry| entry_to_info(entry, sessions.contains_key(&entry.id)))
        .collect::<Vec<_>>();
    let _ = event_tx
        .send(WorkspaceEvent::WorkspacesLoaded { workspaces: loaded })
        .await;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            WorkspaceCommand::Reload => {
                workspaces = load_workspace_entries(&storage_path);
                let missing_ids: Vec<String> = sessions
                    .keys()
                    .filter(|id| !workspaces.contains_key(*id))
                    .cloned()
                    .collect();
                for id in missing_ids {
                    if let Some(session) = sessions.remove(&id) {
                        session.notification_task.abort();
                        session.request_task.abort();
                        let _ = session.client.shutdown().await;
                    }
                }
                let loaded = workspaces
                    .values()
                    .map(|entry| entry_to_info(entry, sessions.contains_key(&entry.id)))
                    .collect::<Vec<_>>();
                let _ = event_tx
                    .send(WorkspaceEvent::WorkspacesLoaded { workspaces: loaded })
                    .await;
            }
            WorkspaceCommand::Add { path, codex_bin } => {
                let entry = create_workspace_entry(&path, codex_bin);
                workspaces.insert(entry.id.clone(), entry.clone());
                if let Err(err) = save_workspace_entries(&storage_path, &workspaces) {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceAddFailed {
                            workspace_id: entry.id.clone(),
                            error: err,
                        })
                        .await;
                    continue;
                }
                let info = entry_to_info(&entry, false);
                let _ = event_tx
                    .send(WorkspaceEvent::WorkspaceAdded { workspace: info })
                    .await;
                if let Err(err) = connect_workspace_session(&entry, &mut sessions, &event_tx).await
                {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceConnectFailed {
                            workspace_id: entry.id.clone(),
                            error: err,
                        })
                        .await;
                }
            }
            WorkspaceCommand::Connect { workspace_id } => {
                let entry = match workspaces.get(&workspace_id) {
                    Some(entry) => entry.clone(),
                    None => {
                        let _ = event_tx
                            .send(WorkspaceEvent::WorkspaceConnectFailed {
                                workspace_id,
                                error: "Workspace not found".to_string(),
                            })
                            .await;
                        continue;
                    }
                };
                if sessions.contains_key(&entry.id) {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceConnected {
                            workspace_id: entry.id.clone(),
                        })
                        .await;
                    continue;
                }
                if let Err(err) = connect_workspace_session(&entry, &mut sessions, &event_tx).await
                {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceConnectFailed {
                            workspace_id: entry.id.clone(),
                            error: err,
                        })
                    .await;
                }
            }
            WorkspaceCommand::UpdateSettings {
                workspace_id,
                settings,
            } => {
                let entry_id = match workspaces.get_mut(&workspace_id) {
                    Some(entry) => {
                        entry.settings = settings.clone();
                        entry.id.clone()
                    }
                    None => {
                        let _ = event_tx
                            .send(WorkspaceEvent::WorkspaceSettingsUpdateFailed {
                                workspace_id,
                                error: "Workspace not found".to_string(),
                            })
                            .await;
                        continue;
                    }
                };
                if let Err(err) = save_workspace_entries(&storage_path, &workspaces) {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceSettingsUpdateFailed {
                            workspace_id: entry_id.clone(),
                            error: err,
                        })
                        .await;
                    continue;
                }
                let _ = event_tx
                    .send(WorkspaceEvent::WorkspaceSettingsUpdated {
                        workspace_id: entry_id,
                        settings,
                    })
                    .await;
            }
            WorkspaceCommand::ListThreads { workspace_id } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::ThreadsListFailed {
                            workspace_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                match list_threads_for_workspace(session).await {
                    Ok(threads) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadsListed {
                                workspace_id: session.entry.id.clone(),
                                threads,
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadsListFailed {
                                workspace_id: session.entry.id.clone(),
                                error: err,
                            })
                            .await;
                    }
                }
            }
            WorkspaceCommand::StartThread {
                workspace_id,
                model,
                access_mode,
            } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::ThreadStartFailed {
                            workspace_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                let start_params = app_server::ThreadStartParams {
                    model,
                    model_provider: None,
                    cwd: Some(session.entry.path.clone()),
                    approval_policy: Some(access_mode.approval_policy()),
                    sandbox: Some(access_mode.sandbox_mode()),
                };
                match session.client.thread_start(start_params).await {
                    Ok(response) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadStarted {
                                workspace_id: session.entry.id.clone(),
                                thread_id: response.thread.id,
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadStartFailed {
                                workspace_id: session.entry.id.clone(),
                                error: format!("thread/start failed: {}", err),
                            })
                            .await;
                    }
                }
            }
            WorkspaceCommand::ResumeThread {
                workspace_id,
                thread_id,
            } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::ThreadResumeFailed {
                            workspace_id,
                            thread_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                let resume_params = app_server::ThreadResumeParams {
                    thread_id: thread_id.clone(),
                    model: None,
                    model_provider: None,
                    cwd: Some(session.entry.path.clone()),
                    approval_policy: None,
                    sandbox: None,
                };
                match session.client.thread_resume(resume_params).await {
                    Ok(response) => {
                        let (items, reviewing, preview) = items_from_thread_snapshot(&response.thread);
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadResumed {
                                workspace_id: session.entry.id.clone(),
                                thread_id: response.thread.id,
                                items,
                                reviewing,
                                preview,
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadResumeFailed {
                                workspace_id: session.entry.id.clone(),
                                thread_id,
                                error: format!("thread/resume failed: {}", err),
                            })
                            .await;
                    }
                }
            }
            WorkspaceCommand::ArchiveThread {
                workspace_id,
                thread_id,
            } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::ThreadArchiveFailed {
                            workspace_id,
                            thread_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                let params = app_server::ThreadArchiveParams {
                    thread_id: thread_id.clone(),
                };
                match session.client.thread_archive(params).await {
                    Ok(_response) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadArchived {
                                workspace_id: session.entry.id.clone(),
                                thread_id,
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadArchiveFailed {
                                workspace_id: session.entry.id.clone(),
                                thread_id,
                                error: format!("thread/archive failed: {}", err),
                            })
                            .await;
                    }
                }
            }
            WorkspaceCommand::SendUserMessage {
                workspace_id,
                thread_id,
                text,
                model,
                effort,
                access_mode,
            } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::UserMessageFailed {
                            workspace_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                let mut thread_id = thread_id;
                if thread_id.is_none() {
                    let start_params = app_server::ThreadStartParams {
                        model: model.clone(),
                        model_provider: None,
                        cwd: Some(session.entry.path.clone()),
                        approval_policy: Some(access_mode.approval_policy()),
                        sandbox: Some(access_mode.sandbox_mode()),
                    };
                    match session.client.thread_start(start_params).await {
                        Ok(response) => {
                            thread_id = Some(response.thread.id);
                        }
                        Err(err) => {
                            let _ = event_tx
                                .send(WorkspaceEvent::UserMessageFailed {
                                    workspace_id: session.entry.id.clone(),
                                    error: format!("thread/start failed: {}", err),
                                })
                                .await;
                            continue;
                        }
                    }
                }
                let Some(thread_id) = thread_id else {
                    continue;
                };
                let _ = event_tx
                    .send(WorkspaceEvent::UserMessageQueued {
                        workspace_id: session.entry.id.clone(),
                        thread_id: thread_id.clone(),
                        text: text.clone(),
                    })
                    .await;

                let turn_params = app_server::TurnStartParams {
                    thread_id: thread_id.clone(),
                    input: vec![app_server::UserInput::Text { text }],
                    model,
                    effort,
                    summary: None,
                    approval_policy: Some(access_mode.approval_policy()),
                    sandbox_policy: Some(access_mode.sandbox_policy(&session.entry.path)),
                    cwd: Some(session.entry.path.clone()),
                };
                if let Err(err) = session.client.turn_start(turn_params).await {
                    let _ = event_tx
                        .send(WorkspaceEvent::UserMessageFailed {
                            workspace_id: session.entry.id.clone(),
                            error: format!("turn/start failed: {}", err),
                        })
                        .await;
                }
            }
            WorkspaceCommand::StartReview {
                workspace_id,
                thread_id,
                target,
                delivery,
                label,
                access_mode,
            } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::ReviewFailed {
                            workspace_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                let mut thread_id = thread_id;
                if thread_id.is_none() {
                    let start_params = app_server::ThreadStartParams {
                        model: None,
                        model_provider: None,
                        cwd: Some(session.entry.path.clone()),
                        approval_policy: Some(access_mode.approval_policy()),
                        sandbox: Some(access_mode.sandbox_mode()),
                    };
                    match session.client.thread_start(start_params).await {
                        Ok(response) => {
                            thread_id = Some(response.thread.id);
                        }
                        Err(err) => {
                            let _ = event_tx
                                .send(WorkspaceEvent::ReviewFailed {
                                    workspace_id: session.entry.id.clone(),
                                    error: format!("thread/start failed: {}", err),
                                })
                                .await;
                            continue;
                        }
                    }
                }
                let Some(thread_id) = thread_id else {
                    continue;
                };
                let _ = event_tx
                    .send(WorkspaceEvent::ReviewQueued {
                        workspace_id: session.entry.id.clone(),
                        thread_id: thread_id.clone(),
                        label,
                    })
                    .await;
                let review_params = app_server::ReviewStartParams {
                    thread_id: thread_id.clone(),
                    target,
                    delivery,
                };
                if let Err(err) = session.client.review_start(review_params).await {
                    let _ = event_tx
                        .send(WorkspaceEvent::ReviewFailed {
                            workspace_id: session.entry.id.clone(),
                            error: format!("review/start failed: {}", err),
                        })
                        .await;
                }
            }
            WorkspaceCommand::ListModels { workspace_id } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::ModelListFailed {
                            workspace_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                match session
                    .client
                    .model_list(app_server::ModelListParams {
                        cursor: None,
                        limit: None,
                    })
                    .await
                {
                    Ok(response) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ModelsListed {
                                workspace_id: session.entry.id.clone(),
                                models: response.data,
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ModelListFailed {
                                workspace_id: session.entry.id.clone(),
                                error: format!("model/list failed: {}", err),
                            })
                            .await;
                    }
                }
            }
            WorkspaceCommand::ListSkills {
                workspace_id,
                force_reload,
            } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::SkillsListFailed {
                            workspace_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                match session
                    .client
                    .skills_list(app_server::SkillsListParams {
                        cwds: vec![session.entry.path.clone()],
                        force_reload,
                    })
                    .await
                {
                    Ok(response) => {
                        let mut skills = Vec::new();
                        let mut errors = Vec::new();
                        for entry in response.data {
                            skills.extend(entry.skills);
                            errors.extend(entry.errors);
                        }
                        let error = if errors.is_empty() {
                            None
                        } else {
                            Some(
                                errors
                                    .into_iter()
                                    .map(|err| format!("{}: {}", err.path, err.message))
                                    .collect::<Vec<_>>()
                                    .join(" | "),
                            )
                        };
                        let _ = event_tx
                            .send(WorkspaceEvent::SkillsListed {
                                workspace_id: session.entry.id.clone(),
                                skills,
                                error,
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::SkillsListFailed {
                                workspace_id: session.entry.id.clone(),
                                error: format!("skills/list failed: {}", err),
                            })
                            .await;
                    }
                }
            }
            WorkspaceCommand::RespondToRequest {
                workspace_id,
                request_id,
                response,
            } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    continue;
                };
                if let Err(err) = session.client.respond(request_id, &response).await {
                    tracing::warn!(error = %err, "Failed to respond to app-server request");
                }
            }
        }
    }
}

async fn connect_workspace_session(
    entry: &WorkspaceEntry,
    sessions: &mut HashMap<String, WorkspaceSession>,
    event_tx: &mpsc::Sender<WorkspaceEvent>,
) -> Result<(), String> {
    let (client, channels) = app_server::AppServerClient::spawn(app_server::AppServerConfig {
        cwd: Some(PathBuf::from(&entry.path)),
        wire_log: None,
    })
    .await
    .map_err(|err| format!("Failed to spawn app-server: {}", err))?;

    let client_info = app_server::ClientInfo {
        name: "autopilot".to_string(),
        title: Some("Autopilot".to_string()),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    client
        .initialize(client_info)
        .await
        .map_err(|err| format!("Failed to initialize app-server: {}", err))?;

    let workspace_id = entry.id.clone();
    let mut notification_rx = channels.notifications;
    let event_tx_notify = event_tx.clone();
    let notification_task = tokio::spawn(async move {
        while let Some(notification) = notification_rx.recv().await {
            let _ = event_tx_notify
                .send(WorkspaceEvent::AppServerNotification {
                    workspace_id: workspace_id.clone(),
                    notification,
                })
                .await;
        }
    });

    let workspace_id = entry.id.clone();
    let mut request_rx = channels.requests;
    let event_tx_request = event_tx.clone();
    let request_task = tokio::spawn(async move {
        while let Some(request) = request_rx.recv().await {
            let _ = event_tx_request
                .send(WorkspaceEvent::AppServerRequest {
                    workspace_id: workspace_id.clone(),
                    request,
                })
                .await;
        }
    });

    sessions.insert(
        entry.id.clone(),
        WorkspaceSession {
            entry: entry.clone(),
            client,
            notification_task,
            request_task,
        },
    );

    let _ = event_tx
        .send(WorkspaceEvent::WorkspaceConnected {
            workspace_id: entry.id.clone(),
        })
        .await;

    if let Some(session) = sessions.get(&entry.id) {
        match list_threads_for_workspace(session).await {
            Ok(threads) => {
                let _ = event_tx
                    .send(WorkspaceEvent::ThreadsListed {
                        workspace_id: entry.id.clone(),
                        threads,
                    })
                    .await;
            }
            Err(err) => {
                let _ = event_tx
                    .send(WorkspaceEvent::ThreadsListFailed {
                        workspace_id: entry.id.clone(),
                        error: err,
                    })
                    .await;
            }
        }
    }

    Ok(())
}

async fn list_threads_for_workspace(
    session: &WorkspaceSession,
) -> Result<Vec<WorkspaceThreadSummary>, String> {
    let mut threads: Vec<app_server::ThreadSummary> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let response = session
            .client
            .thread_list(app_server::ThreadListParams {
                cursor: cursor.clone(),
                limit: Some(50),
                model_providers: None,
            })
            .await
            .map_err(|err| format!("thread/list failed: {}", err))?;
        threads.extend(response.data);
        cursor = response.next_cursor;
        if cursor.is_none() {
            break;
        }
    }

    let filtered = threads
        .into_iter()
        .filter(|thread| {
            thread
                .cwd
                .as_deref()
                .map(|cwd| cwd == session.entry.path)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    let mut ordered = filtered;
    ordered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(ordered
        .into_iter()
        .enumerate()
        .map(|(idx, thread)| {
            let preview = thread.preview.trim().to_string();
            let fallback = format!("Agent {}", idx + 1);
            WorkspaceThreadSummary {
                id: thread.id,
                name: format_thread_name(&preview, &fallback),
                preview,
                created_at: thread.created_at,
            }
        })
        .collect())
}

fn format_thread_name(preview: &str, fallback: &str) -> String {
    if preview.is_empty() {
        return fallback.to_string();
    }
    let mut chars = preview.chars();
    let truncated: String = chars.by_ref().take(THREAD_NAME_MAX_LEN).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        preview.to_string()
    }
}

pub(crate) fn reasoning_effort_label(effort: app_server::ReasoningEffort) -> &'static str {
    match effort {
        app_server::ReasoningEffort::None => "none",
        app_server::ReasoningEffort::Minimal => "minimal",
        app_server::ReasoningEffort::Low => "low",
        app_server::ReasoningEffort::Medium => "medium",
        app_server::ReasoningEffort::High => "high",
        app_server::ReasoningEffort::XHigh => "x-high",
    }
}

fn create_workspace_entry(path: &Path, codex_bin: Option<String>) -> WorkspaceEntry {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Workspace")
        .to_string();
    WorkspaceEntry {
        id: Uuid::new_v4().to_string(),
        name,
        path: path.to_string_lossy().to_string(),
        codex_bin,
        settings: WorkspaceSettings::default(),
    }
}

fn entry_to_info(entry: &WorkspaceEntry, connected: bool) -> WorkspaceInfo {
    WorkspaceInfo {
        id: entry.id.clone(),
        name: entry.name.clone(),
        path: entry.path.clone(),
        codex_bin: entry.codex_bin.clone(),
        connected,
        settings: entry.settings.clone(),
    }
}

fn load_workspace_entries(path: &Path) -> HashMap<String, WorkspaceEntry> {
    if !path.exists() {
        return HashMap::new();
    }
    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(_) => return HashMap::new(),
    };
    let entries: Vec<WorkspaceEntry> = serde_json::from_str(&data).unwrap_or_default();
    entries
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect()
}

fn save_workspace_entries(
    path: &Path,
    entries: &HashMap<String, WorkspaceEntry>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let list: Vec<WorkspaceEntry> = entries.values().cloned().collect();
    let data = serde_json::to_string_pretty(&list).map_err(|err| err.to_string())?;
    std::fs::write(path, data).map_err(|err| err.to_string())
}

pub(crate) fn conversation_item_from_value(item: &Value) -> Option<ConversationItem> {
    let item_type = item.get("type").and_then(Value::as_str)?;
    let id = item.get("id").and_then(Value::as_str)?.to_string();
    match item_type {
        "userMessage" => {
            let text = item
                .get("content")
                .and_then(Value::as_array)
                .map(|parts| join_user_inputs(parts))
                .or_else(|| item.get("text").and_then(Value::as_str).map(|s| s.to_string()))
                .unwrap_or_default();
            Some(ConversationItem::Message {
                id,
                role: ConversationRole::User,
                text,
            })
        }
        "agentMessage" => {
            let text = item
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            Some(ConversationItem::Message {
                id,
                role: ConversationRole::Assistant,
                text,
            })
        }
        "reasoning" => {
            let summary = item
                .get("summary")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts))
                .or_else(|| {
                    item.get("summary")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            let content = item
                .get("content")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts))
                .or_else(|| {
                    item.get("content")
                        .and_then(Value::as_str)
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            Some(ConversationItem::Reasoning {
                id,
                summary,
                content,
            })
        }
        "commandExecution" => {
            let command = command_string_from_item(item);
            let cwd = item
                .get("cwd")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let title = if command.is_empty() {
                "Command".to_string()
            } else {
                format!("Command: {}", command)
            };
            let output = item
                .get("aggregatedOutput")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let mut data = ToolItemData {
                tool_name: "Bash".to_string(),
                tool_type: item_type.to_string(),
                title,
                detail: cwd,
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                output,
                ..ToolItemData::default()
            };
            if !command.is_empty() {
                data.input_value = Some(serde_json::json!({ "command": command }));
            }
            Some(ConversationItem::Tool { id, data })
        }
        "fileChange" => {
            let (changes, paths, first_path, first_diff) = parse_file_changes(item);
            let detail = if paths.is_empty() {
                "Pending changes".to_string()
            } else {
                paths.join(", ")
            };
            let output = changes
                .iter()
                .filter_map(|change| change.diff.clone())
                .filter(|diff| !diff.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            let mut data = ToolItemData {
                tool_name: "Edit".to_string(),
                tool_type: item_type.to_string(),
                title: "File changes".to_string(),
                detail,
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                output,
                changes,
                ..ToolItemData::default()
            };
            if !paths.is_empty() || first_path.is_some() {
                data.input_value = Some(serde_json::json!({
                    "files": paths,
                    "file_path": first_path,
                }));
            }
            if let Some(diff) = first_diff {
                data.output_value = Some(serde_json::json!({ "diff": diff }));
            }
            Some(ConversationItem::Tool { id, data })
        }
        "mcpToolCall" => {
            let server = item_string(item, "server");
            let tool = item_string(item, "tool");
            let tool_name = match (server.as_deref(), tool.as_deref()) {
                (Some(server), Some(tool)) => format!("mcp__{}__{}", server, tool),
                (Some(server), None) => format!("mcp__{}__", server),
                _ => "mcp__tool".to_string(),
            };
            let args = item.get("arguments").cloned();
            let detail = args
                .as_ref()
                .map(|value| serde_json::to_string_pretty(value).unwrap_or_default())
                .unwrap_or_default();
            let output_value = item
                .get("result")
                .cloned()
                .or_else(|| item.get("error").cloned());
            let output = output_value
                .as_ref()
                .map(|value| serde_json::to_string_pretty(value).unwrap_or_default())
                .unwrap_or_default();
            let title = match (server.as_deref(), tool.as_deref()) {
                (Some(server), Some(tool)) => format!("Tool: {} / {}", server, tool),
                (Some(server), None) => format!("Tool: {}", server),
                _ => "MCP tool".to_string(),
            };
            let data = ToolItemData {
                tool_name,
                tool_type: item_type.to_string(),
                title,
                detail,
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                output,
                input_value: args,
                output_value,
                ..ToolItemData::default()
            };
            Some(ConversationItem::Tool { id, data })
        }
        "webSearch" => {
            let query = item_string(item, "query").unwrap_or_default();
            let data = ToolItemData {
                tool_name: "Search".to_string(),
                tool_type: item_type.to_string(),
                title: "Web search".to_string(),
                detail: query.clone(),
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                input_value: if query.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({ "query": query }))
                },
                ..ToolItemData::default()
            };
            Some(ConversationItem::Tool { id, data })
        }
        "imageView" => {
            let path = item_string(item, "path").unwrap_or_default();
            let data = ToolItemData {
                tool_name: "Read".to_string(),
                tool_type: item_type.to_string(),
                title: "Image view".to_string(),
                detail: path.clone(),
                status: item
                    .get("status")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string()),
                input_value: if path.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({ "file_path": path }))
                },
                ..ToolItemData::default()
            };
            Some(ConversationItem::Tool { id, data })
        }
        "enteredReviewMode" => {
            let text = item_string(item, "review").unwrap_or_default();
            Some(ConversationItem::Review {
                id,
                state: ReviewState::Started,
                text,
            })
        }
        "exitedReviewMode" => {
            let text = item_string(item, "review").unwrap_or_default();
            Some(ConversationItem::Review {
                id,
                state: ReviewState::Completed,
                text,
            })
        }
        _ => None,
    }
}

pub(crate) fn turn_diff_item(turn_id: &str, diff: &str) -> ConversationItem {
    let data = ToolItemData {
        tool_name: "Diff".to_string(),
        tool_type: "diff".to_string(),
        title: "Turn diff".to_string(),
        detail: String::new(),
        status: Some("updated".to_string()),
        output: diff.to_string(),
        output_value: Some(serde_json::json!({ "diff": diff })),
        ..ToolItemData::default()
    };
    ConversationItem::Tool {
        id: format!("turn-diff-{}", turn_id),
        data,
    }
}

fn items_from_thread_snapshot(
    thread: &app_server::ThreadSnapshot,
) -> (Vec<ConversationItem>, bool, Option<String>) {
    let mut items = Vec::new();
    let mut reviewing = false;
    for turn in &thread.turns {
        for item in &turn.items {
            if let Some(item_type) = item.get("type").and_then(Value::as_str) {
                match item_type {
                    "enteredReviewMode" => reviewing = true,
                    "exitedReviewMode" => reviewing = false,
                    _ => {}
                }
            }
            if let Some(converted) = conversation_item_from_value(item) {
                items.push(converted);
            }
        }
    }
    let preview = if thread.preview.trim().is_empty() {
        None
    } else {
        Some(thread.preview.trim().to_string())
    };
    (items, reviewing, preview)
}

fn join_string_array(parts: &[Value]) -> String {
    parts
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn join_user_inputs(parts: &[Value]) -> String {
    let mut lines = Vec::new();
    for entry in parts {
        if let Some(text) = entry.get("text").and_then(Value::as_str) {
            if !text.trim().is_empty() {
                lines.push(text.to_string());
            }
            continue;
        }
        if let Some(value) = entry.as_str() {
            if !value.trim().is_empty() {
                lines.push(value.to_string());
            }
        }
    }
    lines.join("\n")
}

fn item_string(item: &Value, key: &str) -> Option<String> {
    item.get(key).and_then(Value::as_str).map(|s| s.to_string())
}

fn command_string_from_item(item: &Value) -> String {
    item.get("command")
        .and_then(value_to_command_string)
        .unwrap_or_default()
}

fn value_to_command_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Value::Array(parts) => {
            let items: Vec<&str> = parts.iter().filter_map(|val| val.as_str()).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(" "))
            }
        }
        _ => None,
    }
}

fn parse_file_changes(
    item: &Value,
) -> (
    Vec<ConversationChange>,
    Vec<String>,
    Option<String>,
    Option<String>,
) {
    let mut changes = Vec::new();
    let mut paths = Vec::new();
    let mut first_path = None;
    let mut first_diff = None;
    if let Some(change_list) = item.get("changes").and_then(Value::as_array) {
        for change in change_list {
            let path = change
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if path.is_empty() {
                continue;
            }
            if first_path.is_none() {
                first_path = Some(path.clone());
            }
            let kind = change
                .get("kind")
                .and_then(|value| {
                    if value.is_string() {
                        value.as_str().map(|s| s.to_string())
                    } else {
                        value
                            .get("type")
                            .and_then(Value::as_str)
                            .map(|s| s.to_string())
                    }
                })
                .map(|value| value.to_ascii_lowercase());
            let diff = change
                .get("diff")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            if first_diff.is_none() {
                first_diff = diff.clone();
            }
            let prefix = match kind.as_deref() {
                Some("add") => "A",
                Some("delete") => "D",
                Some(_) => "M",
                None => "",
            };
            let display = if prefix.is_empty() {
                path.clone()
            } else {
                format!("{} {}", prefix, path)
            };
            paths.push(display);
            changes.push(ConversationChange { path, kind, diff });
        }
    }
    (changes, paths, first_path, first_diff)
}
