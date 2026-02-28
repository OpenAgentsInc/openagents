use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{cell::RefCell, rc::Rc};

use nostr::NostrIdentity;
use wgpui::components::TextInput;
use wgpui::components::hud::{CommandPalette, Hotbar, PaneFrame, ResizablePane, ResizeEdge};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, EventContext, Modifiers, Point, TextSystem};
use winit::window::Window;

use crate::nip_sa_wallet_bridge::spark_total_balance_sats;
use crate::runtime_lanes::{
    AcCreditCommand, AcLaneSnapshot, AcLaneWorker, RuntimeCommandResponse, SaLaneSnapshot,
    SaLaneWorker, SaLifecycleCommand, SkillTrustTier, SklDiscoveryTrustCommand, SklLaneSnapshot,
    SklLaneWorker,
};
use crate::{
    codex_lane::{
        CodexLaneCommand, CodexLaneCommandResponse, CodexLaneNotification, CodexLaneSnapshot,
        CodexLaneWorker,
    },
    spark_wallet::{SparkPaneState, SparkWalletWorker},
};

pub const WINDOW_TITLE: &str = "Autopilot";
pub const WINDOW_WIDTH: f64 = 1280.0;
pub const WINDOW_HEIGHT: f64 = 800.0;

pub struct App {
    pub state: Option<RenderState>,
    pub cursor_position: Point,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            cursor_position: Point::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneKind {
    Empty,
    AutopilotChat,
    CodexAccount,
    CodexModels,
    CodexConfig,
    CodexMcp,
    CodexApps,
    CodexRemoteSkills,
    CodexLabs,
    CodexDiagnostics,
    GoOnline,
    ProviderStatus,
    EarningsScoreboard,
    RelayConnections,
    SyncHealth,
    NetworkRequests,
    StarterJobs,
    ActivityFeed,
    AlertsRecovery,
    Settings,
    JobInbox,
    ActiveJob,
    JobHistory,
    NostrIdentity,
    SparkWallet,
    SparkCreateInvoice,
    SparkPayInvoice,
    AgentProfileState,
    AgentScheduleTick,
    TrajectoryAudit,
    SkillRegistry,
    SkillTrustRevocation,
    CreditDesk,
    CreditSettlementLedger,
    AgentNetworkSimulation,
    TreasuryExchangeSimulation,
    RelaySecuritySimulation,
}

#[derive(Clone, Copy)]
pub enum PaneDragMode {
    Moving {
        pane_id: u64,
        start_mouse: Point,
        start_bounds: Bounds,
    },
    Resizing {
        pane_id: u64,
        edge: ResizeEdge,
        start_mouse: Point,
        start_bounds: Bounds,
    },
}

pub struct DesktopPane {
    pub id: u64,
    pub title: String,
    pub kind: PaneKind,
    pub bounds: Bounds,
    pub z_index: i32,
    pub frame: PaneFrame,
}

pub struct SparkPaneInputs {
    pub invoice_amount: TextInput,
    pub send_request: TextInput,
    pub send_amount: TextInput,
}

impl Default for SparkPaneInputs {
    fn default() -> Self {
        Self {
            invoice_amount: TextInput::new().value("1000").placeholder("Invoice sats"),
            send_request: TextInput::new()
                .placeholder("Spark payment request or invoice")
                .mono(true),
            send_amount: TextInput::new().placeholder("Send sats (optional)"),
        }
    }
}

pub struct PayInvoicePaneInputs {
    pub payment_request: TextInput,
    pub amount_sats: TextInput,
}

impl Default for PayInvoicePaneInputs {
    fn default() -> Self {
        Self {
            payment_request: TextInput::new()
                .placeholder("Lightning invoice / payment request")
                .mono(true),
            amount_sats: TextInput::new().placeholder("Send sats (optional)"),
        }
    }
}

pub struct CreateInvoicePaneInputs {
    pub amount_sats: TextInput,
    pub description: TextInput,
    pub expiry_seconds: TextInput,
}

impl Default for CreateInvoicePaneInputs {
    fn default() -> Self {
        Self {
            amount_sats: TextInput::new().value("1000").placeholder("Invoice sats"),
            description: TextInput::new().placeholder("Description (optional)"),
            expiry_seconds: TextInput::new().value("3600").placeholder("Expiry seconds"),
        }
    }
}

pub struct RelayConnectionsPaneInputs {
    pub relay_url: TextInput,
}

impl Default for RelayConnectionsPaneInputs {
    fn default() -> Self {
        Self {
            relay_url: TextInput::new()
                .value("wss://relay.example.com")
                .placeholder("wss://relay.example.com"),
        }
    }
}

pub struct NetworkRequestsPaneInputs {
    pub request_type: TextInput,
    pub payload: TextInput,
    pub skill_scope_id: TextInput,
    pub credit_envelope_ref: TextInput,
    pub budget_sats: TextInput,
    pub timeout_seconds: TextInput,
}

impl Default for NetworkRequestsPaneInputs {
    fn default() -> Self {
        Self {
            request_type: TextInput::new()
                .value("summarize.text")
                .placeholder("Request type"),
            payload: TextInput::new().placeholder("Request payload"),
            skill_scope_id: TextInput::new()
                .value("33400:npub1agent:summarize-text:0.1.0")
                .placeholder("Skill scope id (optional)"),
            credit_envelope_ref: TextInput::new().placeholder("Credit envelope id (optional)"),
            budget_sats: TextInput::new().value("1500").placeholder("Budget sats"),
            timeout_seconds: TextInput::new().value("60").placeholder("Timeout seconds"),
        }
    }
}

pub struct SettingsPaneInputs {
    pub relay_url: TextInput,
    pub wallet_default_send_sats: TextInput,
    pub provider_max_queue_depth: TextInput,
}

impl Default for SettingsPaneInputs {
    fn default() -> Self {
        Self {
            relay_url: TextInput::new()
                .value("wss://relay.damus.io")
                .placeholder("wss://relay.example.com"),
            wallet_default_send_sats: TextInput::new()
                .value("1000")
                .placeholder("Default send sats"),
            provider_max_queue_depth: TextInput::new()
                .value("4")
                .placeholder("Provider max queue depth"),
        }
    }
}

pub struct JobHistoryPaneInputs {
    pub search_job_id: TextInput,
}

impl Default for JobHistoryPaneInputs {
    fn default() -> Self {
        Self {
            search_job_id: TextInput::new().placeholder("Search job id"),
        }
    }
}

pub struct ChatPaneInputs {
    pub composer: TextInput,
}

impl Default for ChatPaneInputs {
    fn default() -> Self {
        Self {
            composer: TextInput::new().placeholder("Ask Codex to do work..."),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotRole {
    User,
    Codex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutopilotMessageStatus {
    Queued,
    Running,
    Done,
    Error,
}

pub struct AutopilotMessage {
    pub id: u64,
    pub role: AutopilotRole,
    pub status: AutopilotMessageStatus,
    pub content: String,
}

pub struct AutopilotTokenUsage {
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
}

pub struct AutopilotTurnPlanStep {
    pub step: String,
    pub status: String,
}

#[derive(Clone)]
pub struct AutopilotApprovalRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Clone)]
pub struct AutopilotFileChangeApprovalRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    pub grant_root: Option<String>,
}

#[derive(Clone)]
pub struct AutopilotToolCallRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: String,
}

#[derive(Clone)]
pub struct AutopilotToolUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Clone)]
pub struct AutopilotToolUserInputRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<AutopilotToolUserInputQuestion>,
}

#[derive(Clone)]
pub struct AutopilotAuthRefreshRequest {
    pub request_id: codex_client::AppServerRequestId,
    pub reason: String,
    pub previous_account_id: Option<String>,
}

#[derive(Clone)]
pub struct AutopilotThreadMetadata {
    pub thread_name: Option<String>,
    pub status: Option<String>,
    pub loaded: bool,
    pub cwd: Option<String>,
    pub path: Option<String>,
}

pub struct AutopilotThreadResumeTarget {
    pub thread_id: String,
    pub cwd: Option<String>,
    pub path: Option<String>,
}

#[derive(Clone)]
pub struct AutopilotThreadListEntry {
    pub thread_id: String,
    pub thread_name: Option<String>,
    pub status: Option<String>,
    pub loaded: bool,
    pub cwd: Option<String>,
    pub path: Option<String>,
}

pub struct AutopilotChatState {
    pub connection_status: String,
    pub models: Vec<String>,
    pub selected_model: usize,
    pub reasoning_effort: Option<String>,
    pub threads: Vec<String>,
    pub thread_metadata: std::collections::HashMap<String, AutopilotThreadMetadata>,
    pub active_thread_id: Option<String>,
    pub messages: Vec<AutopilotMessage>,
    pub next_message_id: u64,
    pub active_turn_id: Option<String>,
    pub active_assistant_message_id: Option<u64>,
    pub turn_assistant_message_ids: std::collections::HashMap<String, u64>,
    pub last_turn_status: Option<String>,
    pub token_usage: Option<AutopilotTokenUsage>,
    pub turn_plan_explanation: Option<String>,
    pub turn_plan: Vec<AutopilotTurnPlanStep>,
    pub turn_diff: Option<String>,
    pub turn_timeline: Vec<String>,
    pub pending_command_approvals: Vec<AutopilotApprovalRequest>,
    pub pending_file_change_approvals: Vec<AutopilotFileChangeApprovalRequest>,
    pub pending_tool_calls: Vec<AutopilotToolCallRequest>,
    pub pending_tool_user_input: Vec<AutopilotToolUserInputRequest>,
    pub pending_auth_refresh: Vec<AutopilotAuthRefreshRequest>,
    pub auth_refresh_access_token: String,
    pub auth_refresh_account_id: String,
    pub auth_refresh_plan_type: String,
    pub thread_filter_archived: Option<bool>,
    pub thread_filter_sort_key: codex_client::ThreadSortKey,
    pub thread_filter_source_kind: Option<codex_client::ThreadSourceKind>,
    pub thread_filter_model_provider: Option<String>,
    pub thread_filter_search_term: String,
    pub thread_rename_counter: u64,
    pub last_error: Option<String>,
}

impl Default for AutopilotChatState {
    fn default() -> Self {
        Self {
            connection_status: "starting".to_string(),
            // "auto" means "let app-server pick the current default model".
            models: vec!["auto".to_string()],
            selected_model: 0,
            reasoning_effort: Some("medium".to_string()),
            threads: Vec::new(),
            thread_metadata: std::collections::HashMap::new(),
            active_thread_id: None,
            messages: vec![AutopilotMessage {
                id: 1,
                role: AutopilotRole::Codex,
                status: AutopilotMessageStatus::Done,
                content: "Codex lane connecting...".to_string(),
            }],
            next_message_id: 2,
            active_turn_id: None,
            active_assistant_message_id: None,
            turn_assistant_message_ids: std::collections::HashMap::new(),
            last_turn_status: None,
            token_usage: None,
            turn_plan_explanation: None,
            turn_plan: Vec::new(),
            turn_diff: None,
            turn_timeline: Vec::new(),
            pending_command_approvals: Vec::new(),
            pending_file_change_approvals: Vec::new(),
            pending_tool_calls: Vec::new(),
            pending_tool_user_input: Vec::new(),
            pending_auth_refresh: Vec::new(),
            auth_refresh_access_token: std::env::var("OPENAI_ACCESS_TOKEN").unwrap_or_default(),
            auth_refresh_account_id: std::env::var("OPENAI_CHATGPT_ACCOUNT_ID").unwrap_or_default(),
            auth_refresh_plan_type: std::env::var("OPENAI_CHATGPT_PLAN_TYPE").unwrap_or_default(),
            thread_filter_archived: Some(false),
            thread_filter_sort_key: codex_client::ThreadSortKey::UpdatedAt,
            thread_filter_source_kind: None,
            thread_filter_model_provider: None,
            thread_filter_search_term: String::new(),
            thread_rename_counter: 1,
            last_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SidebarState {
    pub width: f32,
    pub is_open: bool,
    pub is_pressed: bool,
    pub is_dragging: bool,
    pub drag_start_x: f32,
    pub drag_start_width: f32,
    pub settings_hover: bool,
    pub settings_tooltip_t: f32,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            width: 300.0,
            is_open: true,
            is_pressed: false,
            is_dragging: false,
            drag_start_x: 0.0,
            drag_start_width: 300.0,
            settings_hover: false,
            settings_tooltip_t: 0.0,
        }
    }
}

impl AutopilotChatState {
    pub fn current_model(&self) -> &str {
        self.models
            .get(self.selected_model)
            .map(String::as_str)
            .unwrap_or("auto")
    }

    pub fn selected_model_override(&self) -> Option<String> {
        let value = self.current_model().trim();
        if value.is_empty() || value.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(value.to_string())
        }
    }

    pub fn set_models(&mut self, models: Vec<String>, default_model: Option<String>) {
        let sanitized = models
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if sanitized.is_empty() {
            return;
        }

        let previous_model = self.models.get(self.selected_model).cloned();
        self.models = sanitized;

        if let Some(default_model) = default_model.as_ref()
            && let Some(index) = self.models.iter().position(|model| model == default_model)
        {
            self.selected_model = index;
            self.last_error = None;
            return;
        }

        if let Some(previous_model) = previous_model.as_ref()
            && let Some(index) = self.models.iter().position(|model| model == previous_model)
        {
            self.selected_model = index;
            self.last_error = None;
            return;
        }

        self.selected_model = 0;
        self.last_error = None;
    }

    pub fn cycle_model(&mut self) {
        if self.models.is_empty() {
            return;
        }
        self.selected_model = (self.selected_model + 1) % self.models.len();
        self.last_error = None;
    }

    pub fn set_connection_status(&mut self, status: impl Into<String>) {
        self.connection_status = status.into();
    }

    pub fn set_thread_entries(&mut self, entries: Vec<AutopilotThreadListEntry>) {
        self.threads = entries
            .iter()
            .map(|entry| entry.thread_id.clone())
            .collect();
        self.thread_metadata.clear();
        for entry in entries {
            self.thread_metadata.insert(
                entry.thread_id.clone(),
                AutopilotThreadMetadata {
                    thread_name: entry.thread_name,
                    status: entry.status,
                    loaded: entry.loaded,
                    cwd: entry.cwd,
                    path: entry.path,
                },
            );
        }
        if let Some(active_id) = self.active_thread_id.as_ref() {
            if !self.threads.iter().any(|thread_id| thread_id == active_id) {
                self.active_thread_id = self.threads.first().cloned();
            }
        } else {
            self.active_thread_id = self.threads.first().cloned();
        }
    }

    pub fn select_thread_by_index(&mut self, index: usize) -> Option<AutopilotThreadResumeTarget> {
        let thread_id = self.threads.get(index).cloned()?;
        self.active_thread_id = Some(thread_id.clone());
        self.last_error = None;
        let metadata = self.thread_metadata.get(&thread_id).cloned();
        Some(AutopilotThreadResumeTarget {
            thread_id,
            cwd: metadata.as_ref().and_then(|value| value.cwd.clone()),
            path: metadata.and_then(|value| value.path),
        })
    }

    pub fn remember_thread(&mut self, thread_id: impl Into<String>) {
        let thread_id = thread_id.into();
        if !self.threads.iter().any(|existing| existing == &thread_id) {
            self.threads.insert(0, thread_id.clone());
        }
        self.thread_metadata
            .entry(thread_id.clone())
            .or_insert_with(|| AutopilotThreadMetadata {
                thread_name: None,
                status: None,
                loaded: false,
                cwd: None,
                path: None,
            });
        if self.active_thread_id.is_none() {
            self.active_thread_id = Some(thread_id);
        }
    }

    pub fn ensure_thread(&mut self, thread_id: String) {
        self.remember_thread(thread_id.clone());
        self.active_thread_id = Some(thread_id);
    }

    pub fn is_active_thread(&self, thread_id: &str) -> bool {
        self.active_thread_id.as_deref() == Some(thread_id)
    }

    pub fn remove_thread(&mut self, thread_id: &str) {
        self.threads.retain(|value| value != thread_id);
        self.thread_metadata.remove(thread_id);
        if self.active_thread_id.as_deref() == Some(thread_id) {
            self.active_thread_id = self.threads.first().cloned();
        }
    }

    pub fn set_active_thread_transcript(
        &mut self,
        thread_id: &str,
        messages: Vec<(AutopilotRole, String)>,
    ) {
        if !self.is_active_thread(thread_id) {
            return;
        }

        self.messages.clear();
        self.next_message_id = 1;
        for (role, content) in messages {
            if content.trim().is_empty() {
                continue;
            }
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role,
                status: AutopilotMessageStatus::Done,
                content,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
        }
        if self.messages.is_empty() {
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::Codex,
                status: AutopilotMessageStatus::Done,
                content: "No transcript available for this thread yet.".to_string(),
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
        }

        self.active_turn_id = None;
        self.active_assistant_message_id = None;
        self.turn_assistant_message_ids.clear();
        self.last_turn_status = None;
        self.token_usage = None;
        self.turn_plan_explanation = None;
        self.turn_plan.clear();
        self.turn_diff = None;
        self.turn_timeline.clear();
        self.last_error = None;
    }

    pub fn submit_prompt(&mut self, prompt: String) {
        self.last_error = None;
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            self.last_error = Some("Prompt cannot be empty".to_string());
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::Codex,
                status: AutopilotMessageStatus::Error,
                content: "Cannot run empty prompt".to_string(),
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
            return;
        }

        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::User,
            status: AutopilotMessageStatus::Done,
            content: trimmed.to_string(),
        });
        self.next_message_id = self.next_message_id.saturating_add(1);

        let assistant_message_id = self.next_message_id;
        self.messages.push(AutopilotMessage {
            id: assistant_message_id,
            role: AutopilotRole::Codex,
            status: AutopilotMessageStatus::Queued,
            content: String::new(),
        });
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.active_assistant_message_id = Some(assistant_message_id);
    }

    pub fn mark_turn_started(&mut self, turn_id: String) {
        self.active_turn_id = Some(turn_id.clone());
        self.last_turn_status = Some("inProgress".to_string());
        if let Some(assistant_message_id) = self.active_assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Running;
            self.turn_assistant_message_ids
                .insert(turn_id, assistant_message_id);
        }
    }

    #[allow(dead_code)]
    pub fn append_turn_delta(&mut self, delta: &str) {
        let Some(turn_id) = self.active_turn_id.clone() else {
            return;
        };
        self.append_turn_delta_for_turn(&turn_id, delta);
    }

    pub fn append_turn_delta_for_turn(&mut self, turn_id: &str, delta: &str) {
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or(self.active_assistant_message_id);
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            if self.active_turn_id.as_deref() == Some(turn_id) {
                message.status = AutopilotMessageStatus::Running;
            }
            message.content.push_str(delta);
        }
    }

    #[allow(dead_code)]
    pub fn mark_turn_completed(&mut self) {
        let Some(turn_id) = self.active_turn_id.clone() else {
            return;
        };
        self.mark_turn_completed_for(&turn_id);
    }

    pub fn mark_turn_completed_for(&mut self, turn_id: &str) {
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or(self.active_assistant_message_id);
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Done;
        }
        self.last_turn_status = Some("completed".to_string());
        if self.active_turn_id.as_deref() == Some(turn_id) {
            self.active_turn_id = None;
            self.active_assistant_message_id = None;
        }
    }

    pub fn mark_turn_error(&mut self, error: impl Into<String>) {
        let error = error.into();
        if let Some(assistant_message_id) = self.active_assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Error;
            if message.content.trim().is_empty() {
                message.content = error.clone();
            }
        }
        self.last_turn_status = Some("failed".to_string());
        self.last_error = Some(error);
        self.active_turn_id = None;
        self.active_assistant_message_id = None;
    }

    pub fn mark_pending_turn_dispatch_failed(&mut self, error: impl Into<String>) {
        let error = error.into();
        if let Some(assistant_message_id) = self.active_assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Error;
            message.content = error.clone();
        }
        self.last_error = Some(error);
        self.last_turn_status = Some("failed".to_string());
        self.active_turn_id = None;
        self.active_assistant_message_id = None;
    }

    pub fn set_turn_status(&mut self, status: Option<String>) {
        self.last_turn_status = status;
    }

    pub fn set_token_usage(
        &mut self,
        input_tokens: i64,
        cached_input_tokens: i64,
        output_tokens: i64,
    ) {
        self.token_usage = Some(AutopilotTokenUsage {
            input_tokens,
            cached_input_tokens,
            output_tokens,
        });
    }

    pub fn set_turn_plan(&mut self, explanation: Option<String>, plan: Vec<AutopilotTurnPlanStep>) {
        self.turn_plan_explanation = explanation;
        self.turn_plan = plan;
    }

    pub fn set_turn_diff(&mut self, diff: Option<String>) {
        self.turn_diff = diff;
    }

    pub fn record_turn_timeline_event(&mut self, event: impl Into<String>) {
        self.turn_timeline.push(event.into());
        if self.turn_timeline.len() > 64 {
            let overflow = self.turn_timeline.len().saturating_sub(64);
            self.turn_timeline.drain(0..overflow);
        }
    }

    pub fn set_thread_loaded_ids(&mut self, loaded_thread_ids: &[String]) {
        let loaded_set: std::collections::HashSet<&str> =
            loaded_thread_ids.iter().map(String::as_str).collect();
        for (thread_id, metadata) in &mut self.thread_metadata {
            metadata.loaded = loaded_set.contains(thread_id.as_str());
        }
    }

    pub fn set_thread_status(&mut self, thread_id: &str, status: Option<String>) {
        if let Some(metadata) = self.thread_metadata.get_mut(thread_id) {
            metadata.status = status;
            return;
        }
        self.thread_metadata.insert(
            thread_id.to_string(),
            AutopilotThreadMetadata {
                thread_name: None,
                status,
                loaded: false,
                cwd: None,
                path: None,
            },
        );
    }

    pub fn set_thread_name(&mut self, thread_id: &str, thread_name: Option<String>) {
        if let Some(metadata) = self.thread_metadata.get_mut(thread_id) {
            metadata.thread_name = thread_name;
            return;
        }
        self.thread_metadata.insert(
            thread_id.to_string(),
            AutopilotThreadMetadata {
                thread_name,
                status: None,
                loaded: false,
                cwd: None,
                path: None,
            },
        );
    }

    pub fn active_thread_status(&self) -> Option<&str> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .and_then(|metadata| metadata.status.as_deref())
    }

    pub fn active_thread_loaded(&self) -> Option<bool> {
        let thread_id = self.active_thread_id.as_ref()?;
        self.thread_metadata
            .get(thread_id)
            .map(|metadata| metadata.loaded)
    }

    pub fn thread_label(&self, thread_id: &str) -> String {
        let short_id = if thread_id.len() > 16 {
            &thread_id[..16]
        } else {
            thread_id
        };
        if let Some(metadata) = self.thread_metadata.get(thread_id)
            && let Some(name) = metadata.thread_name.as_deref()
            && !name.trim().is_empty()
        {
            return format!("{name} [{short_id}]");
        }
        short_id.to_string()
    }

    pub fn cycle_thread_filter_archived(&mut self) {
        self.thread_filter_archived = match self.thread_filter_archived {
            Some(false) => Some(true),
            Some(true) => None,
            None => Some(false),
        };
    }

    pub fn cycle_thread_filter_sort_key(&mut self) {
        self.thread_filter_sort_key = match self.thread_filter_sort_key {
            codex_client::ThreadSortKey::CreatedAt => codex_client::ThreadSortKey::UpdatedAt,
            codex_client::ThreadSortKey::UpdatedAt => codex_client::ThreadSortKey::CreatedAt,
        };
    }

    pub fn cycle_thread_filter_source_kind(&mut self) {
        self.thread_filter_source_kind = match self.thread_filter_source_kind {
            None => Some(codex_client::ThreadSourceKind::AppServer),
            Some(codex_client::ThreadSourceKind::AppServer) => {
                Some(codex_client::ThreadSourceKind::Cli)
            }
            Some(codex_client::ThreadSourceKind::Cli) => Some(codex_client::ThreadSourceKind::Exec),
            Some(codex_client::ThreadSourceKind::Exec) | Some(_) => None,
        };
    }

    pub fn cycle_thread_filter_model_provider(&mut self) {
        self.thread_filter_model_provider = match self.thread_filter_model_provider.as_deref() {
            None => Some("openai".to_string()),
            Some("openai") => Some("azure-openai".to_string()),
            _ => None,
        };
    }

    pub fn build_thread_list_params(&self, cwd: Option<String>) -> codex_client::ThreadListParams {
        codex_client::ThreadListParams {
            cwd,
            cursor: None,
            limit: Some(100),
            sort_key: Some(self.thread_filter_sort_key),
            model_providers: self
                .thread_filter_model_provider
                .as_ref()
                .map(|provider| vec![provider.clone()]),
            source_kinds: self.thread_filter_source_kind.map(|value| vec![value]),
            archived: self.thread_filter_archived,
            search_term: if self.thread_filter_search_term.trim().is_empty() {
                None
            } else {
                Some(self.thread_filter_search_term.trim().to_string())
            },
        }
    }

    pub fn next_thread_name(&mut self) -> String {
        let value = format!("Thread {}", self.thread_rename_counter);
        self.thread_rename_counter = self.thread_rename_counter.saturating_add(1);
        value
    }

    pub fn enqueue_command_approval(&mut self, request: AutopilotApprovalRequest) {
        self.pending_command_approvals.push(request);
    }

    pub fn enqueue_file_change_approval(&mut self, request: AutopilotFileChangeApprovalRequest) {
        self.pending_file_change_approvals.push(request);
    }

    pub fn enqueue_tool_call(&mut self, request: AutopilotToolCallRequest) {
        self.pending_tool_calls.push(request);
    }

    pub fn enqueue_tool_user_input(&mut self, request: AutopilotToolUserInputRequest) {
        self.pending_tool_user_input.push(request);
    }

    pub fn enqueue_auth_refresh(&mut self, request: AutopilotAuthRefreshRequest) {
        self.pending_auth_refresh.push(request);
    }

    pub fn pop_command_approval(&mut self) -> Option<AutopilotApprovalRequest> {
        if self.pending_command_approvals.is_empty() {
            None
        } else {
            Some(self.pending_command_approvals.remove(0))
        }
    }

    pub fn pop_file_change_approval(&mut self) -> Option<AutopilotFileChangeApprovalRequest> {
        if self.pending_file_change_approvals.is_empty() {
            None
        } else {
            Some(self.pending_file_change_approvals.remove(0))
        }
    }

    pub fn pop_tool_call(&mut self) -> Option<AutopilotToolCallRequest> {
        if self.pending_tool_calls.is_empty() {
            None
        } else {
            Some(self.pending_tool_calls.remove(0))
        }
    }

    pub fn pop_tool_user_input(&mut self) -> Option<AutopilotToolUserInputRequest> {
        if self.pending_tool_user_input.is_empty() {
            None
        } else {
            Some(self.pending_tool_user_input.remove(0))
        }
    }

    pub fn pop_auth_refresh(&mut self) -> Option<AutopilotAuthRefreshRequest> {
        if self.pending_auth_refresh.is_empty() {
            None
        } else {
            Some(self.pending_auth_refresh.remove(0))
        }
    }

    pub fn has_pending_messages(&self) -> bool {
        self.messages.iter().any(|message| {
            matches!(
                message.status,
                AutopilotMessageStatus::Queued | AutopilotMessageStatus::Running
            )
        })
    }
}

pub use crate::state::provider_runtime::{ProviderBlocker, ProviderMode, ProviderRuntimeState};
#[allow(unused_imports)]
pub use crate::state::{
    alerts_recovery::{
        AlertDomain, AlertLifecycle, AlertSeverity, AlertsRecoveryState, RecoveryAlertRow,
    },
    job_inbox::{
        JobInboxDecision, JobInboxNetworkRequest, JobInboxRequest, JobInboxState,
        JobInboxValidation,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneLoadState {
    Loading,
    Ready,
    Error,
}

impl PaneLoadState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

pub trait PaneStatusAccess {
    fn pane_load_state_mut(&mut self) -> &mut PaneLoadState;
    fn pane_last_error_mut(&mut self) -> &mut Option<String>;
    fn pane_last_action_mut(&mut self) -> &mut Option<String>;

    fn pane_set_ready(&mut self, action: impl Into<String>) {
        *self.pane_load_state_mut() = PaneLoadState::Ready;
        *self.pane_last_error_mut() = None;
        *self.pane_last_action_mut() = Some(action.into());
    }

    fn pane_set_error(&mut self, error: impl Into<String>) -> String {
        let error = error.into();
        *self.pane_load_state_mut() = PaneLoadState::Error;
        *self.pane_last_error_mut() = Some(error.clone());
        error
    }

    fn pane_clear_error(&mut self) {
        *self.pane_last_error_mut() = None;
    }
}

macro_rules! impl_pane_status_access {
    ($($state:ty),+ $(,)?) => {
        $(
            impl PaneStatusAccess for $state {
                fn pane_load_state_mut(&mut self) -> &mut PaneLoadState {
                    &mut self.load_state
                }

                fn pane_last_error_mut(&mut self) -> &mut Option<String> {
                    &mut self.last_error
                }

                fn pane_last_action_mut(&mut self) -> &mut Option<String> {
                    &mut self.last_action
                }
            }
        )+
    };
}

#[allow(unused_imports)]
pub use crate::state::operations::{
    NetworkRequestStatus, NetworkRequestSubmission, NetworkRequestsState, RelayConnectionRow,
    RelayConnectionStatus, RelayConnectionsState, StarterJobRow, StarterJobStatus,
    StarterJobsState, SubmittedNetworkRequest, SyncHealthState, SyncRecoveryPhase,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityEventDomain {
    Chat,
    Job,
    Wallet,
    Network,
    Sync,
    Sa,
    Skl,
    Ac,
}

impl ActivityEventDomain {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Job => "job",
            Self::Wallet => "wallet",
            Self::Network => "network",
            Self::Sync => "sync",
            Self::Sa => "sa",
            Self::Skl => "skl",
            Self::Ac => "ac",
        }
    }

    pub const fn source_tag(self) -> &'static str {
        match self {
            Self::Chat => "chat.lane",
            Self::Job => "provider.runtime",
            Self::Wallet => "spark.wallet",
            Self::Network => "nip90.network",
            Self::Sync => "spacetime.sync",
            Self::Sa => "nostr.sa",
            Self::Skl => "nostr.skl",
            Self::Ac => "nostr.ac",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivityFeedFilter {
    All,
    Chat,
    Job,
    Wallet,
    Network,
    Sync,
    Sa,
    Skl,
    Ac,
}

impl ActivityFeedFilter {
    pub const fn all() -> [Self; 9] {
        [
            Self::All,
            Self::Chat,
            Self::Job,
            Self::Wallet,
            Self::Network,
            Self::Sync,
            Self::Sa,
            Self::Skl,
            Self::Ac,
        ]
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Chat => "chat",
            Self::Job => "job",
            Self::Wallet => "wallet",
            Self::Network => "network",
            Self::Sync => "sync",
            Self::Sa => "sa",
            Self::Skl => "skl",
            Self::Ac => "ac",
        }
    }

    pub fn matches(self, domain: ActivityEventDomain) -> bool {
        match self {
            Self::All => true,
            Self::Chat => domain == ActivityEventDomain::Chat,
            Self::Job => domain == ActivityEventDomain::Job,
            Self::Wallet => domain == ActivityEventDomain::Wallet,
            Self::Network => domain == ActivityEventDomain::Network,
            Self::Sync => domain == ActivityEventDomain::Sync,
            Self::Sa => domain == ActivityEventDomain::Sa,
            Self::Skl => domain == ActivityEventDomain::Skl,
            Self::Ac => domain == ActivityEventDomain::Ac,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivityEventRow {
    pub event_id: String,
    pub domain: ActivityEventDomain,
    pub source_tag: String,
    pub occurred_at_epoch_seconds: u64,
    pub summary: String,
    pub detail: String,
}

pub struct ActivityFeedState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub active_filter: ActivityFeedFilter,
    pub rows: Vec<ActivityEventRow>,
    pub selected_event_id: Option<String>,
}

impl Default for ActivityFeedState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for activity feed lane snapshot".to_string()),
            active_filter: ActivityFeedFilter::All,
            rows: Vec::new(),
            selected_event_id: None,
        }
    }
}

impl ActivityFeedState {
    pub fn visible_rows(&self) -> Vec<&ActivityEventRow> {
        self.rows
            .iter()
            .filter(|row| self.active_filter.matches(row.domain))
            .collect()
    }

    pub fn selected(&self) -> Option<&ActivityEventRow> {
        let selected = self.selected_event_id.as_deref()?;
        self.rows.iter().find(|row| row.event_id == selected)
    }

    pub fn select_visible_row(&mut self, index: usize) -> bool {
        let Some(event_id) = self
            .visible_rows()
            .get(index)
            .map(|row| row.event_id.clone())
        else {
            return false;
        };
        self.selected_event_id = Some(event_id);
        self.pane_clear_error();
        true
    }

    pub fn set_filter(&mut self, filter: ActivityFeedFilter) {
        self.active_filter = filter;
        if self
            .selected()
            .is_none_or(|row| !filter.matches(row.domain))
        {
            self.selected_event_id = self.visible_rows().first().map(|row| row.event_id.clone());
        }
        self.pane_set_ready(format!("Activity filter -> {}", filter.label()));
    }

    pub fn upsert_event(&mut self, row: ActivityEventRow) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.event_id == row.event_id)
        {
            *existing = row;
        } else {
            self.rows.push(row);
        }

        self.rows.sort_by(|lhs, rhs| {
            rhs.occurred_at_epoch_seconds
                .cmp(&lhs.occurred_at_epoch_seconds)
                .then_with(|| lhs.event_id.cmp(&rhs.event_id))
        });
        self.rows.truncate(96);
    }

    pub fn record_refresh(&mut self, rows: Vec<ActivityEventRow>) {
        for row in rows {
            self.upsert_event(row);
        }

        if self.selected().is_none() {
            self.selected_event_id = self.visible_rows().first().map(|row| row.event_id.clone());
        }

        self.pane_set_ready(format!(
            "Activity feed refreshed ({} events)",
            self.rows.len()
        ));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsDocumentV1 {
    pub schema_version: u16,
    pub relay_url: String,
    pub identity_path: String,
    pub wallet_default_send_sats: u64,
    pub provider_max_queue_depth: u32,
    pub reconnect_required: bool,
}

impl Default for SettingsDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: 1,
            relay_url: "wss://relay.damus.io".to_string(),
            identity_path: settings_identity_path(),
            wallet_default_send_sats: 1000,
            provider_max_queue_depth: 4,
            reconnect_required: false,
        }
    }
}

pub struct SettingsState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub document: SettingsDocumentV1,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Settings loaded from migration-safe defaults".to_string()),
            document: SettingsDocumentV1::default(),
        }
    }
}

impl SettingsState {
    pub fn load_from_disk() -> Self {
        let path = settings_file_path();
        let mut state = Self::default();
        match std::fs::read_to_string(&path) {
            Ok(raw) => match parse_settings_document(&raw) {
                Ok(document) => {
                    state.document = document;
                    state.pane_set_ready(format!("Settings loaded from {}", path.display()));
                }
                Err(error) => {
                    let _ = state.pane_set_error(format!("Settings parse error: {error}"));
                    *state.pane_last_action_mut() =
                        Some("Using migration-safe defaults".to_string());
                }
            },
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    let _ = state.pane_set_error(format!("Settings read error: {error}"));
                }
            }
        }

        state
    }

    pub fn apply_updates(
        &mut self,
        relay_url: &str,
        wallet_default_send_sats: &str,
        provider_max_queue_depth: &str,
    ) -> Result<(), String> {
        self.apply_updates_internal(
            relay_url,
            wallet_default_send_sats,
            provider_max_queue_depth,
            true,
        )
    }

    fn apply_updates_internal(
        &mut self,
        relay_url: &str,
        wallet_default_send_sats: &str,
        provider_max_queue_depth: &str,
        persist: bool,
    ) -> Result<(), String> {
        let relay_url = relay_url.trim();
        if relay_url.is_empty() {
            return Err(self.pane_set_error("Relay URL is required"));
        }
        if !relay_url.starts_with("wss://") {
            return Err(self.pane_set_error("Relay URL must start with wss://"));
        }

        let wallet_default_send_sats = wallet_default_send_sats
            .trim()
            .parse::<u64>()
            .map_err(|error| format!("Wallet default send sats must be an integer: {error}"))?;
        if wallet_default_send_sats == 0 || wallet_default_send_sats > 10_000_000 {
            return Err(
                self.pane_set_error("Wallet default send sats must be between 1 and 10,000,000")
            );
        }

        let provider_max_queue_depth = provider_max_queue_depth
            .trim()
            .parse::<u32>()
            .map_err(|error| format!("Provider max queue depth must be an integer: {error}"))?;
        if provider_max_queue_depth == 0 || provider_max_queue_depth > 512 {
            return Err(self.pane_set_error("Provider max queue depth must be between 1 and 512"));
        }

        let reconnect_required = relay_url != self.document.relay_url
            || provider_max_queue_depth != self.document.provider_max_queue_depth;
        self.document.relay_url = relay_url.to_string();
        self.document.wallet_default_send_sats = wallet_default_send_sats;
        self.document.provider_max_queue_depth = provider_max_queue_depth;
        self.document.reconnect_required = reconnect_required;

        if persist {
            self.persist_to_disk()?;
        }

        self.pane_set_ready(if reconnect_required {
            "Saved settings. Relay/provider changes require reconnect.".to_string()
        } else {
            "Saved settings.".to_string()
        });
        Ok(())
    }

    pub fn reset_defaults(&mut self) -> Result<(), String> {
        self.reset_defaults_internal(true)
    }

    fn reset_defaults_internal(&mut self, persist: bool) -> Result<(), String> {
        self.document = SettingsDocumentV1::default();
        if persist {
            self.persist_to_disk()?;
        }
        self.pane_set_ready("Reset settings to schema defaults.");
        Ok(())
    }

    fn persist_to_disk(&mut self) -> Result<(), String> {
        let path = settings_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create settings dir: {error}"))?;
        }
        std::fs::write(&path, serialize_settings_document(&self.document))
            .map_err(|error| format!("Failed to persist settings: {error}"))?;
        Ok(())
    }
}

impl SettingsPaneInputs {
    pub fn from_state(settings: &SettingsState) -> Self {
        Self {
            relay_url: TextInput::new()
                .value(settings.document.relay_url.clone())
                .placeholder("wss://relay.example.com"),
            wallet_default_send_sats: TextInput::new()
                .value(settings.document.wallet_default_send_sats.to_string())
                .placeholder("Default send sats"),
            provider_max_queue_depth: TextInput::new()
                .value(settings.document.provider_max_queue_depth.to_string())
                .placeholder("Provider max queue depth"),
        }
    }

    pub fn sync_from_state(&mut self, settings: &SettingsState) {
        self.relay_url
            .set_value(settings.document.relay_url.clone());
        self.wallet_default_send_sats
            .set_value(settings.document.wallet_default_send_sats.to_string());
        self.provider_max_queue_depth
            .set_value(settings.document.provider_max_queue_depth.to_string());
    }
}

fn settings_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-settings-v1.conf")
}

fn settings_identity_path() -> String {
    nostr::identity_mnemonic_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "~/.openagents/pylon/identity.mnemonic".to_string())
}

fn serialize_settings_document(document: &SettingsDocumentV1) -> String {
    format!(
        "schema_version={}\nrelay_url={}\nidentity_path={}\nwallet_default_send_sats={}\nprovider_max_queue_depth={}\nreconnect_required={}\n",
        document.schema_version,
        document.relay_url,
        document.identity_path,
        document.wallet_default_send_sats,
        document.provider_max_queue_depth,
        document.reconnect_required,
    )
}

fn parse_settings_document(raw: &str) -> Result<SettingsDocumentV1, String> {
    let mut document = SettingsDocumentV1::default();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(format!("Invalid settings line: {trimmed}"));
        };
        match key.trim() {
            "schema_version" => {
                document.schema_version = value
                    .trim()
                    .parse::<u16>()
                    .map_err(|error| format!("Invalid schema version: {error}"))?;
            }
            "relay_url" => document.relay_url = value.trim().to_string(),
            "identity_path" => document.identity_path = value.trim().to_string(),
            "wallet_default_send_sats" => {
                document.wallet_default_send_sats = value
                    .trim()
                    .parse::<u64>()
                    .map_err(|error| format!("Invalid wallet_default_send_sats: {error}"))?;
            }
            "provider_max_queue_depth" => {
                document.provider_max_queue_depth = value
                    .trim()
                    .parse::<u32>()
                    .map_err(|error| format!("Invalid provider_max_queue_depth: {error}"))?;
            }
            "reconnect_required" => {
                document.reconnect_required = value
                    .trim()
                    .parse::<bool>()
                    .map_err(|error| format!("Invalid reconnect_required: {error}"))?;
            }
            _ => {}
        }
    }

    if document.schema_version != 1 {
        return Err(format!(
            "Unsupported schema version {}, expected 1",
            document.schema_version
        ));
    }

    // Identity path authority is the resolved mnemonic path.
    document.identity_path = settings_identity_path();

    Ok(document)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobLifecycleStage {
    Received,
    Accepted,
    Running,
    Delivered,
    Paid,
    Failed,
}

impl JobLifecycleStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Delivered => "delivered",
            Self::Paid => "paid",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveJobEvent {
    pub seq: u64,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveJobRecord {
    pub job_id: String,
    pub request_id: String,
    pub requester: String,
    pub capability: String,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub sa_trajectory_session_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub ac_settlement_event_id: Option<String>,
    pub ac_default_event_id: Option<String>,
    pub quoted_price_sats: u64,
    pub stage: JobLifecycleStage,
    pub invoice_id: Option<String>,
    pub payment_id: Option<String>,
    pub failure_reason: Option<String>,
    pub events: Vec<ActiveJobEvent>,
}

pub struct ActiveJobState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub runtime_supports_abort: bool,
    pub job: Option<ActiveJobRecord>,
    next_event_seq: u64,
}

impl Default for ActiveJobState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for active job lane snapshot".to_string()),
            runtime_supports_abort: false,
            job: None,
            next_event_seq: 1,
        }
    }
}

impl ActiveJobState {
    pub fn start_from_request(&mut self, request: &JobInboxRequest) {
        let job_id = format!("job-{}", request.request_id);
        self.job = Some(ActiveJobRecord {
            job_id,
            request_id: request.request_id.clone(),
            requester: request.requester.clone(),
            capability: request.capability.clone(),
            skill_scope_id: request.skill_scope_id.clone(),
            skl_manifest_a: request.skl_manifest_a.clone(),
            skl_manifest_event_id: request.skl_manifest_event_id.clone(),
            sa_tick_request_event_id: request.sa_tick_request_event_id.clone(),
            sa_tick_result_event_id: request.sa_tick_result_event_id.clone(),
            sa_trajectory_session_id: Some(format!("traj:{}", request.request_id)),
            ac_envelope_event_id: request.ac_envelope_event_id.clone(),
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            quoted_price_sats: request.price_sats,
            stage: JobLifecycleStage::Accepted,
            invoice_id: None,
            payment_id: None,
            failure_reason: None,
            events: Vec::new(),
        });
        self.next_event_seq = 1;
        self.append_event("received request from inbox");
        self.append_event("accepted request and queued runtime execution");
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!("Selected {} as active job", request.request_id));
    }

    pub fn append_event(&mut self, message: impl Into<String>) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        job.events.push(ActiveJobEvent {
            seq: self.next_event_seq,
            message: message.into(),
        });
        self.next_event_seq = self.next_event_seq.saturating_add(1);
    }

    pub fn advance_stage(&mut self) -> Result<JobLifecycleStage, String> {
        let Some(job) = self.job.as_mut() else {
            self.last_error = Some("No active job selected".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("No active job selected".to_string());
        };

        let next_stage = match job.stage {
            JobLifecycleStage::Received => JobLifecycleStage::Accepted,
            JobLifecycleStage::Accepted => JobLifecycleStage::Running,
            JobLifecycleStage::Running => JobLifecycleStage::Delivered,
            JobLifecycleStage::Delivered => JobLifecycleStage::Paid,
            JobLifecycleStage::Paid | JobLifecycleStage::Failed => {
                self.last_error = Some("Active job already terminal".to_string());
                self.load_state = PaneLoadState::Error;
                return Err("Active job already terminal".to_string());
            }
        };

        job.stage = next_stage;
        if next_stage == JobLifecycleStage::Delivered {
            job.invoice_id = Some(format!("inv-{}", job.request_id));
            if job.sa_tick_result_event_id.is_none() {
                job.sa_tick_result_event_id = Some(format!("sa:39211:{}", job.request_id));
            }
        }
        if next_stage == JobLifecycleStage::Paid {
            job.payment_id = Some(format!("pay-{}", job.request_id));
            if job.ac_settlement_event_id.is_none() {
                job.ac_settlement_event_id = Some(format!("ac:39244:{}", job.request_id));
            }
        }
        self.append_event(format!("stage advanced to {}", next_stage.label()));
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Advanced active job to {}", next_stage.label()));
        Ok(next_stage)
    }

    pub fn abort_job(&mut self, reason: &str) -> Result<(), String> {
        if !self.runtime_supports_abort {
            self.last_error =
                Some("Abort unavailable: runtime lane does not support cancel".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("Abort unavailable".to_string());
        }
        let Some(job) = self.job.as_mut() else {
            self.last_error = Some("No active job selected".to_string());
            self.load_state = PaneLoadState::Error;
            return Err("No active job selected".to_string());
        };

        let reason_text = reason.trim().to_string();
        job.stage = JobLifecycleStage::Failed;
        job.failure_reason = Some(reason_text.clone());
        if job.ac_default_event_id.is_none() {
            job.ac_default_event_id = Some(format!("ac:39245:{}", job.request_id));
        }
        self.append_event(format!("job aborted: {reason_text}"));
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some("Aborted active job".to_string());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryStatus {
    Succeeded,
    Failed,
}

impl JobHistoryStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryStatusFilter {
    All,
    Succeeded,
    Failed,
}

impl JobHistoryStatusFilter {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::All => Self::Succeeded,
            Self::Succeeded => Self::Failed,
            Self::Failed => Self::All,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobHistoryTimeRange {
    All,
    Last24h,
    Last7d,
}

impl JobHistoryTimeRange {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all-time",
            Self::Last24h => "24h",
            Self::Last7d => "7d",
        }
    }

    pub const fn max_age_seconds(self) -> Option<u64> {
        match self {
            Self::All => None,
            Self::Last24h => Some(86_400),
            Self::Last7d => Some(604_800),
        }
    }

    pub const fn cycle(self) -> Self {
        match self {
            Self::All => Self::Last24h,
            Self::Last24h => Self::Last7d,
            Self::Last7d => Self::All,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobHistoryReceiptRow {
    pub job_id: String,
    pub status: JobHistoryStatus,
    pub completed_at_epoch_seconds: u64,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub sa_trajectory_session_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub ac_settlement_event_id: Option<String>,
    pub ac_default_event_id: Option<String>,
    pub payout_sats: u64,
    pub result_hash: String,
    pub payment_pointer: String,
    pub failure_reason: Option<String>,
}

pub struct JobHistoryState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub rows: Vec<JobHistoryReceiptRow>,
    pub status_filter: JobHistoryStatusFilter,
    pub time_range: JobHistoryTimeRange,
    pub page: usize,
    pub page_size: usize,
    pub search_job_id: String,
    pub reference_epoch_seconds: u64,
}

impl Default for JobHistoryState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for receipt lane snapshot".to_string()),
            rows: Vec::new(),
            status_filter: JobHistoryStatusFilter::All,
            time_range: JobHistoryTimeRange::All,
            page: 0,
            page_size: 6,
            search_job_id: String::new(),
            reference_epoch_seconds: 1_761_920_000,
        }
    }
}

impl JobHistoryState {
    pub fn set_search_job_id(&mut self, value: String) {
        self.search_job_id = value;
        self.page = 0;
    }

    pub fn cycle_status_filter(&mut self) {
        self.status_filter = self.status_filter.cycle();
        self.page = 0;
        self.last_error = None;
        self.last_action = Some(format!("Status filter -> {}", self.status_filter.label()));
    }

    pub fn cycle_time_range(&mut self) {
        self.time_range = self.time_range.cycle();
        self.page = 0;
        self.last_error = None;
        self.last_action = Some(format!("Time range -> {}", self.time_range.label()));
    }

    pub fn previous_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
        }
    }

    pub fn next_page(&mut self) {
        let pages = self.total_pages();
        if self.page + 1 < pages {
            self.page += 1;
        }
    }

    pub fn total_pages(&self) -> usize {
        let filtered = self.filtered_rows();
        ((filtered.len() + self.page_size.saturating_sub(1)) / self.page_size.max(1)).max(1)
    }

    pub fn paged_rows(&self) -> Vec<&JobHistoryReceiptRow> {
        let filtered = self.filtered_rows();
        let page = self.page.min(self.total_pages().saturating_sub(1));
        let start = page.saturating_mul(self.page_size.max(1));
        let end = (start + self.page_size.max(1)).min(filtered.len());
        filtered[start..end].to_vec()
    }

    pub fn upsert_row(&mut self, row: JobHistoryReceiptRow) {
        if let Some(existing) = self
            .rows
            .iter_mut()
            .find(|existing| existing.job_id == row.job_id)
        {
            *existing = row;
        } else {
            self.rows.push(row);
        }
        self.rows.sort_by(|lhs, rhs| {
            rhs.completed_at_epoch_seconds
                .cmp(&lhs.completed_at_epoch_seconds)
                .then_with(|| lhs.job_id.cmp(&rhs.job_id))
        });
    }

    pub fn record_from_active_job(&mut self, job: &ActiveJobRecord, status: JobHistoryStatus) {
        let completed = self
            .reference_epoch_seconds
            .saturating_add(self.rows.len() as u64 * 17);
        self.upsert_row(JobHistoryReceiptRow {
            job_id: job.job_id.clone(),
            status,
            completed_at_epoch_seconds: completed,
            skill_scope_id: job.skill_scope_id.clone(),
            skl_manifest_a: job.skl_manifest_a.clone(),
            skl_manifest_event_id: job.skl_manifest_event_id.clone(),
            sa_tick_result_event_id: job.sa_tick_result_event_id.clone(),
            sa_trajectory_session_id: job.sa_trajectory_session_id.clone(),
            ac_envelope_event_id: job.ac_envelope_event_id.clone(),
            ac_settlement_event_id: job.ac_settlement_event_id.clone(),
            ac_default_event_id: job.ac_default_event_id.clone(),
            payout_sats: if status == JobHistoryStatus::Succeeded {
                job.quoted_price_sats
            } else {
                0
            },
            result_hash: format!("sha256:{}-{}", job.request_id, job.stage.label()),
            payment_pointer: job
                .payment_id
                .clone()
                .or_else(|| job.invoice_id.clone())
                .unwrap_or_else(|| format!("pending:{}", job.request_id)),
            failure_reason: job.failure_reason.clone(),
        });
        self.page = 0;
        self.last_error = None;
        self.load_state = PaneLoadState::Ready;
        self.last_action = Some(format!("Recorded history receipt for {}", job.job_id));
    }

    fn filtered_rows(&self) -> Vec<&JobHistoryReceiptRow> {
        let search = self.search_job_id.trim().to_lowercase();
        self.rows
            .iter()
            .filter(|row| match self.status_filter {
                JobHistoryStatusFilter::All => true,
                JobHistoryStatusFilter::Succeeded => row.status == JobHistoryStatus::Succeeded,
                JobHistoryStatusFilter::Failed => row.status == JobHistoryStatus::Failed,
            })
            .filter(|row| {
                if let Some(max_age) = self.time_range.max_age_seconds() {
                    let age = self
                        .reference_epoch_seconds
                        .saturating_sub(row.completed_at_epoch_seconds);
                    age <= max_age
                } else {
                    true
                }
            })
            .filter(|row| {
                if search.is_empty() {
                    true
                } else {
                    row.job_id.to_lowercase().contains(&search)
                }
            })
            .collect()
    }
}

pub struct CodexAccountPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub account_summary: String,
    pub requires_openai_auth: bool,
    pub auth_mode: Option<String>,
    pub pending_login_id: Option<String>,
    pub pending_login_url: Option<String>,
    pub rate_limits_summary: Option<String>,
}

impl Default for CodexAccountPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for account/read".to_string()),
            account_summary: "unknown".to_string(),
            requires_openai_auth: true,
            auth_mode: None,
            pending_login_id: None,
            pending_login_url: None,
            rate_limits_summary: None,
        }
    }
}

pub struct CodexModelCatalogEntryState {
    pub model: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub is_default: bool,
    pub default_reasoning_effort: String,
    pub supported_reasoning_efforts: Vec<String>,
}

pub struct CodexModelsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub include_hidden: bool,
    pub entries: Vec<CodexModelCatalogEntryState>,
    pub last_reroute: Option<String>,
}

impl Default for CodexModelsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for model/list".to_string()),
            include_hidden: false,
            entries: Vec::new(),
            last_reroute: None,
        }
    }
}

pub struct CodexConfigPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub config_json: String,
    pub requirements_json: String,
    pub detected_external_configs: usize,
}

impl Default for CodexConfigPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for config/read".to_string()),
            config_json: "{}".to_string(),
            requirements_json: "null".to_string(),
            detected_external_configs: 0,
        }
    }
}

pub struct CodexMcpServerEntryState {
    pub name: String,
    pub auth_status: String,
    pub tool_count: usize,
    pub resource_count: usize,
    pub template_count: usize,
}

pub struct CodexMcpPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub servers: Vec<CodexMcpServerEntryState>,
    pub selected_server_index: Option<usize>,
    pub last_oauth_url: Option<String>,
    pub last_oauth_result: Option<String>,
    pub next_cursor: Option<String>,
}

impl Default for CodexMcpPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for mcpServerStatus/list".to_string()),
            servers: Vec::new(),
            selected_server_index: None,
            last_oauth_url: None,
            last_oauth_result: None,
            next_cursor: None,
        }
    }
}

pub struct CodexAppEntryState {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_accessible: bool,
    pub is_enabled: bool,
}

pub struct CodexAppsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub apps: Vec<CodexAppEntryState>,
    pub selected_app_index: Option<usize>,
    pub next_cursor: Option<String>,
    pub update_count: u64,
}

impl Default for CodexAppsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for app/list".to_string()),
            apps: Vec::new(),
            selected_app_index: None,
            next_cursor: None,
            update_count: 0,
        }
    }
}

pub struct CodexRemoteSkillEntryState {
    pub id: String,
    pub name: String,
    pub description: String,
}

pub struct CodexRemoteSkillsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub skills: Vec<CodexRemoteSkillEntryState>,
    pub selected_skill_index: Option<usize>,
    pub last_exported_path: Option<String>,
}

impl Default for CodexRemoteSkillsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for skills/remote/list".to_string()),
            skills: Vec::new(),
            selected_skill_index: None,
            last_exported_path: None,
        }
    }
}

pub struct CodexLabsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub review_last_turn_id: Option<String>,
    pub review_last_thread_id: Option<String>,
    pub command_last_exit_code: Option<i32>,
    pub command_last_stdout: String,
    pub command_last_stderr: String,
    pub collaboration_modes_json: String,
    pub experimental_features_json: String,
    pub experimental_enabled: bool,
    pub realtime_started: bool,
    pub fuzzy_session_id: String,
    pub fuzzy_last_status: String,
    pub windows_last_status: Option<String>,
}

impl Default for CodexLabsPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Codex Labs ready".to_string()),
            review_last_turn_id: None,
            review_last_thread_id: None,
            command_last_exit_code: None,
            command_last_stdout: String::new(),
            command_last_stderr: String::new(),
            collaboration_modes_json: "[]".to_string(),
            experimental_features_json: "[]".to_string(),
            experimental_enabled: false,
            realtime_started: false,
            fuzzy_session_id: format!("labs-{}", std::process::id()),
            fuzzy_last_status: "idle".to_string(),
            windows_last_status: None,
        }
    }
}

pub struct CodexDiagnosticsMethodCountState {
    pub method: String,
    pub count: u64,
}

pub struct CodexDiagnosticsPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub notification_counts: Vec<CodexDiagnosticsMethodCountState>,
    pub server_request_counts: Vec<CodexDiagnosticsMethodCountState>,
    pub raw_events: Vec<String>,
    pub last_command_failure: Option<String>,
    pub last_snapshot_error: Option<String>,
    pub wire_log_path: String,
    pub wire_log_enabled: bool,
}

impl Default for CodexDiagnosticsPaneState {
    fn default() -> Self {
        let env_wire_log_path = std::env::var("OPENAGENTS_CODEX_WIRE_LOG_PATH").ok();
        let wire_log_path = env_wire_log_path
            .clone()
            .unwrap_or_else(|| "/tmp/openagents-codex-wire.log".to_string());
        Self {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: Some("Codex diagnostics idle".to_string()),
            notification_counts: Vec::new(),
            server_request_counts: Vec::new(),
            raw_events: Vec::new(),
            last_command_failure: None,
            last_snapshot_error: None,
            wire_log_path: wire_log_path.clone(),
            wire_log_enabled: env_wire_log_path.is_some(),
        }
    }
}

pub struct AgentProfileStatePaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub profile_name: String,
    pub profile_about: String,
    pub goals_summary: String,
    pub profile_event_id: Option<String>,
    pub state_event_id: Option<String>,
    pub goals_event_id: Option<String>,
}

impl Default for AgentProfileStatePaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA profile/state snapshot".to_string()),
            profile_name: "Autopilot".to_string(),
            profile_about: "Desktop sovereign agent runtime".to_string(),
            goals_summary: "Earn sats and complete queued jobs".to_string(),
            profile_event_id: None,
            state_event_id: None,
            goals_event_id: None,
        }
    }
}

pub struct AgentScheduleTickPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub heartbeat_seconds: u64,
    pub next_tick_reason: String,
    pub last_tick_outcome: String,
    pub schedule_event_id: Option<String>,
    pub tick_request_event_id: Option<String>,
    pub tick_result_event_id: Option<String>,
}

impl Default for AgentScheduleTickPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SA schedule/tick snapshot".to_string()),
            heartbeat_seconds: 30,
            next_tick_reason: "manual.operator".to_string(),
            last_tick_outcome: "n/a".to_string(),
            schedule_event_id: None,
            tick_request_event_id: None,
            tick_result_event_id: None,
        }
    }
}

pub struct TrajectoryAuditPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub active_session_id: Option<String>,
    pub verified_hash: Option<String>,
    pub step_filter: String,
}

impl Default for TrajectoryAuditPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for trajectory session stream".to_string()),
            active_session_id: None,
            verified_hash: None,
            step_filter: "all".to_string(),
        }
    }
}

pub struct SkillRegistryPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub search_query: String,
    pub manifest_slug: String,
    pub manifest_version: String,
    pub manifest_a: Option<String>,
    pub manifest_event_id: Option<String>,
    pub version_event_id: Option<String>,
    pub search_result_event_id: Option<String>,
    pub source: String,
    pub repo_skills_root: Option<String>,
    pub discovered_skills: Vec<SkillRegistryDiscoveredSkill>,
    pub discovery_errors: Vec<String>,
    pub selected_skill_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillRegistryDiscoveredSkill {
    pub name: String,
    pub path: String,
    pub scope: String,
    pub enabled: bool,
    pub interface_display_name: Option<String>,
    pub dependency_count: usize,
}

impl Default for SkillRegistryPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL registry snapshot".to_string()),
            search_query: "summarize".to_string(),
            manifest_slug: "summarize-text".to_string(),
            manifest_version: "0.1.0".to_string(),
            manifest_a: None,
            manifest_event_id: None,
            version_event_id: None,
            search_result_event_id: None,
            source: "codex".to_string(),
            repo_skills_root: None,
            discovered_skills: Vec::new(),
            discovery_errors: Vec::new(),
            selected_skill_index: None,
        }
    }
}

pub struct SkillTrustRevocationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub trust_tier: String,
    pub manifest_a: Option<String>,
    pub attestation_count: u32,
    pub kill_switch_active: bool,
    pub revocation_event_id: Option<String>,
}

impl Default for SkillTrustRevocationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for SKL trust gate snapshot".to_string()),
            trust_tier: "unknown".to_string(),
            manifest_a: None,
            attestation_count: 0,
            kill_switch_active: false,
            revocation_event_id: None,
        }
    }
}

pub struct CreditDeskPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub scope: String,
    pub requested_sats: u64,
    pub offered_sats: u64,
    pub envelope_cap_sats: u64,
    pub spend_sats: u64,
    pub spend_job_id: String,
    pub intent_event_id: Option<String>,
    pub offer_event_id: Option<String>,
    pub envelope_event_id: Option<String>,
    pub spend_event_id: Option<String>,
}

impl Default for CreditDeskPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC credit desk snapshot".to_string()),
            scope: "skill:33400:npub1agent:summarize-text:0.1.0:constraints".to_string(),
            requested_sats: 1500,
            offered_sats: 1400,
            envelope_cap_sats: 1200,
            spend_sats: 600,
            spend_job_id: "job-credit-001".to_string(),
            intent_event_id: None,
            offer_event_id: None,
            envelope_event_id: None,
            spend_event_id: None,
        }
    }
}

pub struct CreditSettlementLedgerPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub result_event_id: String,
    pub payment_pointer: String,
    pub default_reason: String,
    pub settlement_event_id: Option<String>,
    pub default_event_id: Option<String>,
}

impl Default for CreditSettlementLedgerPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for AC settlement ledger snapshot".to_string()),
            result_event_id: "nip90:result:pending".to_string(),
            payment_pointer: "pay:pending".to_string(),
            default_reason: "settlement timeout".to_string(),
            settlement_event_id: None,
            default_event_id: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentNetworkSimulationEvent {
    pub seq: u64,
    pub protocol: String,
    pub event_ref: String,
    pub summary: String,
}

pub struct AgentNetworkSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub channel_name: String,
    pub channel_event_id: Option<String>,
    pub rounds_run: u32,
    pub total_transferred_sats: u64,
    pub learned_skills: Vec<String>,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
}

impl Default for AgentNetworkSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Run simulation to create NIP-28 coordination channel".to_string()),
            channel_name: "sovereign-agents-lab".to_string(),
            channel_event_id: None,
            rounds_run: 0,
            total_transferred_sats: 0,
            learned_skills: Vec::new(),
            events: Vec::new(),
            next_seq: 1,
        }
    }
}

impl AgentNetworkSimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let channel_event_id = match self.channel_event_id.clone() {
            Some(existing) => existing,
            None => {
                let metadata = nostr::ChannelMetadata::new(
                    self.channel_name.clone(),
                    "Public SA/SKL/AC negotiation channel",
                    "https://openagents.com/channel.png",
                )
                .with_relays(vec!["wss://relay.openagents.dev".to_string()]);
                let creation = nostr::ChannelCreateEvent::new(metadata, now_epoch_seconds);
                if let Err(error) = creation.content() {
                    self.load_state = PaneLoadState::Error;
                    self.last_error = Some(error.to_string());
                    return Err(error.to_string());
                }
                let id = format!(
                    "sim:{}:{:08x}",
                    nostr::KIND_CHANNEL_CREATION,
                    self.rounds_run + 1
                );
                self.channel_event_id = Some(id.clone());
                self.push_event(
                    "NIP-28",
                    &id,
                    format!(
                        "created channel #{} ({})",
                        self.channel_name, creation.created_at
                    ),
                );
                id
            }
        };

        let round = self.rounds_run.saturating_add(1);
        let relay = "wss://relay.openagents.dev";
        let skill_version = format!("0.{}.0", round + 1);
        let skill_ref = format!("33400:npub1beta:summarize-text:{skill_version}");
        let base = u64::from(round) * 10;

        let announce = nostr::ChannelMessageEvent::new(
            channel_event_id.clone(),
            relay,
            format!(
                "agent-alpha requests summarize-text@{skill_version} for client brief #{round}"
            ),
            now_epoch_seconds.saturating_add(base + 1),
        );
        let announce_id = format!("sim:{}:{:08x}:a", nostr::KIND_CHANNEL_MESSAGE, round);
        self.push_event(
            "NIP-28",
            &announce_id,
            format!(
                "alpha broadcast in channel ({} tags)",
                announce.to_tags().len()
            ),
        );

        let negotiation = nostr::ChannelMessageEvent::reply(
            channel_event_id,
            announce_id.clone(),
            relay,
            format!("agent-beta offers {skill_ref} with AC escrow"),
            now_epoch_seconds.saturating_add(base + 2),
        )
        .mention_pubkey("npub1alpha", Some(relay.to_string()));
        let negotiation_id = format!("sim:{}:{:08x}:b", nostr::KIND_CHANNEL_MESSAGE, round);
        self.push_event(
            "NIP-28",
            &negotiation_id,
            format!(
                "beta replied with terms ({} tags)",
                negotiation.to_tags().len()
            ),
        );

        self.push_event(
            "NIP-SKL",
            &format!("sim:{}:{:08x}", nostr::KIND_SKILL_MANIFEST, round),
            format!("beta published manifest {skill_ref}"),
        );
        self.push_event(
            "NIP-SA",
            &format!("sim:{}:{:08x}", nostr::KIND_SKILL_LICENSE, round),
            format!("alpha learned skill summarize-text@{skill_version}"),
        );
        self.push_event(
            "NIP-AC",
            &format!("sim:{}:{:08x}", nostr::KIND_CREDIT_INTENT, round),
            "opened escrow intent for skill execution".to_string(),
        );
        self.push_event(
            "NIP-AC",
            &format!("sim:{}:{:08x}", nostr::KIND_CREDIT_SETTLEMENT, round),
            "settled escrow after successful delivery".to_string(),
        );

        let transfer_sats = 250_u64.saturating_add(u64::from(round) * 35);
        self.total_transferred_sats = self.total_transferred_sats.saturating_add(transfer_sats);
        self.rounds_run = round;

        if !self.learned_skills.iter().any(|skill| skill == &skill_ref) {
            self.learned_skills.push(skill_ref);
        }

        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: agents exchanged NIP-28 messages and settled {transfer_sats} sats"
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Simulation reset".to_string());
        self.channel_event_id = None;
        self.rounds_run = 0;
        self.total_transferred_sats = 0;
        self.learned_skills.clear();
        self.events.clear();
        self.next_seq = 1;
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

pub struct TreasuryExchangeSimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub rounds_run: u32,
    pub order_event_id: Option<String>,
    pub mint_reference: Option<String>,
    pub wallet_connect_url: Option<String>,
    pub total_liquidity_sats: u64,
    pub trade_volume_sats: u64,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
}

impl Default for TreasuryExchangeSimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model treasury + exchange NIP interactions".to_string(),
            ),
            rounds_run: 0,
            order_event_id: None,
            mint_reference: None,
            wallet_connect_url: None,
            total_liquidity_sats: 0,
            trade_volume_sats: 0,
            events: Vec::new(),
            next_seq: 1,
        }
    }
}

impl TreasuryExchangeSimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let round = self.rounds_run.saturating_add(1);
        let swap_sats = 40_000_u64.saturating_add(u64::from(round) * 5_000);

        let handler = nostr::nip89::HandlerInfo::new(
            "npub1treasury",
            nostr::nip89::HandlerType::Agent,
            nostr::nip89::HandlerMetadata::new(
                "Treasury Agent",
                "Provides FX and liquidity routing for agent payments",
            ),
        )
        .add_capability("fx.quote.btcusd")
        .add_capability("swap.cashu.lightning")
        .with_pricing(
            nostr::nip89::PricingInfo::new(25)
                .with_model("per-swap")
                .with_currency("sat"),
        );
        let handler_tags = handler.to_tags();
        self.push_event(
            "NIP-89",
            &format!("sim:{}:{:08x}", nostr::nip89::KIND_HANDLER_INFO, round),
            format!("announced treasury handler ({} tags)", handler_tags.len()),
        );

        let mint_tags = nostr::nip87::create_cashu_mint_tags(
            "mint-pubkey-alpha",
            "https://mint.openagents.dev",
            &[1, 2, 3, 4, 5, 11, 12],
            nostr::nip87::MintNetwork::Mainnet,
        );
        let mint = nostr::nip87::parse_cashu_mint(
            nostr::nip87::KIND_CASHU_MINT,
            &mint_tags,
            "{\"name\":\"OpenAgents Mint\"}",
        )
        .map_err(|error| error.to_string())?;
        self.mint_reference = Some(format!("{} ({})", mint.url, mint.network.as_str()));
        self.push_event(
            "NIP-87",
            &format!("sim:{}:{:08x}", nostr::nip87::KIND_CASHU_MINT, round),
            format!("discovered mint {} with {} nuts", mint.url, mint.nuts.len()),
        );

        let order_id = format!("order-{:04}", round);
        let expires_at = now_epoch_seconds.saturating_add(900);
        let order_event = nostr::Event {
            id: format!("sim:{}:{:08x}", nostr::nip69::P2P_ORDER_KIND, round),
            pubkey: "npub1treasury".to_string(),
            created_at: now_epoch_seconds,
            kind: nostr::nip69::P2P_ORDER_KIND,
            tags: vec![
                vec!["d".to_string(), order_id.clone()],
                vec!["k".to_string(), "sell".to_string()],
                vec!["f".to_string(), "USD".to_string()],
                vec!["s".to_string(), "pending".to_string()],
                vec!["amt".to_string(), swap_sats.to_string()],
                vec!["fa".to_string(), "1250".to_string()],
                vec!["pm".to_string(), "wire".to_string(), "cashapp".to_string()],
                vec!["premium".to_string(), "1.5".to_string()],
                vec!["network".to_string(), "bitcoin".to_string()],
                vec!["layer".to_string(), "lightning".to_string()],
                vec!["expires_at".to_string(), expires_at.to_string()],
                vec!["expiration".to_string(), expires_at.to_string()],
                vec!["y".to_string(), "openagents-exchange".to_string()],
            ],
            content: String::new(),
            sig: format!("sim-signature-{round}"),
        };
        let order = nostr::nip69::P2POrder::from_event(order_event.clone())
            .map_err(|error| error.to_string())?;
        self.order_event_id = Some(order_event.id.clone());
        self.push_event(
            "NIP-69",
            &order_event.id,
            format!(
                "published {} order {} for {} sats",
                order.order_type.as_str(),
                order.order_id,
                order.amount_sats
            ),
        );

        let token_content = nostr::nip60::TokenContent::new(
            mint.url.clone(),
            vec![nostr::nip60::CashuProof::new(
                format!("proof-{round:04}"),
                swap_sats,
                format!("secret-{round:04}"),
                format!("C-{round:04}"),
            )],
        )
        .with_unit("sat".to_string());
        let locked_sats = token_content.total_amount();
        self.total_liquidity_sats = self.total_liquidity_sats.saturating_add(locked_sats);
        self.push_event(
            "NIP-60",
            &format!("sim:{}:{:08x}", nostr::nip60::TOKEN_KIND, round),
            format!("locked {} sats in wallet token batch", locked_sats),
        );

        let nutzap_proof = nostr::nip61::NutzapProof::new(
            swap_sats,
            format!("C-{round:04}"),
            format!("proof-{round:04}"),
            format!("secret-{round:04}"),
        );
        let proof_tag =
            nostr::nip61::create_proof_tag(&nutzap_proof).map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-61",
            &format!("sim:{}:{:08x}", nostr::nip61::NUTZAP_KIND, round),
            format!(
                "created nutzap settlement proof ({} fields)",
                proof_tag.len()
            ),
        );

        let wallet_connect_url = nostr::nip47::NostrWalletConnectUrl::new(
            "walletpubkey123",
            vec!["wss://relay.openagents.dev".to_string()],
            format!("secret-{round:04}"),
        )
        .with_lud16("treasury@openagents.dev")
        .to_string();
        self.wallet_connect_url = Some(wallet_connect_url.clone());
        self.push_event(
            "NIP-47",
            &format!("sim:{}:{:08x}", nostr::nip47::REQUEST_KIND, round),
            format!(
                "prepared wallet connect session ({} chars)",
                wallet_connect_url.len()
            ),
        );

        let reputation_label = nostr::nip32::LabelEvent::new(
            vec![nostr::nip32::Label::new("success", "exchange/trade")],
            vec![nostr::nip32::LabelTarget::event(
                order_event.id.clone(),
                Some("wss://relay.openagents.dev"),
            )],
        )
        .with_content("atomic swap completed within policy bounds");
        self.push_event(
            "NIP-32",
            &format!("sim:{}:{:08x}", nostr::nip32::KIND_LABEL, round),
            format!(
                "emitted trade attestation ({} tags)",
                reputation_label.to_tags().len()
            ),
        );

        self.rounds_run = round;
        self.trade_volume_sats = self.trade_volume_sats.saturating_add(swap_sats);
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: routed {} sats through discovery, orderbook, wallet and settlement rails",
            swap_sats
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Treasury exchange simulation reset".to_string());
        self.rounds_run = 0;
        self.order_event_id = None;
        self.mint_reference = None;
        self.wallet_connect_url = None;
        self.total_liquidity_sats = 0;
        self.trade_volume_sats = 0;
        self.events.clear();
        self.next_seq = 1;
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

pub struct RelaySecuritySimulationPaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub relay_url: String,
    pub challenge: String,
    pub auth_event_id: Option<String>,
    pub rounds_run: u32,
    pub dm_relay_count: u32,
    pub sync_ranges: u32,
    pub events: Vec<AgentNetworkSimulationEvent>,
    next_seq: u64,
}

impl Default for RelaySecuritySimulationPaneState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some(
                "Run simulation to model secure relay, auth, and sync lifecycle".to_string(),
            ),
            relay_url: "wss://relay.openagents.dev".to_string(),
            challenge: "auth-bootstrap".to_string(),
            auth_event_id: None,
            rounds_run: 0,
            dm_relay_count: 0,
            sync_ranges: 0,
            events: Vec::new(),
            next_seq: 1,
        }
    }
}

impl RelaySecuritySimulationPaneState {
    pub fn run_round(&mut self, now_epoch_seconds: u64) -> Result<(), String> {
        let round = self.rounds_run.saturating_add(1);

        let relay_doc = nostr::nip11::RelayInformationDocument {
            name: Some("OpenAgents Auth Relay".to_string()),
            description: Some("Relay exposing auth, DM, and audit sync capabilities".to_string()),
            supported_nips: Some(vec![11, 17, 42, 46, 65, 77, 98]),
            limitation: Some(nostr::nip11::RelayLimitation {
                auth_required: Some(true),
                restricted_writes: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let relay_doc_json = relay_doc.to_json().map_err(|error| error.to_string())?;
        let relay_doc_roundtrip =
            nostr::nip11::RelayInformationDocument::from_json(&relay_doc_json)
                .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-11",
            &format!("sim:30111:{:08x}", round),
            format!(
                "loaded relay document ({} advertised nips)",
                relay_doc_roundtrip
                    .supported_nips
                    .map_or(0, |nips| nips.len())
            ),
        );

        let relay_list = nostr::nip65::RelayListMetadata::new(vec![
            nostr::nip65::RelayEntry::write(self.relay_url.clone()),
            nostr::nip65::RelayEntry::read("wss://relay.backup.openagents.dev".to_string()),
        ]);
        self.dm_relay_count = relay_list.all_relays().len() as u32;
        self.push_event(
            "NIP-65",
            &format!(
                "sim:{}:{:08x}",
                nostr::nip65::RELAY_LIST_METADATA_KIND,
                round
            ),
            format!(
                "published relay list (read={} write={})",
                relay_list.read_relays().len(),
                relay_list.write_relays().len()
            ),
        );

        let challenge = format!("auth-{round:04x}");
        self.challenge.clone_from(&challenge);
        let auth_template = nostr::nip42::create_auth_event_template(&self.relay_url, &challenge);
        let auth_event = nostr::Event {
            id: format!("sim:{}:{:08x}", nostr::nip42::AUTH_KIND, round),
            pubkey: "npub1agentalpha".to_string(),
            created_at: auth_template.created_at,
            kind: auth_template.kind,
            tags: auth_template.tags,
            content: auth_template.content,
            sig: format!("sim-auth-signature-{round}"),
        };
        nostr::nip42::validate_auth_event(
            &auth_event,
            &self.relay_url,
            &challenge,
            Some(auth_event.created_at),
        )
        .map_err(|error| error.to_string())?;
        self.auth_event_id = Some(auth_event.id.clone());
        self.push_event(
            "NIP-42",
            &auth_event.id,
            "validated relay authentication event".to_string(),
        );

        let signer_request = nostr::nip46::NostrConnectRequest::get_public_key();
        let signer_request_json = signer_request
            .to_json()
            .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-46",
            &format!("sim:{}:{:08x}", nostr::nip46::KIND_NOSTR_CONNECT, round),
            format!(
                "queued remote signing request {} ({} bytes)",
                signer_request.id,
                signer_request_json.len()
            ),
        );

        let sender_sk = nostr::generate_secret_key();
        let recipient_sk = nostr::generate_secret_key();
        let recipient_pk = nostr::get_public_key_hex(&recipient_sk).map_err(|e| e.to_string())?;
        let message =
            nostr::nip17::ChatMessage::new(format!("relay-auth heartbeat {} confirmed", round))
                .add_recipient(recipient_pk.clone(), Some(self.relay_url.clone()))
                .subject("secure-ops");
        let wrapped = nostr::nip17::send_chat_message(
            &message,
            &sender_sk,
            &recipient_pk,
            now_epoch_seconds.saturating_add(3),
        )
        .map_err(|error| error.to_string())?;
        let received = nostr::nip17::receive_chat_message(&wrapped, &recipient_sk)
            .map_err(|e| e.to_string())?;
        self.push_event(
            "NIP-17",
            &format!("sim:{}:{:08x}", nostr::nip17::KIND_CHAT_MESSAGE, round),
            format!(
                "sent private chat message to {} recipient(s)",
                received.recipients.len()
            ),
        );
        self.push_event(
            "NIP-59",
            &format!("sim:{}:{:08x}", nostr::nip59::KIND_GIFT_WRAP, round),
            format!("wrapped private message event (kind {})", wrapped.kind),
        );

        let endpoint = format!("https://api.openagents.dev/v1/relay/check/{round}");
        let payload = format!(
            "{{\"challenge\":\"{}\",\"relay\":\"{}\"}}",
            challenge, self.relay_url
        );
        let payload_hash = nostr::nip98::hash_payload(payload.as_bytes());
        let http_auth =
            nostr::nip98::HttpAuth::new(endpoint.clone(), nostr::nip98::HttpMethod::Post)
                .with_payload_hash(payload_hash.clone());
        let http_auth_tags = http_auth.to_tags();
        let validation = nostr::nip98::ValidationParams::new(
            endpoint,
            nostr::nip98::HttpMethod::Post,
            now_epoch_seconds.saturating_add(4),
        )
        .with_payload_hash(payload_hash)
        .with_timestamp_window(120);
        nostr::nip98::validate_http_auth_event(
            nostr::nip98::KIND_HTTP_AUTH,
            now_epoch_seconds.saturating_add(4),
            &http_auth_tags,
            &validation,
        )
        .map_err(|error| error.to_string())?;
        self.push_event(
            "NIP-98",
            &format!("sim:{}:{:08x}", nostr::nip98::KIND_HTTP_AUTH, round),
            "validated HTTP auth request tags".to_string(),
        );

        let round_byte = (round % u32::from(u8::MAX.saturating_sub(2))).saturating_add(1) as u8;
        let mut records = vec![
            nostr::nip77::Record::new(now_epoch_seconds.saturating_add(1), [round_byte; 32]),
            nostr::nip77::Record::new(
                now_epoch_seconds.saturating_add(2),
                [round_byte.saturating_add(1); 32],
            ),
            nostr::nip77::Record::new(
                now_epoch_seconds.saturating_add(3),
                [round_byte.saturating_add(2); 32],
            ),
        ];
        nostr::nip77::sort_records(&mut records);
        let ids: Vec<nostr::nip77::EventId> = records.iter().map(|record| record.id).collect();
        let fingerprint = nostr::nip77::calculate_fingerprint(&ids);
        let negentropy =
            nostr::nip77::NegentropyMessage::new(vec![nostr::nip77::Range::fingerprint(
                nostr::nip77::Bound::infinity(),
                fingerprint,
            )]);
        let encoded = negentropy.encode_hex().map_err(|error| error.to_string())?;
        let decoded =
            nostr::nip77::NegentropyMessage::decode_hex(&encoded).map_err(|e| e.to_string())?;
        self.sync_ranges = self.sync_ranges.saturating_add(decoded.ranges.len() as u32);
        self.push_event(
            "NIP-77",
            &format!("sim:30077:{:08x}", round),
            format!(
                "reconciled negentropy message ({} hex chars)",
                encoded.len()
            ),
        );

        self.rounds_run = round;
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some(format!(
            "Round {round}: relay auth, private messaging, HTTP auth, and negentropy sync succeeded"
        ));
        Ok(())
    }

    pub fn reset(&mut self) {
        self.load_state = PaneLoadState::Ready;
        self.last_error = None;
        self.last_action = Some("Relay security simulation reset".to_string());
        self.challenge = "auth-bootstrap".to_string();
        self.auth_event_id = None;
        self.rounds_run = 0;
        self.dm_relay_count = 0;
        self.sync_ranges = 0;
        self.events.clear();
        self.next_seq = 1;
    }

    fn push_event(&mut self, protocol: &str, event_ref: &str, summary: String) {
        self.events.push(AgentNetworkSimulationEvent {
            seq: self.next_seq,
            protocol: protocol.to_string(),
            event_ref: event_ref.to_string(),
            summary,
        });
        self.next_seq = self.next_seq.saturating_add(1);
        if self.events.len() > 18 {
            let overflow = self.events.len().saturating_sub(18);
            self.events.drain(0..overflow);
        }
    }
}

pub struct EarningsScoreboardState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub sats_today: u64,
    pub lifetime_sats: u64,
    pub jobs_today: u64,
    pub last_job_result: String,
    pub online_uptime_seconds: u64,
    pub stale_after: Duration,
    pub last_refreshed_at: Option<Instant>,
}

impl Default for EarningsScoreboardState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for wallet + job receipts".to_string()),
            sats_today: 0,
            lifetime_sats: 0,
            jobs_today: 0,
            last_job_result: "none".to_string(),
            online_uptime_seconds: 0,
            stale_after: Duration::from_secs(12),
            last_refreshed_at: None,
        }
    }
}

impl EarningsScoreboardState {
    pub fn refresh_from_sources(
        &mut self,
        now: Instant,
        provider_runtime: &ProviderRuntimeState,
        job_history: &JobHistoryState,
        spark_wallet: &SparkPaneState,
    ) {
        self.last_refreshed_at = Some(now);
        self.online_uptime_seconds = provider_runtime.uptime_seconds(now);
        self.last_error = None;

        if let Some(error) = spark_wallet.last_error.as_deref() {
            self.load_state = PaneLoadState::Error;
            self.last_error = Some(format!("Wallet source error: {error}"));
            self.last_action = Some("Scoreboard degraded due to wallet error".to_string());
        } else if spark_wallet.balance.is_none() {
            self.load_state = PaneLoadState::Loading;
            self.last_action = Some("Scoreboard waiting for first wallet refresh".to_string());
        } else {
            self.load_state = PaneLoadState::Ready;
            self.last_action = Some("Scoreboard refreshed from authoritative sources".to_string());
        }

        self.lifetime_sats = spark_wallet
            .balance
            .as_ref()
            .map_or(0, spark_total_balance_sats);

        let threshold = job_history.reference_epoch_seconds.saturating_sub(86_400);
        self.jobs_today = job_history
            .rows
            .iter()
            .filter(|row| {
                row.status == JobHistoryStatus::Succeeded
                    && row.completed_at_epoch_seconds >= threshold
            })
            .count() as u64;
        self.sats_today = job_history
            .rows
            .iter()
            .filter(|row| {
                row.status == JobHistoryStatus::Succeeded
                    && row.completed_at_epoch_seconds >= threshold
            })
            .map(|row| row.payout_sats)
            .sum();

        self.last_job_result = job_history
            .rows
            .first()
            .map(|row| {
                if let Some(reason) = row.failure_reason.as_deref() {
                    format!("{} ({reason})", row.status.label())
                } else {
                    row.status.label().to_string()
                }
            })
            .unwrap_or_else(|| "none".to_string());
    }

    pub fn is_stale(&self, now: Instant) -> bool {
        self.last_refreshed_at
            .is_none_or(|refresh| now.duration_since(refresh) > self.stale_after)
    }
}

pub struct NostrSecretState {
    pub reveal_duration: Duration,
    pub revealed_until: Option<Instant>,
    pub copy_notice: Option<String>,
    pub copy_notice_until: Option<Instant>,
}

impl Default for NostrSecretState {
    fn default() -> Self {
        Self {
            reveal_duration: Duration::from_secs(12),
            revealed_until: None,
            copy_notice: None,
            copy_notice_until: None,
        }
    }
}

impl NostrSecretState {
    pub fn is_revealed(&self, now: Instant) -> bool {
        self.revealed_until.is_some_and(|until| until > now)
    }

    pub fn toggle_reveal(&mut self, now: Instant) {
        if self.is_revealed(now) {
            self.revealed_until = None;
        } else {
            self.revealed_until = Some(now + self.reveal_duration);
        }
    }

    pub fn set_copy_notice(&mut self, now: Instant, message: String) {
        self.copy_notice = Some(message);
        self.copy_notice_until = Some(now + Duration::from_secs(4));
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        let mut changed = false;

        if self.revealed_until.is_some_and(|until| until <= now) {
            self.revealed_until = None;
            changed = true;
        }

        if self.copy_notice_until.is_some_and(|until| until <= now) {
            self.copy_notice = None;
            self.copy_notice_until = None;
            changed = true;
        }

        changed
    }
}

impl_pane_status_access!(
    CodexAccountPaneState,
    CodexModelsPaneState,
    CodexConfigPaneState,
    CodexMcpPaneState,
    CodexAppsPaneState,
    CodexRemoteSkillsPaneState,
    CodexLabsPaneState,
    CodexDiagnosticsPaneState,
    RelayConnectionsState,
    SyncHealthState,
    NetworkRequestsState,
    StarterJobsState,
    ActivityFeedState,
    AlertsRecoveryState,
    SettingsState,
    JobInboxState,
    ActiveJobState,
    JobHistoryState,
    AgentProfileStatePaneState,
    AgentScheduleTickPaneState,
    TrajectoryAuditPaneState,
    SkillRegistryPaneState,
    SkillTrustRevocationPaneState,
    CreditDeskPaneState,
    CreditSettlementLedgerPaneState,
    AgentNetworkSimulationPaneState,
    TreasuryExchangeSimulationPaneState,
    RelaySecuritySimulationPaneState,
);

pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub scale_factor: f32,
    pub hotbar: Hotbar,
    pub hotbar_bounds: Bounds,
    pub event_context: EventContext,
    pub input_modifiers: Modifiers,
    pub panes: Vec<DesktopPane>,
    pub nostr_identity: Option<NostrIdentity>,
    pub nostr_identity_error: Option<String>,
    pub nostr_secret_state: NostrSecretState,
    pub spark_wallet: SparkPaneState,
    pub spark_worker: SparkWalletWorker,
    pub spark_inputs: SparkPaneInputs,
    pub pay_invoice_inputs: PayInvoicePaneInputs,
    pub create_invoice_inputs: CreateInvoicePaneInputs,
    pub relay_connections_inputs: RelayConnectionsPaneInputs,
    pub network_requests_inputs: NetworkRequestsPaneInputs,
    pub settings_inputs: SettingsPaneInputs,
    pub job_history_inputs: JobHistoryPaneInputs,
    pub chat_inputs: ChatPaneInputs,
    pub autopilot_chat: AutopilotChatState,
    pub codex_account: CodexAccountPaneState,
    pub codex_models: CodexModelsPaneState,
    pub codex_config: CodexConfigPaneState,
    pub codex_mcp: CodexMcpPaneState,
    pub codex_apps: CodexAppsPaneState,
    pub codex_remote_skills: CodexRemoteSkillsPaneState,
    pub codex_labs: CodexLabsPaneState,
    pub codex_diagnostics: CodexDiagnosticsPaneState,
    pub codex_lane: CodexLaneSnapshot,
    pub codex_lane_config: crate::codex_lane::CodexLaneConfig,
    pub codex_lane_worker: CodexLaneWorker,
    pub codex_command_responses: Vec<CodexLaneCommandResponse>,
    pub codex_notifications: Vec<CodexLaneNotification>,
    pub next_codex_command_seq: u64,
    pub sa_lane: SaLaneSnapshot,
    pub skl_lane: SklLaneSnapshot,
    pub ac_lane: AcLaneSnapshot,
    pub sa_lane_worker: SaLaneWorker,
    pub skl_lane_worker: SklLaneWorker,
    pub ac_lane_worker: AcLaneWorker,
    pub runtime_command_responses: Vec<RuntimeCommandResponse>,
    pub next_runtime_command_seq: u64,
    pub provider_runtime: ProviderRuntimeState,
    pub earnings_scoreboard: EarningsScoreboardState,
    pub relay_connections: RelayConnectionsState,
    pub sync_health: SyncHealthState,
    pub network_requests: NetworkRequestsState,
    pub starter_jobs: StarterJobsState,
    pub activity_feed: ActivityFeedState,
    pub alerts_recovery: AlertsRecoveryState,
    pub settings: SettingsState,
    pub job_inbox: JobInboxState,
    pub active_job: ActiveJobState,
    pub job_history: JobHistoryState,
    pub agent_profile_state: AgentProfileStatePaneState,
    pub agent_schedule_tick: AgentScheduleTickPaneState,
    pub trajectory_audit: TrajectoryAuditPaneState,
    pub skill_registry: SkillRegistryPaneState,
    pub skill_trust_revocation: SkillTrustRevocationPaneState,
    pub credit_desk: CreditDeskPaneState,
    pub credit_settlement_ledger: CreditSettlementLedgerPaneState,
    pub agent_network_simulation: AgentNetworkSimulationPaneState,
    pub treasury_exchange_simulation: TreasuryExchangeSimulationPaneState,
    pub relay_security_simulation: RelaySecuritySimulationPaneState,
    pub sidebar: SidebarState,
    pub next_pane_id: u64,
    pub next_z_index: i32,
    pub pane_drag_mode: Option<PaneDragMode>,
    pub pane_resizer: ResizablePane,
    pub hotbar_flash_was_active: bool,
    pub command_palette: CommandPalette,
    pub command_palette_actions: Rc<RefCell<Vec<String>>>,
}

impl RenderState {
    fn allocate_runtime_command_seq(&mut self) -> u64 {
        let seq = self.next_runtime_command_seq;
        self.next_runtime_command_seq = self.next_runtime_command_seq.saturating_add(1);
        seq
    }

    pub fn queue_sa_command(&mut self, command: SaLifecycleCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.sa_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    fn allocate_codex_command_seq(&mut self) -> u64 {
        let seq = self.next_codex_command_seq;
        self.next_codex_command_seq = self.next_codex_command_seq.saturating_add(1);
        seq
    }

    pub fn queue_codex_command(&mut self, command: CodexLaneCommand) -> Result<u64, String> {
        let seq = self.allocate_codex_command_seq();
        self.codex_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn restart_codex_lane(&mut self) {
        let replacement = CodexLaneWorker::spawn(self.codex_lane_config.clone());
        let mut previous = std::mem::replace(&mut self.codex_lane_worker, replacement);
        previous.shutdown();
        self.codex_lane = CodexLaneSnapshot::default();
        self.autopilot_chat.set_connection_status("starting");
    }

    pub fn record_codex_command_response(&mut self, response: CodexLaneCommandResponse) {
        self.codex_command_responses.push(response);
        if self.codex_command_responses.len() > 128 {
            let overflow = self.codex_command_responses.len().saturating_sub(128);
            self.codex_command_responses.drain(0..overflow);
        }
    }

    pub fn record_codex_notification(&mut self, notification: CodexLaneNotification) {
        self.codex_notifications.push(notification);
        if self.codex_notifications.len() > 256 {
            let overflow = self.codex_notifications.len().saturating_sub(256);
            self.codex_notifications.drain(0..overflow);
        }
    }

    pub fn queue_skl_command(&mut self, command: SklDiscoveryTrustCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.skl_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn queue_ac_command(&mut self, command: AcCreditCommand) -> Result<u64, String> {
        let seq = self.allocate_runtime_command_seq();
        self.ac_lane_worker.enqueue(seq, command).map(|()| seq)
    }

    pub fn record_runtime_command_response(&mut self, response: RuntimeCommandResponse) {
        self.runtime_command_responses.push(response);
        if self.runtime_command_responses.len() > 128 {
            let overflow = self.runtime_command_responses.len().saturating_sub(128);
            self.runtime_command_responses.drain(0..overflow);
        }
    }

    pub fn provider_blockers(&self) -> Vec<ProviderBlocker> {
        let mut blockers = Vec::new();
        if self.nostr_identity.is_none() {
            blockers.push(ProviderBlocker::IdentityMissing);
        }
        if self.spark_wallet.last_error.is_some() {
            blockers.push(ProviderBlocker::WalletError);
        }
        if self.skl_lane.trust_tier != SkillTrustTier::Trusted {
            blockers.push(ProviderBlocker::SkillTrustUnavailable);
        }
        if !self.ac_lane.credit_available {
            blockers.push(ProviderBlocker::CreditLaneUnavailable);
        }
        blockers
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveJobState, ActivityEventDomain, ActivityEventRow, ActivityFeedFilter,
        ActivityFeedState, AgentNetworkSimulationPaneState, AlertDomain, AlertLifecycle,
        AlertsRecoveryState, AutopilotChatState, AutopilotMessageStatus, AutopilotRole,
        EarningsScoreboardState, JobHistoryState, JobHistoryStatus, JobHistoryStatusFilter,
        JobHistoryTimeRange, JobInboxDecision, JobInboxNetworkRequest, JobInboxState,
        JobInboxValidation, JobLifecycleStage, NetworkRequestStatus, NetworkRequestSubmission,
        NetworkRequestsState, NostrSecretState, ProviderRuntimeState, RecoveryAlertRow,
        RelayConnectionRow, RelayConnectionStatus, RelayConnectionsState,
        RelaySecuritySimulationPaneState, SettingsState, SparkPaneState, StarterJobRow,
        StarterJobStatus, StarterJobsState, SyncHealthState, SyncRecoveryPhase,
        TreasuryExchangeSimulationPaneState,
    };

    fn fixture_inbox_request(
        request_id: &str,
        capability: &str,
        price_sats: u64,
        ttl_seconds: u64,
        validation: JobInboxValidation,
    ) -> JobInboxNetworkRequest {
        JobInboxNetworkRequest {
            request_id: request_id.to_string(),
            requester: format!("npub1{request_id}"),
            capability: capability.to_string(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats,
            ttl_seconds,
            validation,
        }
    }

    fn seed_job_inbox(requests: Vec<JobInboxNetworkRequest>) -> JobInboxState {
        let mut inbox = JobInboxState::default();
        for request in requests {
            inbox.upsert_network_request(request);
        }
        inbox
    }

    fn fixture_history_row(
        job_id: &str,
        status: JobHistoryStatus,
        completed_at_epoch_seconds: u64,
        payout_sats: u64,
    ) -> super::JobHistoryReceiptRow {
        super::JobHistoryReceiptRow {
            job_id: job_id.to_string(),
            status,
            completed_at_epoch_seconds,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            payout_sats,
            result_hash: format!("sha256:{job_id}"),
            payment_pointer: format!("pay:{job_id}"),
            failure_reason: if status == JobHistoryStatus::Failed {
                Some("failure".to_string())
            } else {
                None
            },
        }
    }

    fn seed_job_history(rows: Vec<super::JobHistoryReceiptRow>) -> JobHistoryState {
        let mut history = JobHistoryState::default();
        for row in rows {
            history.upsert_row(row);
        }
        history
    }

    fn fixture_starter_job(
        job_id: &str,
        payout_sats: u64,
        eligible: bool,
        status: StarterJobStatus,
    ) -> StarterJobRow {
        StarterJobRow {
            job_id: job_id.to_string(),
            summary: "Process starter job".to_string(),
            payout_sats,
            eligible,
            status,
            payout_pointer: None,
        }
    }

    fn fixture_activity_event(
        event_id: &str,
        domain: ActivityEventDomain,
        occurred_at_epoch_seconds: u64,
    ) -> ActivityEventRow {
        ActivityEventRow {
            event_id: event_id.to_string(),
            domain,
            source_tag: domain.source_tag().to_string(),
            occurred_at_epoch_seconds,
            summary: format!("summary {event_id}"),
            detail: format!("detail {event_id}"),
        }
    }

    fn fixture_alert(
        alert_id: &str,
        domain: AlertDomain,
        severity: super::AlertSeverity,
        lifecycle: AlertLifecycle,
    ) -> RecoveryAlertRow {
        RecoveryAlertRow {
            alert_id: alert_id.to_string(),
            domain,
            severity,
            lifecycle,
            summary: format!("summary {alert_id}"),
            remediation: format!("remediation {alert_id}"),
            last_transition_epoch_seconds: 1_761_920_080,
        }
    }

    #[test]
    fn nostr_reveal_state_expires() {
        let mut state = NostrSecretState::default();
        let now = std::time::Instant::now();
        state.toggle_reveal(now);
        assert!(state.is_revealed(now));

        let expired_at = now + state.reveal_duration + std::time::Duration::from_millis(1);
        assert!(state.expire(expired_at));
        assert!(!state.is_revealed(expired_at));
    }

    #[test]
    fn nostr_copy_notice_expires() {
        let mut state = NostrSecretState::default();
        let now = std::time::Instant::now();
        state.set_copy_notice(now, "Copied".to_string());
        assert_eq!(state.copy_notice.as_deref(), Some("Copied"));

        let expired_at = now + std::time::Duration::from_secs(5);
        assert!(state.expire(expired_at));
        assert!(state.copy_notice.is_none());
    }

    #[test]
    fn chat_state_tracks_codex_turn_lifecycle() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("ping".to_string());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Queued)
        );

        chat.mark_turn_started("turn-1".to_string());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.status == AutopilotMessageStatus::Running)
        );

        chat.append_turn_delta("pong");
        chat.mark_turn_completed();
        assert!(!chat.has_pending_messages());
        assert!(
            chat.messages
                .iter()
                .any(|message| message.content.contains("pong"))
        );
    }

    #[test]
    fn chat_state_replaces_transcript_for_active_thread() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-a".to_string());
        chat.set_active_thread_transcript(
            "thread-a",
            vec![
                (AutopilotRole::User, "hello".to_string()),
                (AutopilotRole::Codex, "world".to_string()),
            ],
        );
        assert_eq!(chat.messages.len(), 2);
        assert_eq!(chat.messages[0].content, "hello");
        assert_eq!(chat.messages[1].content, "world");

        chat.remember_thread("thread-b");
        chat.set_active_thread_transcript(
            "thread-b",
            vec![(AutopilotRole::User, "ignored".to_string())],
        );
        assert_eq!(chat.messages[0].content, "hello");

        chat.ensure_thread("thread-b".to_string());
        chat.set_active_thread_transcript(
            "thread-b",
            vec![(AutopilotRole::User, "new thread".to_string())],
        );
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(chat.messages[0].content, "new thread");
    }

    #[test]
    fn chat_state_tracks_thread_metadata_and_filters() {
        let mut chat = AutopilotChatState::default();
        chat.set_thread_entries(vec![
            super::AutopilotThreadListEntry {
                thread_id: "thread-a".to_string(),
                thread_name: Some("Alpha".to_string()),
                status: Some("idle".to_string()),
                loaded: false,
                cwd: Some("/tmp/a".to_string()),
                path: Some("/tmp/a.jsonl".to_string()),
            },
            super::AutopilotThreadListEntry {
                thread_id: "thread-b".to_string(),
                thread_name: None,
                status: Some("active:waitingOnApproval".to_string()),
                loaded: false,
                cwd: Some("/tmp/b".to_string()),
                path: None,
            },
        ]);

        chat.set_thread_loaded_ids(&["thread-b".to_string()]);
        assert_eq!(chat.active_thread_id.as_deref(), Some("thread-a"));
        assert_eq!(chat.thread_label("thread-a"), "Alpha [thread-a]");
        assert_eq!(chat.thread_metadata["thread-a"].loaded, false);
        assert_eq!(chat.thread_metadata["thread-b"].loaded, true);

        chat.cycle_thread_filter_archived();
        chat.cycle_thread_filter_sort_key();
        chat.cycle_thread_filter_source_kind();
        chat.cycle_thread_filter_model_provider();
        chat.thread_filter_search_term = "alpha".to_string();
        let params = chat.build_thread_list_params(Some("/workspace".to_string()));
        assert_eq!(params.archived, Some(true));
        assert_eq!(
            params.sort_key,
            Some(codex_client::ThreadSortKey::CreatedAt)
        );
        assert_eq!(
            params.source_kinds,
            Some(vec![codex_client::ThreadSourceKind::AppServer])
        );
        assert_eq!(params.model_providers, Some(vec!["openai".to_string()]));
        assert_eq!(params.search_term.as_deref(), Some("alpha"));
        assert_eq!(params.cwd.as_deref(), Some("/workspace"));
    }

    #[test]
    fn job_inbox_upsert_collapses_duplicate_request_ids() {
        let inbox = seed_job_inbox(vec![
            fixture_inbox_request("req-dup", "cap.one", 11, 60, JobInboxValidation::Pending),
            fixture_inbox_request("req-dup", "cap.one", 22, 120, JobInboxValidation::Valid),
        ]);

        let duplicates = inbox
            .requests
            .iter()
            .filter(|request| request.request_id == "req-dup")
            .count();
        assert_eq!(duplicates, 1);
        let request = inbox
            .requests
            .iter()
            .find(|request| request.request_id == "req-dup")
            .expect("request should exist");
        assert_eq!(request.price_sats, 22);
        assert_eq!(request.ttl_seconds, 120);
    }

    #[test]
    fn job_inbox_accept_updates_selected_request_decision() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-accept",
            "summarize.text",
            900,
            120,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request_id = inbox
            .selected_request()
            .expect("selection should exist")
            .request_id
            .clone();

        let decided = inbox
            .decide_selected(true, "valid + priced")
            .expect("decision should succeed");
        assert_eq!(decided, request_id);
        let selected = inbox.selected_request().expect("selected request remains");
        assert!(matches!(
            selected.decision,
            JobInboxDecision::Accepted { ref reason } if reason == "valid + priced"
        ));
    }

    #[test]
    fn active_job_advance_stage_updates_lifecycle() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-active",
            "summarize.text",
            1500,
            300,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let request = inbox
            .selected_request()
            .expect("request should exist")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        let stage = active.advance_stage().expect("advance should succeed");
        assert_eq!(stage, JobLifecycleStage::Running);
        let stage = active
            .advance_stage()
            .expect("second advance should succeed");
        assert_eq!(stage, JobLifecycleStage::Delivered);
        let current = active.job.as_ref().expect("active job exists");
        assert_eq!(current.stage, JobLifecycleStage::Delivered);
        assert!(current.invoice_id.is_some());
    }

    #[test]
    fn job_history_filters_search_status_and_time() {
        let mut history = seed_job_history(vec![
            fixture_history_row(
                "job-bootstrap-000",
                JobHistoryStatus::Succeeded,
                1_761_919_970,
                2100,
            ),
            fixture_history_row(
                "job-bootstrap-001",
                JobHistoryStatus::Failed,
                1_761_919_940,
                0,
            ),
        ]);
        history.status_filter = JobHistoryStatusFilter::Succeeded;
        history.time_range = JobHistoryTimeRange::All;
        history.set_search_job_id("bootstrap-000".to_string());

        let rows = history.paged_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, JobHistoryStatus::Succeeded);
        assert!(rows[0].job_id.contains("bootstrap-000"));
    }

    #[test]
    fn job_history_upsert_keeps_single_row_per_job_id() {
        let mut history = seed_job_history(vec![fixture_history_row(
            "job-bootstrap-000",
            JobHistoryStatus::Succeeded,
            1_761_920_000,
            1200,
        )]);
        let before = history.rows.len();
        history.upsert_row(super::JobHistoryReceiptRow {
            job_id: "job-bootstrap-000".to_string(),
            status: JobHistoryStatus::Failed,
            completed_at_epoch_seconds: history.reference_epoch_seconds + 10,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_result_event_id: None,
            sa_trajectory_session_id: None,
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            payout_sats: 0,
            result_hash: "sha256:updated".to_string(),
            payment_pointer: "pay:updated".to_string(),
            failure_reason: Some("updated".to_string()),
        });

        assert_eq!(history.rows.len(), before);
        let row = history
            .rows
            .iter()
            .find(|row| row.job_id == "job-bootstrap-000")
            .expect("row should exist");
        assert_eq!(row.result_hash, "sha256:updated");
    }

    #[test]
    fn relay_connections_add_retry_remove_flow() {
        let mut relays = RelayConnectionsState::default();
        assert!(relays.add_relay("wss://relay.new.example").is_ok());
        assert_eq!(
            relays.selected().map(|row| row.url.as_str()),
            Some("wss://relay.new.example")
        );

        assert!(relays.retry_selected().is_ok());
        assert_eq!(
            relays.selected().map(|row| row.status),
            Some(RelayConnectionStatus::Connected)
        );

        assert!(relays.remove_selected().is_ok());
        assert!(
            relays
                .relays
                .iter()
                .all(|row| row.url != "wss://relay.new.example")
        );
    }

    #[test]
    fn sync_health_detects_stale_cursor_and_rebootstrap() {
        let provider = ProviderRuntimeState::default();
        let relays = RelayConnectionsState::default();
        let mut sync = SyncHealthState::default();

        sync.cursor_last_advanced_seconds_ago = sync.cursor_stale_after_seconds + 5;
        sync.refresh_from_runtime(std::time::Instant::now(), &provider, &relays);
        assert_eq!(sync.load_state, super::PaneLoadState::Error);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Reconnecting);

        sync.rebootstrap();
        assert_eq!(sync.load_state, super::PaneLoadState::Ready);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Replaying);
        assert_eq!(sync.cursor_last_advanced_seconds_ago, 0);
    }

    #[test]
    fn sync_health_marks_resubscribing_when_relays_are_lost() {
        let provider = ProviderRuntimeState::default();
        let mut relays = RelayConnectionsState::default();
        relays.relays.push(RelayConnectionRow {
            url: "wss://relay-a.example".to_string(),
            status: RelayConnectionStatus::Connected,
            latency_ms: Some(42),
            last_seen_seconds_ago: Some(0),
            last_error: None,
        });
        let mut sync = SyncHealthState::default();

        sync.refresh_from_runtime(std::time::Instant::now(), &provider, &relays);
        assert_eq!(sync.subscription_state, "subscribed");

        relays.relays[0].status = RelayConnectionStatus::Error;
        relays.relays[0].last_error = Some("relay dropped connection".to_string());
        sync.refresh_from_runtime(std::time::Instant::now(), &provider, &relays);
        assert_eq!(sync.subscription_state, "resubscribing");
    }

    #[test]
    fn network_requests_submit_validates_and_records_stream_link() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_type: "translate.text".to_string(),
                payload: "{\"text\":\"hola\"}".to_string(),
                skill_scope_id: Some("33400:npub1agent:summarize-text:0.1.0".to_string()),
                credit_envelope_ref: Some("ac:39242:00000001".to_string()),
                budget_sats: 1200,
                timeout_seconds: 90,
                authority_command_seq: 44,
            })
            .expect("request should be accepted");
        let first = requests
            .submitted
            .first()
            .expect("new request should be inserted at head");
        assert_eq!(first.request_id, request_id);
        assert_eq!(first.response_stream_id, format!("stream:{request_id}"));
        assert_eq!(first.status, NetworkRequestStatus::Submitted);
        assert_eq!(first.authority_command_seq, 44);
    }

    #[test]
    fn agent_network_simulation_rounds_generate_channel_skill_and_credit_events() {
        let mut state = AgentNetworkSimulationPaneState::default();
        state
            .run_round(1_761_920_800)
            .expect("first simulation round should succeed");
        assert_eq!(state.rounds_run, 1);
        assert!(state.channel_event_id.is_some());
        assert!(state.total_transferred_sats > 0);
        assert!(!state.learned_skills.is_empty());
        assert!(state.events.iter().any(|event| event.protocol == "NIP-28"));
        assert!(state.events.iter().any(|event| event.protocol == "NIP-SKL"));
        assert!(state.events.iter().any(|event| event.protocol == "NIP-AC"));

        state.reset();
        assert_eq!(state.rounds_run, 0);
        assert!(state.channel_event_id.is_none());
        assert!(state.events.is_empty());
    }

    #[test]
    fn treasury_exchange_simulation_rounds_generate_exchange_and_wallet_events() {
        let mut state = TreasuryExchangeSimulationPaneState::default();
        state
            .run_round(1_761_920_900)
            .expect("treasury simulation round should succeed");
        assert_eq!(state.rounds_run, 1);
        assert!(state.order_event_id.is_some());
        assert!(state.mint_reference.is_some());
        assert!(state.wallet_connect_url.is_some());
        assert!(state.trade_volume_sats > 0);
        assert!(state.events.iter().any(|event| event.protocol == "NIP-69"));
        assert!(state.events.iter().any(|event| event.protocol == "NIP-60"));
        assert!(state.events.iter().any(|event| event.protocol == "NIP-61"));

        state.reset();
        assert_eq!(state.rounds_run, 0);
        assert!(state.events.is_empty());
    }

    #[test]
    fn relay_security_simulation_rounds_generate_auth_privacy_and_sync_events() {
        let mut state = RelaySecuritySimulationPaneState::default();
        state
            .run_round(1_761_921_000)
            .expect("relay security simulation round should succeed");
        assert_eq!(state.rounds_run, 1);
        assert!(state.auth_event_id.is_some());
        assert!(state.dm_relay_count >= 2);
        assert!(state.sync_ranges > 0);
        assert!(state.events.iter().any(|event| event.protocol == "NIP-42"));
        assert!(state.events.iter().any(|event| event.protocol == "NIP-59"));
        assert!(state.events.iter().any(|event| event.protocol == "NIP-77"));

        state.reset();
        assert_eq!(state.rounds_run, 0);
        assert!(state.events.is_empty());
    }

    #[test]
    fn starter_jobs_complete_selected_sets_payout_pointer() {
        let mut starter_jobs = StarterJobsState::default();
        starter_jobs.jobs.push(fixture_starter_job(
            "job-starter-001",
            1200,
            true,
            StarterJobStatus::Queued,
        ));
        starter_jobs.select_by_index(0);
        let (job_id, _payout, pointer) = starter_jobs
            .complete_selected()
            .expect("eligible starter job should complete");
        let job = starter_jobs
            .jobs
            .iter()
            .find(|job| job.job_id == job_id)
            .expect("job should remain present");
        assert_eq!(job.status, StarterJobStatus::Completed);
        assert_eq!(job.payout_pointer.as_deref(), Some(pointer.as_str()));
    }

    #[test]
    fn activity_feed_upsert_deduplicates_stable_event_ids() {
        let mut feed = ActivityFeedState::default();
        feed.upsert_event(fixture_activity_event(
            "wallet:payment:latest",
            ActivityEventDomain::Wallet,
            1_761_920_180,
        ));
        let baseline_count = feed.rows.len();
        feed.upsert_event(fixture_activity_event(
            "wallet:payment:latest",
            ActivityEventDomain::Wallet,
            1_761_920_200,
        ));
        assert_eq!(feed.rows.len(), baseline_count);

        feed.set_filter(ActivityFeedFilter::Wallet);
        assert!(
            feed.visible_rows()
                .into_iter()
                .all(|row| row.domain == ActivityEventDomain::Wallet)
        );
    }

    #[test]
    fn alerts_recovery_lifecycle_transitions_are_deterministic() {
        let mut alerts = AlertsRecoveryState::default();
        alerts.alerts.push(fixture_alert(
            "alert:identity:missing",
            AlertDomain::Identity,
            super::AlertSeverity::Critical,
            AlertLifecycle::Active,
        ));
        alerts.select_by_index(0);
        let alert_id = alerts
            .acknowledge_selected()
            .expect("active alert should acknowledge");
        let alert = alerts
            .alerts
            .iter()
            .find(|alert| alert.alert_id == alert_id)
            .expect("alert should exist after ack");
        assert_eq!(alert.lifecycle, AlertLifecycle::Acknowledged);
        assert_eq!(alerts.selected_domain(), Some(AlertDomain::Identity));

        let resolved_id = alerts
            .resolve_selected()
            .expect("acknowledged alert should resolve");
        let resolved = alerts
            .alerts
            .iter()
            .find(|alert| alert.alert_id == resolved_id)
            .expect("alert should exist after resolve");
        assert_eq!(resolved.lifecycle, AlertLifecycle::Resolved);
    }

    #[test]
    fn settings_updates_validate_ranges_and_reconnect_notice() {
        let mut settings = SettingsState::default();
        settings
            .apply_updates_internal("wss://relay.primal.net", "2500", "8", false)
            .expect("valid settings update should apply");
        assert_eq!(settings.document.relay_url, "wss://relay.primal.net");
        assert_eq!(settings.document.wallet_default_send_sats, 2500);
        assert_eq!(settings.document.provider_max_queue_depth, 8);
        assert!(settings.document.reconnect_required);

        let invalid = settings.apply_updates_internal("https://bad-relay", "0", "0", false);
        assert!(invalid.is_err());
        assert_eq!(settings.load_state, super::PaneLoadState::Error);
    }

    #[test]
    fn settings_document_default_uses_identity_authority_path() {
        let document = super::SettingsDocumentV1::default();
        assert!(document.identity_path.contains("identity.mnemonic"));
    }

    #[test]
    fn parse_settings_document_overrides_stale_identity_path() {
        let raw = "schema_version=1\nrelay_url=wss://relay.example\nidentity_path=~/.openagents/nostr/identity.json\nwallet_default_send_sats=1000\nprovider_max_queue_depth=4\nreconnect_required=false\n";
        let document = super::parse_settings_document(raw).expect("settings parse should succeed");
        assert_ne!(document.identity_path, "~/.openagents/nostr/identity.json");
        assert!(document.identity_path.contains("identity.mnemonic"));
    }

    #[test]
    fn earnings_scoreboard_refreshes_from_wallet_and_history() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let history = seed_job_history(vec![fixture_history_row(
            "job-earned-001",
            JobHistoryStatus::Succeeded,
            1_761_919_970,
            2100,
        )]);
        let mut spark = SparkPaneState::default();
        spark.balance = Some(openagents_spark::Balance {
            spark_sats: 1000,
            lightning_sats: 2000,
            onchain_sats: 3000,
        });

        let now = std::time::Instant::now();
        score.refresh_from_sources(now, &provider, &history, &spark);

        assert_eq!(score.load_state, super::PaneLoadState::Ready);
        assert_eq!(score.lifetime_sats, 6000);
        assert!(score.jobs_today >= 1);
        assert!(score.sats_today >= 1);
        assert!(!score.is_stale(now));
    }

    #[test]
    fn earnings_scoreboard_surfaces_wallet_errors() {
        let mut score = EarningsScoreboardState::default();
        let provider = ProviderRuntimeState::default();
        let history = JobHistoryState::default();
        let mut spark = SparkPaneState::default();
        spark.last_error = Some("wallet backend unavailable".to_string());

        score.refresh_from_sources(std::time::Instant::now(), &provider, &history, &spark);

        assert_eq!(score.load_state, super::PaneLoadState::Error);
        assert!(
            score
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("wallet backend unavailable"))
        );
    }
}
