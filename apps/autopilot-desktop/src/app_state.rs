use std::collections::VecDeque;
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
use crate::provider_nip90_lane::{
    ProviderNip90LaneCommand, ProviderNip90LaneSnapshot, ProviderNip90LaneWorker,
};
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
    spark_wallet::{SparkPaneState, SparkWalletCommand, SparkWalletWorker},
    stablesats_blink_worker::StableSatsBlinkWorker,
};

#[path = "app_state_domains.rs"]
mod app_state_domains;
mod credentials_state;
pub use app_state_domains::*;
pub use credentials_state::CredentialsState;

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
    Credentials,
    JobInbox,
    ActiveJob,
    JobHistory,
    EmailInbox,
    EmailDraftQueue,
    EmailApprovalQueue,
    EmailSendLog,
    EmailFollowUpQueue,
    NostrIdentity,
    SparkWallet,
    SparkCreateInvoice,
    SparkPayInvoice,
    AgentProfileState,
    AgentScheduleTick,
    TrajectoryAudit,
    CastControl,
    SkillRegistry,
    SkillTrustRevocation,
    CreditDesk,
    CreditSettlementLedger,
    AgentNetworkSimulation,
    TreasuryExchangeSimulation,
    RelaySecuritySimulation,
    StableSatsSimulation,
    Calculator,
    CadDemo,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CadCameraDragMode {
    Orbit,
    Pan,
}

#[derive(Clone, Copy, Debug)]
pub struct CadCameraDragState {
    pub pane_id: u64,
    pub tile_index: usize,
    pub mode: CadCameraDragMode,
    pub last_mouse: Point,
    pub moved: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ChatTranscriptSelectionDragState {
    pub message_id: u64,
    pub anchor_byte_offset: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ChatTranscriptSelectionState {
    pub message_id: u64,
    pub start_byte_offset: usize,
    pub end_byte_offset: usize,
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

pub struct CredentialsPaneInputs {
    pub variable_name: TextInput,
    pub variable_value: TextInput,
}

impl Default for CredentialsPaneInputs {
    fn default() -> Self {
        Self {
            variable_name: TextInput::new().placeholder("ENV_VAR_NAME"),
            variable_value: TextInput::new().placeholder("Value (stored in secure keychain)"),
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
            composer: TextInput::new().placeholder("Message Autopilot"),
        }
    }
}

pub struct CalculatorPaneInputs {
    pub expression: TextInput,
}

impl Default for CalculatorPaneInputs {
    fn default() -> Self {
        Self {
            expression: TextInput::new()
                .placeholder("e.g. (8 + 2) * 3 - 4 / 2")
                .mono(true),
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
    pub structured: Option<AutopilotStructuredMessage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotProgressRow {
    pub label: String,
    pub value: String,
    pub tone: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotProgressBlock {
    pub kind: String,
    pub title: String,
    pub status: String,
    pub rows: Vec<AutopilotProgressRow>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AutopilotStructuredMessage {
    pub reasoning: String,
    pub answer: String,
    pub events: Vec<String>,
    pub status: Option<String>,
    pub progress_blocks: Vec<AutopilotProgressBlock>,
}

impl AutopilotStructuredMessage {
    fn rendered_content(&self) -> String {
        let reasoning = self.reasoning.trim_end();
        let answer = self.answer.trim_end();
        if reasoning.is_empty() {
            return answer.to_string();
        }
        if answer.is_empty() {
            return reasoning.to_string();
        }
        format!("{reasoning}\n\n{answer}")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AutopilotDeltaSignature {
    turn_id: String,
    item_id: String,
    delta: String,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutopilotTurnMetadata {
    pub submission_seq: u64,
    pub thread_id: String,
    pub is_cad_turn: bool,
    pub classifier_reason: String,
    pub submitted_at_epoch_ms: u64,
    pub selected_skill_names: Vec<String>,
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
    pub startup_new_thread_bootstrap_pending: bool,
    pub startup_new_thread_bootstrap_sent: bool,
    pub messages: Vec<AutopilotMessage>,
    pub next_message_id: u64,
    pub active_turn_id: Option<String>,
    pub active_assistant_message_id: Option<u64>,
    pub pending_assistant_message_ids: VecDeque<u64>,
    pub turn_assistant_message_ids: std::collections::HashMap<String, u64>,
    pub next_turn_submission_seq: u64,
    pub pending_turn_metadata: VecDeque<AutopilotTurnMetadata>,
    pub turn_metadata_by_turn_id: std::collections::HashMap<String, AutopilotTurnMetadata>,
    pub last_submitted_turn_metadata: Option<AutopilotTurnMetadata>,
    last_agent_item_ids: std::collections::HashMap<String, String>,
    last_reasoning_item_ids: std::collections::HashMap<String, String>,
    last_agent_delta_signature: Option<AutopilotDeltaSignature>,
    last_reasoning_delta_signature: Option<AutopilotDeltaSignature>,
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
    pub transcript_scroll_offset: f32,
    pub transcript_follow_tail: bool,
    pub transcript_selection: Option<ChatTranscriptSelectionState>,
    pub last_error: Option<String>,
    pub copy_notice: Option<String>,
    pub copy_notice_until: Option<Instant>,
}

impl Default for AutopilotChatState {
    fn default() -> Self {
        Self {
            connection_status: "ready".to_string(),
            // "auto" means "let app-server pick the current default model".
            models: vec!["auto".to_string()],
            selected_model: 0,
            reasoning_effort: Some("medium".to_string()),
            threads: Vec::new(),
            thread_metadata: std::collections::HashMap::new(),
            active_thread_id: None,
            startup_new_thread_bootstrap_pending: true,
            startup_new_thread_bootstrap_sent: false,
            messages: Vec::new(),
            next_message_id: 1,
            active_turn_id: None,
            active_assistant_message_id: None,
            pending_assistant_message_ids: VecDeque::new(),
            turn_assistant_message_ids: std::collections::HashMap::new(),
            next_turn_submission_seq: 1,
            pending_turn_metadata: VecDeque::new(),
            turn_metadata_by_turn_id: std::collections::HashMap::new(),
            last_submitted_turn_metadata: None,
            last_agent_item_ids: std::collections::HashMap::new(),
            last_reasoning_item_ids: std::collections::HashMap::new(),
            last_agent_delta_signature: None,
            last_reasoning_delta_signature: None,
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
            transcript_scroll_offset: 0.0,
            transcript_follow_tail: true,
            transcript_selection: None,
            last_error: None,
            copy_notice: None,
            copy_notice_until: None,
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
    pub fn reset_transcript_scroll(&mut self) {
        self.transcript_scroll_offset = 0.0;
        self.transcript_follow_tail = true;
    }

    pub fn clear_transcript_selection(&mut self) {
        self.transcript_selection = None;
    }

    pub fn transcript_effective_scroll_offset(&self, max_scroll: f32) -> f32 {
        let max_scroll = max_scroll.max(0.0);
        if self.transcript_follow_tail {
            max_scroll
        } else {
            self.transcript_scroll_offset.clamp(0.0, max_scroll)
        }
    }

    pub fn scroll_transcript_by(&mut self, delta: f32, max_scroll: f32) {
        let max_scroll = max_scroll.max(0.0);
        if max_scroll <= 0.0 {
            self.reset_transcript_scroll();
            return;
        }

        let mut offset = self.transcript_effective_scroll_offset(max_scroll) + delta;
        if !offset.is_finite() {
            offset = max_scroll;
        }
        offset = offset.clamp(0.0, max_scroll);
        self.transcript_scroll_offset = offset;
        self.transcript_follow_tail = (max_scroll - offset).abs() <= 1.0;
    }

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

        // Preserve an explicit prior user selection first.
        if let Some(previous_model) = previous_model.as_ref()
            && !previous_model.eq_ignore_ascii_case("auto")
            && let Some(index) = self.models.iter().position(|model| model == previous_model)
        {
            self.selected_model = index;
            self.last_error = None;
            return;
        }

        // Prefer current high-quality coding models when available.
        let preferred_models = [
            "gpt-5.2-codex",
            "gpt-5.1-codex-max",
            "gpt-5.2",
            "gpt-5.3-codex",
            "gpt-5.3-codex-spark",
        ];
        if let Some(index) = preferred_models
            .iter()
            .find_map(|model| self.models.iter().position(|value| value == model))
        {
            self.selected_model = index;
            self.last_error = None;
            return;
        }

        if let Some(default_model) = default_model.as_ref()
            && let Some(index) = self.models.iter().position(|model| model == default_model)
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
        let previous_active_thread_id = self.active_thread_id.clone();
        let previous_metadata = self.thread_metadata.clone();
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
        if let Some(active_id) = previous_active_thread_id {
            if !self.threads.iter().any(|thread_id| thread_id == &active_id) {
                self.threads.insert(0, active_id.clone());
                self.thread_metadata.insert(
                    active_id.clone(),
                    previous_metadata
                        .get(&active_id)
                        .cloned()
                        .unwrap_or(AutopilotThreadMetadata {
                            thread_name: None,
                            status: None,
                            loaded: false,
                            cwd: None,
                            path: None,
                        }),
                );
            }
            self.active_thread_id = Some(active_id);
        } else {
            self.active_thread_id = self.threads.first().cloned();
        }
    }

    pub fn select_thread_by_index(&mut self, index: usize) -> Option<AutopilotThreadResumeTarget> {
        let thread_id = self.threads.get(index).cloned()?;
        self.active_thread_id = Some(thread_id.clone());
        self.reset_transcript_scroll();
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
        self.reset_transcript_scroll();
    }

    pub fn is_active_thread(&self, thread_id: &str) -> bool {
        self.active_thread_id.as_deref() == Some(thread_id)
    }

    pub fn remove_thread(&mut self, thread_id: &str) {
        self.threads.retain(|value| value != thread_id);
        self.thread_metadata.remove(thread_id);
        self.pending_turn_metadata
            .retain(|metadata| metadata.thread_id != thread_id);
        self.turn_metadata_by_turn_id
            .retain(|_, metadata| metadata.thread_id != thread_id);
        if self
            .last_submitted_turn_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.thread_id == thread_id)
        {
            self.last_submitted_turn_metadata = None;
        }
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
                structured: None,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
        }

        self.active_turn_id = None;
        self.active_assistant_message_id = None;
        self.pending_assistant_message_ids.clear();
        self.turn_assistant_message_ids.clear();
        self.pending_turn_metadata
            .retain(|metadata| metadata.thread_id != thread_id);
        self.turn_metadata_by_turn_id
            .retain(|_, metadata| metadata.thread_id != thread_id);
        if self
            .last_submitted_turn_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.thread_id == thread_id)
        {
            self.last_submitted_turn_metadata = None;
        }
        self.last_agent_item_ids.clear();
        self.last_reasoning_item_ids.clear();
        self.last_agent_delta_signature = None;
        self.last_reasoning_delta_signature = None;
        self.last_turn_status = None;
        self.token_usage = None;
        self.turn_plan_explanation = None;
        self.turn_plan.clear();
        self.turn_diff = None;
        self.turn_timeline.clear();
        self.transcript_selection = None;
        self.last_error = None;
    }

    pub fn submit_prompt(&mut self, prompt: String) {
        self.last_error = None;
        self.transcript_selection = None;
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            self.last_error = Some("Prompt cannot be empty".to_string());
            self.messages.push(AutopilotMessage {
                id: self.next_message_id,
                role: AutopilotRole::Codex,
                status: AutopilotMessageStatus::Error,
                content: "Cannot run empty prompt".to_string(),
                structured: None,
            });
            self.next_message_id = self.next_message_id.saturating_add(1);
            return;
        }

        self.messages.push(AutopilotMessage {
            id: self.next_message_id,
            role: AutopilotRole::User,
            status: AutopilotMessageStatus::Done,
            content: trimmed.to_string(),
            structured: None,
        });
        self.next_message_id = self.next_message_id.saturating_add(1);

        let assistant_message_id = self.next_message_id;
        self.messages.push(AutopilotMessage {
            id: assistant_message_id,
            role: AutopilotRole::Codex,
            status: AutopilotMessageStatus::Queued,
            content: String::new(),
            structured: Some(AutopilotStructuredMessage::default()),
        });
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.pending_assistant_message_ids
            .push_back(assistant_message_id);
        self.active_assistant_message_id = Some(assistant_message_id);
    }

    pub fn record_turn_submission_metadata(
        &mut self,
        thread_id: &str,
        is_cad_turn: bool,
        classifier_reason: impl Into<String>,
        submitted_at_epoch_ms: u64,
        selected_skill_names: Vec<String>,
    ) {
        let mut selected_skill_names = selected_skill_names
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        selected_skill_names.sort();
        selected_skill_names.dedup();
        let metadata = AutopilotTurnMetadata {
            submission_seq: self.next_turn_submission_seq,
            thread_id: thread_id.to_string(),
            is_cad_turn,
            classifier_reason: classifier_reason.into(),
            submitted_at_epoch_ms,
            selected_skill_names,
        };
        self.next_turn_submission_seq = self.next_turn_submission_seq.saturating_add(1);
        self.last_submitted_turn_metadata = Some(metadata.clone());
        self.pending_turn_metadata.push_back(metadata);
        if self.pending_turn_metadata.len() > 64 {
            let overflow = self.pending_turn_metadata.len().saturating_sub(64);
            self.pending_turn_metadata.drain(0..overflow);
        }
    }

    pub fn set_last_pending_turn_selected_skills(&mut self, selected_skill_names: Vec<String>) {
        let mut selected_skill_names = selected_skill_names
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        selected_skill_names.sort();
        selected_skill_names.dedup();
        if let Some(last) = self.pending_turn_metadata.back_mut() {
            last.selected_skill_names = selected_skill_names.clone();
        }
        if let Some(last) = self.last_submitted_turn_metadata.as_mut() {
            last.selected_skill_names = selected_skill_names;
        }
    }

    pub fn turn_metadata_for(&self, turn_id: &str) -> Option<&AutopilotTurnMetadata> {
        self.turn_metadata_by_turn_id.get(turn_id)
    }

    pub fn active_turn_metadata(&self) -> Option<&AutopilotTurnMetadata> {
        self.active_turn_id
            .as_deref()
            .and_then(|turn_id| self.turn_metadata_for(turn_id))
            .or(self.last_submitted_turn_metadata.as_ref())
    }

    pub fn mark_turn_started(&mut self, turn_id: String) {
        self.active_turn_id = Some(turn_id.clone());
        if let Some(metadata) = self.pending_turn_metadata.pop_front() {
            self.turn_metadata_by_turn_id
                .insert(turn_id.clone(), metadata);
            if self.turn_metadata_by_turn_id.len() > 64 {
                let mut sorted = self
                    .turn_metadata_by_turn_id
                    .iter()
                    .map(|(key, value)| (key.clone(), value.submission_seq))
                    .collect::<Vec<_>>();
                sorted.sort_by_key(|(_, seq)| *seq);
                let overflow = sorted.len().saturating_sub(64);
                for (key, _) in sorted.into_iter().take(overflow) {
                    self.turn_metadata_by_turn_id.remove(&key);
                }
            }
        }
        self.last_turn_status = Some("inProgress".to_string());
        if let Some(assistant_message_id) = self.bind_turn_to_assistant_message(&turn_id)
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Running;
            self.active_assistant_message_id = Some(assistant_message_id);
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
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            if self.active_turn_id.as_deref() == Some(turn_id) {
                message.status = AutopilotMessageStatus::Running;
            }
            if let Some(structured) = message.structured.as_mut() {
                structured.answer.push_str(delta);
                structured
                    .events
                    .push(format!("answer+{}", delta.chars().count()));
                message.content = structured.rendered_content();
            } else {
                message.content.push_str(delta);
            }
        }
    }

    pub fn append_turn_reasoning_delta_for_turn(&mut self, turn_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            if self.active_turn_id.as_deref() == Some(turn_id) {
                message.status = AutopilotMessageStatus::Running;
            }
            if let Some(structured) = message.structured.as_mut() {
                // Explicit typed ordering rule: ignore late reasoning after answer streaming starts.
                if !structured.answer.trim().is_empty() {
                    return;
                }
                structured.reasoning.push_str(delta);
                structured
                    .events
                    .push(format!("reasoning+{}", delta.chars().count()));
                message.content = structured.rendered_content();
            } else {
                message.content.push_str(delta);
            }
        }
    }

    pub fn set_turn_message_for_turn(&mut self, turn_id: &str, content: &str) {
        let content = content.trim_end();
        if content.trim().is_empty() {
            return;
        }
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            if self.active_turn_id.as_deref() == Some(turn_id) {
                message.status = AutopilotMessageStatus::Running;
            }
            if let Some(structured) = message.structured.as_mut() {
                if structured.answer == content {
                    return;
                }
                structured.answer = content.to_string();
                structured.status = Some("answer".to_string());
                let rendered = structured.rendered_content();
                if message.content != rendered {
                    message.content = rendered;
                }
                return;
            }
            if message.content != content {
                message.content = content.to_string();
            }
        }
    }

    pub fn set_turn_progress_blocks_for_turn(
        &mut self,
        turn_id: &str,
        progress_blocks: Vec<AutopilotProgressBlock>,
    ) {
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            let structured = message
                .structured
                .get_or_insert_with(AutopilotStructuredMessage::default);
            if structured.progress_blocks == progress_blocks {
                return;
            }
            structured.progress_blocks = progress_blocks;
            message.content = structured.rendered_content();
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
            .or_else(|| self.bind_turn_to_assistant_message(turn_id));
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Done;
            if self.active_assistant_message_id == Some(assistant_message_id) {
                self.active_assistant_message_id = None;
            }
        }
        self.last_turn_status = Some("completed".to_string());
        if self.active_turn_id.as_deref() == Some(turn_id) {
            self.active_turn_id = None;
        }
    }

    #[allow(dead_code)]
    pub fn mark_turn_error(&mut self, error: impl Into<String>) {
        let error = error.into();
        if let Some(turn_id) = self.active_turn_id.clone() {
            self.mark_turn_error_for(&turn_id, error);
            return;
        }
        self.mark_turn_error_for("unknown-turn", error);
    }

    pub fn mark_turn_error_for(&mut self, turn_id: &str, error: impl Into<String>) {
        let error = error.into();
        let assistant_message_id = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or_else(|| self.bind_turn_to_assistant_message(turn_id))
            .or(self.active_assistant_message_id);
        if let Some(assistant_message_id) = assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Error;
            if message.content.trim().is_empty() {
                message.content.clone_from(&error);
            }
            if self.active_assistant_message_id == Some(assistant_message_id) {
                self.active_assistant_message_id = None;
            }
        }
        self.last_turn_status = Some("failed".to_string());
        self.last_error = Some(error);
        if self.active_turn_id.as_deref() == Some(turn_id) {
            self.active_turn_id = None;
        }
    }

    pub fn mark_pending_turn_dispatch_failed(&mut self, error: impl Into<String>) {
        let error = error.into();
        let _ = self.pending_turn_metadata.pop_front();
        if let Some(assistant_message_id) = self.active_assistant_message_id
            && let Some(message) = self
                .messages
                .iter_mut()
                .find(|message| message.id == assistant_message_id)
        {
            message.status = AutopilotMessageStatus::Error;
            message.content.clone_from(&error);
        }
        self.pending_assistant_message_ids
            .retain(|id| Some(*id) != self.active_assistant_message_id);
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

    pub fn set_copy_notice(&mut self, now: Instant, message: String) {
        self.copy_notice = Some(message);
        self.copy_notice_until = Some(now + Duration::from_secs(3));
    }

    pub fn expire_copy_notice(&mut self, now: Instant) -> bool {
        if self.copy_notice_until.is_some_and(|until| until <= now) {
            self.copy_notice = None;
            self.copy_notice_until = None;
            return true;
        }
        false
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

    pub fn turn_has_visible_output(&self, turn_id: &str) -> bool {
        let Some(assistant_message_id) = self
            .turn_assistant_message_ids
            .get(turn_id)
            .copied()
            .or(self.active_assistant_message_id)
        else {
            return false;
        };
        self.messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .is_some_and(|message| !message.content.trim().is_empty())
    }

    fn bind_turn_to_assistant_message(&mut self, turn_id: &str) -> Option<u64> {
        if let Some(existing) = self.turn_assistant_message_ids.get(turn_id).copied() {
            return Some(existing);
        }
        let assistant_message_id = self
            .pending_assistant_message_ids
            .pop_front()
            .or(self.active_assistant_message_id)?;
        self.turn_assistant_message_ids
            .insert(turn_id.to_string(), assistant_message_id);
        Some(assistant_message_id)
    }

    pub fn is_duplicate_agent_delta(&mut self, turn_id: &str, item_id: &str, delta: &str) -> bool {
        Self::is_duplicate_delta(
            turn_id,
            item_id,
            delta,
            &mut self.last_agent_item_ids,
            &mut self.last_agent_delta_signature,
            |value| value.is_empty() || value == "event-agent-message" || value == "n/a",
        )
    }

    pub fn is_duplicate_reasoning_delta(
        &mut self,
        turn_id: &str,
        item_id: &str,
        delta: &str,
    ) -> bool {
        Self::is_duplicate_delta(
            turn_id,
            item_id,
            delta,
            &mut self.last_reasoning_item_ids,
            &mut self.last_reasoning_delta_signature,
            |value| value.is_empty() || value == "event-reasoning" || value == "n/a",
        )
    }

    fn is_duplicate_delta(
        turn_id: &str,
        item_id: &str,
        delta: &str,
        item_ids_by_turn: &mut std::collections::HashMap<String, String>,
        last_signature: &mut Option<AutopilotDeltaSignature>,
        is_fallback_item: impl Fn(&str) -> bool,
    ) -> bool {
        if delta.is_empty() {
            return false;
        }

        let canonical_item_id = if is_fallback_item(item_id) {
            item_ids_by_turn
                .get(turn_id)
                .cloned()
                .unwrap_or_else(|| item_id.to_string())
        } else {
            item_ids_by_turn.insert(turn_id.to_string(), item_id.to_string());
            item_id.to_string()
        };

        let candidate = AutopilotDeltaSignature {
            turn_id: turn_id.to_string(),
            item_id: canonical_item_id,
            delta: delta.to_string(),
        };
        let duplicate = last_signature
            .as_ref()
            .is_some_and(|last| *last == candidate);
        *last_signature = Some(candidate);
        duplicate
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
    Cad,
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
            Self::Cad => "cad",
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
            Self::Cad => "cad.events",
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
    Cad,
    Job,
    Wallet,
    Network,
    Sync,
    Sa,
    Skl,
    Ac,
}

impl ActivityFeedFilter {
    pub const fn all() -> [Self; 10] {
        [
            Self::All,
            Self::Chat,
            Self::Cad,
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
            Self::Cad => "cad",
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
            Self::Cad => domain == ActivityEventDomain::Cad,
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
    pub request_kind: u16,
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
            request_kind: request.request_kind,
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

        match job.stage {
            JobLifecycleStage::Accepted => {
                if job.sa_tick_request_event_id.is_none() {
                    self.last_error =
                        Some("Cannot mark running without authoritative running event".to_string());
                    self.load_state = PaneLoadState::Error;
                    return Err(
                        "missing authoritative running event (sa_tick_request_event_id)"
                            .to_string(),
                    );
                }
            }
            JobLifecycleStage::Running => {
                if job.sa_tick_result_event_id.is_none() {
                    self.last_error = Some(
                        "Cannot mark delivered without authoritative delivery event".to_string(),
                    );
                    self.load_state = PaneLoadState::Error;
                    return Err(
                        "missing authoritative delivered event (sa_tick_result_event_id)"
                            .to_string(),
                    );
                }
            }
            JobLifecycleStage::Delivered => {
                if !is_authoritative_payment_pointer(job.payment_id.as_deref()) {
                    self.last_error = Some(
                        "Cannot mark paid without wallet-authoritative payment pointer".to_string(),
                    );
                    self.load_state = PaneLoadState::Error;
                    return Err(
                        "missing authoritative payment pointer for paid transition".to_string()
                    );
                }
            }
            JobLifecycleStage::Received | JobLifecycleStage::Paid | JobLifecycleStage::Failed => {}
        }

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
        let authority_ref = match next_stage {
            JobLifecycleStage::Accepted => Some(job.request_id.clone()),
            JobLifecycleStage::Running => job.sa_tick_request_event_id.clone(),
            JobLifecycleStage::Delivered => job.sa_tick_result_event_id.clone(),
            JobLifecycleStage::Paid => job
                .payment_id
                .clone()
                .or_else(|| job.ac_settlement_event_id.clone()),
            JobLifecycleStage::Received | JobLifecycleStage::Failed => None,
        };
        self.append_event(format!(
            "stage advanced to {} (authority={})",
            next_stage.label(),
            authority_ref.unwrap_or_else(|| "none".to_string())
        ));
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
        let payment_pointer = job
            .payment_id
            .clone()
            .or_else(|| job.invoice_id.clone())
            .unwrap_or_else(|| format!("pending:{}", job.request_id));
        let authoritative_settlement =
            is_authoritative_payment_pointer(Some(payment_pointer.as_str()));
        let converted_to_failed =
            status == JobHistoryStatus::Succeeded && !authoritative_settlement;
        let settled_success = status == JobHistoryStatus::Succeeded && authoritative_settlement;
        let status = if settled_success {
            JobHistoryStatus::Succeeded
        } else if converted_to_failed {
            JobHistoryStatus::Failed
        } else {
            status
        };
        let failure_reason = if settled_success {
            None
        } else if converted_to_failed && job.failure_reason.is_none() {
            Some("payment settlement not wallet-confirmed".to_string())
        } else {
            job.failure_reason.clone()
        };
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
            payout_sats: if settled_success {
                job.quoted_price_sats
            } else {
                0
            },
            result_hash: format!("sha256:{}-{}", job.request_id, job.stage.label()),
            payment_pointer,
            failure_reason,
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

fn is_authoritative_payment_pointer(pointer: Option<&str>) -> bool {
    let Some(pointer) = pointer else {
        return false;
    };
    let pointer = pointer.trim();
    !pointer.is_empty()
        && !pointer.starts_with("pending:")
        && !pointer.starts_with("pay:")
        && !pointer.starts_with("inv-")
        && !pointer.starts_with("pay-req-")
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
    CodexLabsPaneState,
    CodexDiagnosticsPaneState,
    RelayConnectionsState,
    SyncHealthState,
    NetworkRequestsState,
    StarterJobsState,
    ActivityFeedState,
    AlertsRecoveryState,
    SettingsState,
    CredentialsState,
    JobInboxState,
    ActiveJobState,
    JobHistoryState,
    AgentProfileStatePaneState,
    AgentScheduleTickPaneState,
    TrajectoryAuditPaneState,
    CastControlPaneState,
    SkillRegistryPaneState,
    SkillTrustRevocationPaneState,
    CadDemoPaneState,
    CreditDeskPaneState,
    CreditSettlementLedgerPaneState,
    AgentNetworkSimulationPaneState,
    TreasuryExchangeSimulationPaneState,
    RelaySecuritySimulationPaneState,
    StableSatsSimulationPaneState,
);

pub struct CastControlProcess {
    pub child: std::process::Child,
    pub operation: String,
    pub receipt_path: String,
    pub log_path: String,
}

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
    pub stable_sats_blink_worker: StableSatsBlinkWorker,
    pub spark_inputs: SparkPaneInputs,
    pub pay_invoice_inputs: PayInvoicePaneInputs,
    pub create_invoice_inputs: CreateInvoicePaneInputs,
    pub relay_connections_inputs: RelayConnectionsPaneInputs,
    pub network_requests_inputs: NetworkRequestsPaneInputs,
    pub settings_inputs: SettingsPaneInputs,
    pub credentials_inputs: CredentialsPaneInputs,
    pub job_history_inputs: JobHistoryPaneInputs,
    pub chat_inputs: ChatPaneInputs,
    pub calculator_inputs: CalculatorPaneInputs,
    pub autopilot_chat: AutopilotChatState,
    pub chat_transcript_selection_drag: Option<ChatTranscriptSelectionDragState>,
    pub codex_account: CodexAccountPaneState,
    pub codex_models: CodexModelsPaneState,
    pub codex_config: CodexConfigPaneState,
    pub codex_mcp: CodexMcpPaneState,
    pub codex_apps: CodexAppsPaneState,
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
    pub provider_nip90_lane: ProviderNip90LaneSnapshot,
    pub provider_nip90_lane_worker: ProviderNip90LaneWorker,
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
    pub credentials: CredentialsState,
    pub job_inbox: JobInboxState,
    pub active_job: ActiveJobState,
    pub job_history: JobHistoryState,
    pub agent_profile_state: AgentProfileStatePaneState,
    pub agent_schedule_tick: AgentScheduleTickPaneState,
    pub trajectory_audit: TrajectoryAuditPaneState,
    pub cast_control: CastControlPaneState,
    pub cast_control_process: Option<CastControlProcess>,
    pub skill_registry: SkillRegistryPaneState,
    pub skill_trust_revocation: SkillTrustRevocationPaneState,
    pub credit_desk: CreditDeskPaneState,
    pub credit_settlement_ledger: CreditSettlementLedgerPaneState,
    pub cad_demo: CadDemoPaneState,
    pub agent_network_simulation: AgentNetworkSimulationPaneState,
    pub treasury_exchange_simulation: TreasuryExchangeSimulationPaneState,
    pub relay_security_simulation: RelaySecuritySimulationPaneState,
    pub stable_sats_simulation: StableSatsSimulationPaneState,
    pub autopilot_goals: crate::state::autopilot_goals::AutopilotGoalsState,
    pub goal_loop_executor: crate::state::goal_loop_executor::GoalLoopExecutorState,
    pub goal_restart_recovery_ran: bool,
    pub sidebar: SidebarState,
    pub next_pane_id: u64,
    pub next_z_index: i32,
    pub pane_drag_mode: Option<PaneDragMode>,
    pub cad_camera_drag_state: Option<CadCameraDragState>,
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
        tracing::info!("codex lane restart requested");
        let replacement = CodexLaneWorker::spawn(self.codex_lane_config.clone());
        let mut previous = std::mem::replace(&mut self.codex_lane_worker, replacement);
        previous.shutdown_async();
        self.codex_lane = CodexLaneSnapshot::default();
        self.autopilot_chat.set_connection_status("starting");
        tracing::info!("codex lane restart dispatched (non-blocking shutdown)");
    }

    pub fn sync_credentials_runtime(&mut self, restart_codex: bool) {
        let codex_scope = crate::credentials::CREDENTIAL_SCOPE_CODEX
            | crate::credentials::CREDENTIAL_SCOPE_SKILLS
            | crate::credentials::CREDENTIAL_SCOPE_GLOBAL;
        let spark_scope = crate::credentials::CREDENTIAL_SCOPE_SPARK
            | crate::credentials::CREDENTIAL_SCOPE_GLOBAL;

        match self.credentials.resolve_env_for_scope(codex_scope) {
            Ok(codex_env) => {
                let changed = self.codex_lane_config.env != codex_env;
                self.codex_lane_config.env = codex_env;
                if restart_codex && changed {
                    self.restart_codex_lane();
                    self.autopilot_chat
                        .set_connection_status("restarting (credential env updated)");
                }
            }
            Err(error) => {
                self.credentials.last_error = Some(error);
                self.credentials.load_state = PaneLoadState::Error;
            }
        }

        match self.credentials.resolve_env_for_scope(spark_scope) {
            Ok(spark_env) => {
                if let Err(error) = self
                    .spark_worker
                    .enqueue(SparkWalletCommand::ConfigureEnv { vars: spark_env })
                {
                    self.credentials.last_error = Some(error);
                    self.credentials.load_state = PaneLoadState::Error;
                } else {
                    let _ = self.spark_worker.enqueue(SparkWalletCommand::Refresh);
                }
            }
            Err(error) => {
                self.credentials.last_error = Some(error);
                self.credentials.load_state = PaneLoadState::Error;
            }
        }
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

    pub fn queue_provider_nip90_lane_command(
        &mut self,
        command: ProviderNip90LaneCommand,
    ) -> Result<(), String> {
        self.provider_nip90_lane_worker.enqueue(command)
    }

    pub fn configured_provider_relay_urls(&self) -> Vec<String> {
        let mut relays = self
            .relay_connections
            .relays
            .iter()
            .map(|row| row.url.trim())
            .filter(|url| !url.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        if relays.is_empty() {
            let default_relay = self.settings.document.relay_url.trim();
            if !default_relay.is_empty() {
                relays.push(default_relay.to_string());
            }
        }

        let mut seen = std::collections::HashSet::<String>::new();
        relays.retain(|relay| seen.insert(relay.clone()));
        relays
    }

    pub fn sync_provider_nip90_lane_relays(&mut self) -> Result<(), String> {
        let relays = self.configured_provider_relay_urls();
        self.queue_provider_nip90_lane_command(ProviderNip90LaneCommand::ConfigureRelays { relays })
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
        CadBuildFailureClass, CadBuildSessionPhase, CadCameraViewSnap, CadContextMenuTargetKind,
        CadDemoPaneState, CadDemoWarningState, CadDrawingViewDirection, CadDrawingViewMode,
        CadHiddenLineMode, CadHotkeyAction, CadProjectionMode, CadSectionAxis, CadSnapMode,
        CadThreeDMouseAxis, CadThreeDMouseMode, CadThreeDMouseProfile, EarningsScoreboardState,
        JobHistoryState, JobHistoryStatus, JobHistoryStatusFilter, JobHistoryTimeRange,
        JobInboxDecision, JobInboxNetworkRequest, JobInboxState, JobInboxValidation,
        JobLifecycleStage, NetworkRequestStatus, NetworkRequestSubmission, NetworkRequestsState,
        NostrSecretState, ProviderMode, ProviderRuntimeState, RecoveryAlertRow, RelayConnectionRow,
        RelayConnectionStatus, RelayConnectionsState, RelaySecuritySimulationPaneState,
        SettingsState, SparkPaneState, StableSatsSimulationPaneState, StarterJobRow,
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
            request_kind: 5050,
            capability: capability.to_string(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some(format!("req-event:{request_id}")),
            sa_tick_result_event_id: Some(format!("result-event:{request_id}")),
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
    fn chat_state_records_turn_metadata_and_binds_to_started_turn() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("design a rack".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            true,
            "keyword-pair:design+rack",
            1000,
            Vec::new(),
        );

        let pending = chat
            .active_turn_metadata()
            .expect("latest submitted metadata should be available");
        assert!(pending.is_cad_turn);
        assert_eq!(pending.classifier_reason, "keyword-pair:design+rack");

        chat.mark_turn_started("turn-1".to_string());
        let bound = chat
            .turn_metadata_for("turn-1")
            .expect("turn metadata should bind when turn starts");
        assert!(bound.is_cad_turn);
        assert_eq!(bound.classifier_reason, "keyword-pair:design+rack");
        assert_eq!(chat.active_turn_metadata(), Some(bound));
    }

    #[test]
    fn chat_state_updates_pending_turn_selected_skills_for_audit_capture() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("earn bitcoin".to_string());
        chat.record_turn_submission_metadata("thread-1", false, "no-cad-signals", 1000, Vec::new());

        chat.set_last_pending_turn_selected_skills(vec![
            "blink".to_string(),
            "l402".to_string(),
            "blink".to_string(),
        ]);

        let pending = chat
            .pending_turn_metadata
            .back()
            .expect("pending metadata should remain queued");
        assert_eq!(
            pending.selected_skill_names,
            vec!["blink".to_string(), "l402".to_string()]
        );
    }

    #[test]
    fn chat_state_binds_turn_metadata_in_submission_order() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());

        chat.submit_prompt("summarize commits".to_string());
        chat.record_turn_submission_metadata("thread-1", false, "no-cad-signals", 1010, Vec::new());
        chat.submit_prompt("design wall mount bracket".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            true,
            "keyword-pair:design+bracket",
            1020,
            Vec::new(),
        );

        chat.mark_turn_started("turn-a".to_string());
        chat.mark_turn_started("turn-b".to_string());

        let first = chat
            .turn_metadata_for("turn-a")
            .expect("first turn metadata should bind");
        assert!(!first.is_cad_turn);
        assert_eq!(first.classifier_reason, "no-cad-signals");

        let second = chat
            .turn_metadata_for("turn-b")
            .expect("second turn metadata should bind");
        assert!(second.is_cad_turn);
        assert_eq!(second.classifier_reason, "keyword-pair:design+bracket");
    }

    #[test]
    fn chat_state_drops_pending_metadata_when_dispatch_fails() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("design fixture".to_string());
        chat.record_turn_submission_metadata(
            "thread-1",
            true,
            "keyword-pair:design+fixture",
            1200,
            Vec::new(),
        );
        assert_eq!(chat.pending_turn_metadata.len(), 1);

        chat.mark_pending_turn_dispatch_failed("queue failed");
        assert!(chat.pending_turn_metadata.is_empty());
    }

    #[test]
    fn chat_copy_notice_expires() {
        let mut chat = AutopilotChatState::default();
        let now = std::time::Instant::now();
        chat.set_copy_notice(now, "Copied message".to_string());
        assert_eq!(chat.copy_notice.as_deref(), Some("Copied message"));

        let expired_at = now + std::time::Duration::from_secs(4);
        assert!(chat.expire_copy_notice(expired_at));
        assert!(chat.copy_notice.is_none());
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
    fn chat_state_binds_turns_to_assistant_slots_in_submission_order() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("first".to_string());
        chat.submit_prompt("second".to_string());
        let queued_assistant_ids = chat
            .messages
            .iter()
            .filter(|message| {
                message.role == AutopilotRole::Codex
                    && message.status == AutopilotMessageStatus::Queued
            })
            .map(|message| message.id)
            .collect::<Vec<_>>();
        assert_eq!(queued_assistant_ids.len(), 2);

        // The first turn starts after a second prompt was already queued.
        chat.mark_turn_started("turn-1".to_string());
        chat.append_turn_delta_for_turn("turn-1", "resp-first");
        chat.mark_turn_completed_for("turn-1");

        // First assistant slot should carry first turn output.
        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == queued_assistant_ids[0])
                .map(|message| message.content.as_str()),
            Some("resp-first")
        );
        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == queued_assistant_ids[1])
                .map(|message| message.status),
            Some(AutopilotMessageStatus::Queued)
        );

        chat.mark_turn_started("turn-2".to_string());
        chat.append_turn_delta_for_turn("turn-2", "resp-second");
        chat.mark_turn_completed_for("turn-2");

        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == queued_assistant_ids[1])
                .map(|message| message.content.as_str()),
            Some("resp-second")
        );
        assert!(!chat.has_pending_messages());
    }

    #[test]
    fn chat_state_preserves_turn_binding_after_completion_for_late_events() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("first".to_string());
        let first_assistant_id = chat
            .messages
            .iter()
            .rev()
            .find(|message| {
                message.role == AutopilotRole::Codex
                    && message.status == AutopilotMessageStatus::Queued
            })
            .map(|message| message.id)
            .unwrap_or_default();

        chat.mark_turn_started("turn-1".to_string());
        chat.append_turn_delta_for_turn("turn-1", "resp");
        chat.mark_turn_completed_for("turn-1");

        // Queue another prompt so there is a fresh pending assistant slot.
        chat.submit_prompt("second".to_string());
        chat.append_turn_delta_for_turn("turn-1", "-late");

        assert_eq!(
            chat.messages
                .iter()
                .find(|message| message.id == first_assistant_id)
                .map(|message| message.content.as_str()),
            Some("resp-late")
        );
    }

    #[test]
    fn chat_state_dedupes_agent_deltas_with_fallback_item_ids() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("hello".to_string());
        chat.mark_turn_started("turn-1".to_string());

        assert!(!chat.is_duplicate_agent_delta("turn-1", "item-1", "abc"));
        assert!(chat.is_duplicate_agent_delta("turn-1", "item-1", "abc"));
        assert!(chat.is_duplicate_agent_delta("turn-1", "event-agent-message", "abc"));
        assert!(!chat.is_duplicate_agent_delta("turn-1", "item-1", "def"));
    }

    #[test]
    fn chat_state_dedupes_reasoning_deltas_with_na_item_id() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("hello".to_string());
        chat.mark_turn_started("turn-1".to_string());

        assert!(!chat.is_duplicate_reasoning_delta("turn-1", "rs-1", "plan"));
        assert!(chat.is_duplicate_reasoning_delta("turn-1", "rs-1", "plan"));
        assert!(chat.is_duplicate_reasoning_delta("turn-1", "n/a", "plan"));
        assert!(!chat.is_duplicate_reasoning_delta("turn-1", "rs-1", "next"));
    }

    #[test]
    fn chat_state_preserves_preformatted_reasoning_answer_messages() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-1".to_string());
        chat.submit_prompt("hello".to_string());
        chat.mark_turn_started("turn-1".to_string());

        let combined = "Reasoning:\n**Check**\n\nplan\n\nAnswer:\nDone.";
        chat.set_turn_message_for_turn("turn-1", combined);
        let assistant_id = *chat
            .turn_assistant_message_ids
            .get("turn-1")
            .expect("turn should bind to an assistant message");
        let codex = chat
            .messages
            .iter()
            .find(|message| message.id == assistant_id)
            .expect("codex message should exist");
        assert_eq!(codex.content, combined);
    }

    #[test]
    fn chat_state_prefers_coding_model_over_server_default() {
        let mut chat = AutopilotChatState::default();
        chat.set_models(
            vec![
                "gpt-5.3-codex".to_string(),
                "gpt-5.2-codex".to_string(),
                "gpt-5.2".to_string(),
            ],
            Some("gpt-5.3-codex".to_string()),
        );
        assert_eq!(chat.current_model(), "gpt-5.2-codex");
    }

    #[test]
    fn chat_state_preserves_explicit_user_model_selection() {
        let mut chat = AutopilotChatState::default();
        chat.set_models(
            vec![
                "gpt-5.3-codex".to_string(),
                "gpt-5.2-codex".to_string(),
                "gpt-5.2".to_string(),
            ],
            Some("gpt-5.3-codex".to_string()),
        );
        // Simulate user cycling to a specific model.
        chat.selected_model = 0; // gpt-5.3-codex
        chat.set_models(
            vec![
                "gpt-5.3-codex".to_string(),
                "gpt-5.2-codex".to_string(),
                "gpt-5.2".to_string(),
            ],
            Some("gpt-5.3-codex".to_string()),
        );
        assert_eq!(chat.current_model(), "gpt-5.3-codex");
    }

    #[test]
    fn chat_state_preserves_active_thread_when_list_omits_it() {
        let mut chat = AutopilotChatState::default();
        chat.ensure_thread("thread-active".to_string());
        chat.set_thread_name("thread-active", Some("Active".to_string()));
        chat.set_thread_status("thread-active", Some("active".to_string()));

        chat.set_thread_entries(vec![super::AutopilotThreadListEntry {
            thread_id: "thread-other".to_string(),
            thread_name: Some("Other".to_string()),
            status: Some("idle".to_string()),
            loaded: false,
            cwd: Some("/tmp/other".to_string()),
            path: None,
        }]);

        assert_eq!(chat.active_thread_id.as_deref(), Some("thread-active"));
        assert!(
            chat.threads
                .iter()
                .any(|thread_id| thread_id == "thread-active")
        );
        assert_eq!(
            chat.thread_metadata
                .get("thread-active")
                .and_then(|metadata| metadata.thread_name.as_deref()),
            Some("Active")
        );
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
        assert!(current.invoice_id.is_none());
        assert!(current.events.iter().any(|event| {
            event
                .message
                .contains("running (authority=req-event:req-active)")
        }));
        assert!(current.events.iter().any(|event| {
            event
                .message
                .contains("delivered (authority=result-event:req-active)")
        }));
    }

    #[test]
    fn job_history_rejects_unconfirmed_success_settlement_from_active_job() {
        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-unconfirmed",
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
        active
            .advance_stage()
            .expect("accepted->running should succeed");
        active
            .advance_stage()
            .expect("running->delivered should succeed");
        let paid_transition = active.advance_stage();
        assert!(paid_transition.is_err());
        assert!(
            paid_transition
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("payment pointer"))
        );
        let job = active.job.as_ref().expect("active job exists");
        assert!(job.payment_id.is_none());
        assert_eq!(job.stage, JobLifecycleStage::Delivered);

        let mut history = JobHistoryState::default();
        history.record_from_active_job(job, JobHistoryStatus::Succeeded);
        let row = history
            .rows
            .first()
            .expect("history row should be recorded");
        assert_eq!(row.status, JobHistoryStatus::Failed);
        assert_eq!(row.payout_sats, 0);
        assert!(
            row.failure_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("not wallet-confirmed"))
        );
    }

    #[test]
    fn active_job_rejects_running_without_authoritative_request_event() {
        let mut request = fixture_inbox_request(
            "req-missing-running-authority",
            "summarize.text",
            500,
            90,
            JobInboxValidation::Valid,
        );
        request.sa_tick_request_event_id = None;

        let mut active = ActiveJobState::default();
        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        active.start_from_request(&selected);

        let outcome = active.advance_stage();
        assert!(outcome.is_err());
        assert!(
            outcome
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("running event"))
        );
        assert_eq!(
            active.job.as_ref().expect("job should exist").stage,
            JobLifecycleStage::Accepted
        );
    }

    #[test]
    fn active_job_rejects_delivered_without_authoritative_result_event() {
        let mut request = fixture_inbox_request(
            "req-missing-delivered-authority",
            "summarize.text",
            500,
            90,
            JobInboxValidation::Valid,
        );
        request.sa_tick_result_event_id = None;

        let mut active = ActiveJobState::default();
        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        active.start_from_request(&selected);
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running should succeed"),
            JobLifecycleStage::Running
        );

        let outcome = active.advance_stage();
        assert!(outcome.is_err());
        assert!(
            outcome
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("delivered event"))
        );
        assert_eq!(
            active.job.as_ref().expect("job should exist").stage,
            JobLifecycleStage::Running
        );
    }

    #[test]
    fn active_job_rejects_paid_without_wallet_authority() {
        let request = fixture_inbox_request(
            "req-missing-payment-authority",
            "summarize.text",
            500,
            90,
            JobInboxValidation::Valid,
        );
        let mut active = ActiveJobState::default();
        let mut inbox = seed_job_inbox(vec![request]);
        assert!(inbox.select_by_index(0));
        let selected = inbox
            .selected_request()
            .expect("request should exist")
            .clone();
        active.start_from_request(&selected);
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running should succeed"),
            JobLifecycleStage::Running
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("running->delivered should succeed"),
            JobLifecycleStage::Delivered
        );

        let outcome = active.advance_stage();
        assert!(outcome.is_err());
        assert!(
            outcome
                .err()
                .as_deref()
                .is_some_and(|error| error.contains("payment pointer"))
        );
        assert_eq!(
            active.job.as_ref().expect("job should exist").stage,
            JobLifecycleStage::Delivered
        );
    }

    #[test]
    fn mission_control_earn_loop_wallet_confirmed_end_to_end() {
        let mut provider = ProviderRuntimeState::default();
        provider.mode = ProviderMode::Online;
        provider.online_since = Some(std::time::Instant::now());

        let mut inbox = seed_job_inbox(vec![fixture_inbox_request(
            "req-e2e",
            "run_model",
            50,
            120,
            JobInboxValidation::Valid,
        )]);
        assert!(inbox.select_by_index(0));
        let selected_request_id = inbox
            .decide_selected(true, "capability matched")
            .expect("accept request");
        assert_eq!(selected_request_id, "req-e2e");
        let request = inbox
            .selected_request()
            .expect("selected request exists")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&request);
        assert_eq!(
            active.job.as_ref().expect("active job").stage,
            JobLifecycleStage::Accepted
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running transition"),
            JobLifecycleStage::Running
        );
        assert_eq!(
            active
                .advance_stage()
                .expect("running->delivered transition"),
            JobLifecycleStage::Delivered
        );
        active.job.as_mut().expect("active job").payment_id =
            Some("wallet-payment-001".to_string());
        assert_eq!(
            active.advance_stage().expect("delivered->paid transition"),
            JobLifecycleStage::Paid
        );
        let terminal_job = active.job.clone().expect("terminal active job");

        let mut history = JobHistoryState::default();
        history.record_from_active_job(&terminal_job, JobHistoryStatus::Succeeded);
        let row = history.rows.first().expect("history row recorded");
        assert_eq!(row.status, JobHistoryStatus::Succeeded);
        assert_eq!(row.payout_sats, 50);
        assert_eq!(row.payment_pointer, "wallet-payment-001");

        let mut wallet = SparkPaneState::default();
        wallet.balance = Some(openagents_spark::Balance {
            spark_sats: 10_000,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        wallet
            .recent_payments
            .push(openagents_spark::PaymentSummary {
                id: "wallet-payment-001".to_string(),
                direction: "receive".to_string(),
                status: "succeeded".to_string(),
                amount_sats: 50,
                timestamp: history.reference_epoch_seconds,
            });

        let swap_receipts = Vec::<crate::state::swap_contract::GoalSwapExecutionReceipt>::new();
        let reconciliation = crate::state::wallet_reconciliation::reconcile_wallet_events_for_goal(
            history.reference_epoch_seconds.saturating_sub(60),
            9_950,
            10_000,
            "goal-e2e",
            &history,
            &wallet,
            &swap_receipts,
        );
        assert_eq!(reconciliation.earned_wallet_delta_sats, 50);
        assert_eq!(reconciliation.unattributed_receive_sats, 0);

        let mut score = EarningsScoreboardState::default();
        score.refresh_from_sources(std::time::Instant::now(), &provider, &history, &wallet);
        assert_eq!(score.load_state, super::PaneLoadState::Ready);
        assert!(score.sats_today >= 50);
        assert_eq!(score.jobs_today, 1);
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
        let mut provider = ProviderRuntimeState::default();
        provider.mode = ProviderMode::Online;
        let mut relays = RelayConnectionsState::default();
        relays.relays.push(RelayConnectionRow {
            url: "wss://relay-a.example".to_string(),
            status: RelayConnectionStatus::Connected,
            latency_ms: Some(42),
            last_seen_seconds_ago: Some(0),
            last_error: None,
        });
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
    fn sync_health_does_not_mark_stale_while_provider_is_offline() {
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

        sync.cursor_last_advanced_seconds_ago = sync.cursor_stale_after_seconds + 5;
        sync.refresh_from_runtime(std::time::Instant::now(), &provider, &relays);

        assert_eq!(sync.load_state, super::PaneLoadState::Ready);
        assert_eq!(sync.recovery_phase, SyncRecoveryPhase::Idle);
        assert_eq!(sync.last_error, None);
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
    fn stablesats_simulation_rounds_switch_agent_wallet_modes() {
        let mut state = StableSatsSimulationPaneState::default();
        let initial_modes: Vec<_> = state
            .agents
            .iter()
            .map(|agent| agent.active_wallet)
            .collect();
        let initial_total_usd = state.total_usd_balance_cents();

        state
            .run_round(1_761_921_100)
            .expect("stablesats simulation round should succeed");

        assert_eq!(state.rounds_run, 1);
        assert!(state.last_settlement_ref.is_some());
        assert!(state.total_converted_sats > 0);
        assert!(state.total_converted_usd_cents > 0);
        assert_eq!(state.price_history_usd_cents_per_btc.len(), 1);
        assert_eq!(state.converted_sats_history.len(), 1);
        assert!(
            state
                .events
                .iter()
                .any(|event| event.protocol == "BLINK-PRICE")
        );
        assert!(
            state
                .events
                .iter()
                .any(|event| event.protocol == "BLINK-SWAP")
        );
        assert!(
            state
                .events
                .iter()
                .any(|event| event.protocol == "BLINK-LEDGER")
        );
        assert!(!state.transfer_ledger.is_empty());
        assert!(
            state
                .transfer_ledger
                .iter()
                .all(|entry| entry.status == crate::app_state::StableSatsTransferStatus::Settled)
        );

        let next_modes: Vec<_> = state
            .agents
            .iter()
            .map(|agent| agent.active_wallet)
            .collect();
        assert_ne!(initial_modes, next_modes);
        assert_ne!(state.total_usd_balance_cents(), initial_total_usd);

        state.reset();
        assert_eq!(state.rounds_run, 0);
        assert!(state.events.is_empty());
        assert!(state.last_settlement_ref.is_none());
        assert!(state.price_history_usd_cents_per_btc.is_empty());
        assert!(state.converted_sats_history.is_empty());
        assert!(state.transfer_ledger.is_empty());
    }

    #[test]
    fn stablesats_real_mode_initializes_single_wallet_topology() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        assert_eq!(state.agents.len(), 1);
        assert_eq!(
            state.agents[0].owner_kind,
            crate::app_state::StableSatsWalletOwnerKind::Operator
        );
        assert!(
            state.agents[0]
                .credential_key_name
                .starts_with("BLINK_API_KEY")
        );
    }

    #[test]
    fn stablesats_live_snapshot_updates_operator_wallet_and_ledger() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        state.apply_live_snapshot(1_761_921_200, 2_000, 250, 8_500_000, "btc:op usd:op");
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.agents[0].agent_name, "autopilot-user");
        assert_eq!(state.agents[0].btc_balance_sats, 2_000);
        assert_eq!(state.agents[0].usd_balance_cents, 250);
        assert!(!state.transfer_ledger.is_empty());
        assert!(
            state
                .transfer_ledger
                .iter()
                .all(|entry| entry.transfer_ref.starts_with("blink:live:transfer:"))
        );
    }

    #[test]
    fn stablesats_live_wallet_snapshots_apply_partial_refresh_without_global_failure() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        state.apply_live_wallet_snapshots(
            1_761_921_260,
            8_600_000,
            &[(
                "operator:autopilot".to_string(),
                1_500,
                80,
                "btc:op usd:op".to_string(),
            )],
            &[(
                "operator:autopilot".to_string(),
                "missing secure credential".to_string(),
            )],
        );

        assert_eq!(state.load_state, crate::app_state::PaneLoadState::Ready);
        assert_eq!(state.agents[0].btc_balance_sats, 1_500);
        assert_eq!(state.agents[0].usd_balance_cents, 80);
        assert_eq!(
            state.agents[0].last_switch_summary,
            "refresh failed: missing secure credential"
        );
        assert!(
            state
                .last_error
                .as_deref()
                .is_some_and(|value| value.contains("1 wallet error"))
        );
        assert!(!state.transfer_ledger.is_empty());
    }

    #[test]
    fn stablesats_treasury_operation_log_tracks_refresh_lifecycle() {
        let mut state = StableSatsSimulationPaneState::default();
        state.set_mode(crate::app_state::StableSatsSimulationMode::RealBlink);
        let request_id = state
            .begin_live_refresh()
            .expect("real mode refresh should start");
        let running_entry = state
            .treasury_operations
            .iter()
            .find(|entry| entry.request_id == request_id)
            .expect("refresh operation should be recorded");
        assert_eq!(
            running_entry.kind,
            crate::app_state::StableSatsTreasuryOperationKind::Refresh
        );
        assert_eq!(
            running_entry.status,
            crate::app_state::StableSatsTreasuryOperationStatus::Running
        );

        assert!(state.fail_live_refresh(request_id, "timeout".to_string()));
        let finished_entry = state
            .treasury_operations
            .iter()
            .find(|entry| entry.request_id == request_id)
            .expect("refresh operation should remain recorded");
        assert_eq!(
            finished_entry.status,
            crate::app_state::StableSatsTreasuryOperationStatus::Failed
        );
    }

    #[test]
    fn starter_jobs_complete_selected_requires_wallet_pointer() {
        let mut starter_jobs = StarterJobsState::default();
        starter_jobs.jobs.push(fixture_starter_job(
            "job-starter-001",
            1200,
            true,
            StarterJobStatus::Queued,
        ));
        starter_jobs.select_by_index(0);
        let (job_id, _payout) = starter_jobs
            .start_selected_execution()
            .expect("eligible starter job should start");
        let (_job_id, _payout, pointer) = starter_jobs
            .complete_selected_with_payment("wallet:payment:starter-001")
            .expect("wallet-confirmed payout should complete");
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

        feed.upsert_event(fixture_activity_event(
            "cad:event:1",
            ActivityEventDomain::Cad,
            1_761_920_260,
        ));
        feed.set_filter(ActivityFeedFilter::Cad);
        assert!(
            feed.visible_rows()
                .into_iter()
                .all(|row| row.domain == ActivityEventDomain::Cad)
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

    #[test]
    fn cad_demo_state_defaults_are_deterministic() {
        let state = CadDemoPaneState::default();
        assert_eq!(state.load_state, super::PaneLoadState::Ready);
        assert_eq!(state.session_id, "cad.session.local");
        assert!(state.active_chat_session_id.is_none());
        assert!(state.chat_thread_session_bindings.is_empty());
        assert!(state.dispatch_sessions.is_empty());
        assert!(state.last_chat_intent_name.is_none());
        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        assert!(state.build_session.failure_class.is_none());
        assert_eq!(state.build_session.retry_attempts, 0);
        assert_eq!(state.build_session.retry_limit, 0);
        assert!(state.build_session.events.is_empty());
        assert!(state.last_build_session.is_none());
        assert_eq!(state.build_failure_metrics.tool_transport_failures, 0);
        assert_eq!(state.build_failure_metrics.intent_parse_failures, 0);
        assert_eq!(state.build_failure_metrics.dispatch_rebuild_failures, 0);
        assert_eq!(state.build_failure_metrics.tool_transport_retries, 0);
        assert_eq!(state.build_failure_metrics.intent_parse_retries, 0);
        assert_eq!(state.build_failure_metrics.dispatch_rebuild_retries, 0);
        assert_eq!(state.build_failure_metrics.terminal_failures, 0);
        assert_eq!(state.document_id, "cad.doc.demo-rack");
        assert_eq!(state.document_revision, 0);
        assert_eq!(state.active_variant_id, "variant.baseline");
        assert_eq!(state.variant_ids.len(), 4);
        assert_eq!(state.variant_ids[0], "variant.baseline");
        assert_eq!(state.active_variant_tile_index, 0);
        assert_eq!(state.variant_viewports.len(), 4);
        assert_eq!(state.variant_viewports[0].variant_id, "variant.baseline");
        assert!(state.last_rebuild_receipt.is_none());
        assert!(state.rebuild_receipts.is_empty());
        assert_eq!(state.eval_cache.len(), 0);
        assert!(state.rebuild_worker.is_none());
        assert_eq!(state.next_rebuild_request_id, 1);
        assert!(state.pending_rebuild_request_id.is_none());
        assert!(state.last_good_mesh_id.is_none());
        assert!(state.last_good_mesh_payload.is_none());
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-6061-t6")
        );
        assert_eq!(state.variant_analysis_snapshots.len(), 4);
        assert!(state.warnings.is_empty());
        assert_eq!(state.variant_warning_sets.len(), 4);
        assert_eq!(state.warning_filter_severity, "all");
        assert_eq!(state.warning_filter_code, "all");
        assert!(state.warning_hover_index.is_none());
        assert!(state.focused_warning_index.is_none());
        assert!(state.focused_geometry_ref.is_none());
        assert_eq!(state.hidden_line_mode, CadHiddenLineMode::Shaded);
        assert!(state.section_axis.is_none());
        assert_eq!(state.section_offset_normalized, 0.0);
        assert_eq!(state.section_summary(), "off");
        assert!(state.snap_toggles.grid);
        assert!(state.snap_toggles.origin);
        assert!(!state.snap_toggles.endpoint);
        assert!(!state.snap_toggles.midpoint);
        assert_eq!(state.projection_mode, CadProjectionMode::Orthographic);
        assert_eq!(state.drawing_view_mode, CadDrawingViewMode::ThreeD);
        assert_eq!(state.drawing_view_direction, CadDrawingViewDirection::Front);
        assert!(state.drawing_show_hidden_lines);
        assert!(state.drawing_show_dimensions);
        assert_eq!(state.drawing_zoom, 1.0);
        assert_eq!(state.drawing_pan_x, 0.0);
        assert_eq!(state.drawing_pan_y, 0.0);
        assert!(state.drawing_detail_views.is_empty());
        assert_eq!(state.drawing_next_detail_id, 1);
        assert_eq!(state.hotkey_profile, "default");
        assert_eq!(state.hotkeys.snap_top, "t");
        assert_eq!(state.hotkeys.toggle_projection, "p");
        assert_eq!(state.three_d_mouse_mode, CadThreeDMouseMode::Translate);
        assert_eq!(state.three_d_mouse_profile, CadThreeDMouseProfile::Balanced);
        assert_eq!(state.three_d_mouse_status(), "absent");
        assert_eq!(state.camera_zoom, 1.0);
        assert_eq!(state.camera_pan_x, 0.0);
        assert_eq!(state.camera_pan_y, 0.0);
        assert_eq!(state.camera_orbit_yaw_deg, 26.0);
        assert_eq!(state.camera_orbit_pitch_deg, 18.0);
        assert_eq!(state.history_stack.session_id, "cad.session.local");
        assert_eq!(state.history_stack.len_undo(), 0);
        assert!(state.timeline_rows.is_empty());
        assert!(state.timeline_selected_index.is_none());
        assert_eq!(state.timeline_scroll_offset, 0);
        assert!(state.selected_feature_params.is_empty());
        assert!(!state.context_menu.is_open);
        assert!(state.context_menu.items.is_empty());
    }

    #[test]
    fn cad_build_session_valid_transitions_archive_terminal_state() {
        let mut state = CadDemoPaneState::default();
        state
            .begin_agent_build_session("thread-1", "turn-1")
            .expect("build session should start");
        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Planning);
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Applying,
                "cad.build.applying.start",
                "tool executing".to_string(),
            )
            .expect("planning -> applying should be valid");
        state.record_agent_build_tool_result("OA-CAD-INTENT-OK", true, "intent applied");
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Rebuilding,
                "cad.build.rebuilding.wait",
                "waiting for background rebuild".to_string(),
            )
            .expect("applying -> rebuilding should be valid");
        state.record_agent_build_rebuild_result("ai-intent:setmaterial", "ok");
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Summarizing,
                "cad.build.summarizing.start",
                "rebuild committed".to_string(),
            )
            .expect("rebuilding -> summarizing should be valid");
        state
            .complete_agent_build_session("build complete".to_string())
            .expect("summarizing -> done should be valid");

        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        let archived = state
            .last_build_session
            .as_ref()
            .expect("completed session should be archived");
        assert_eq!(archived.thread_id, "thread-1");
        assert_eq!(archived.turn_id, "turn-1");
        assert_eq!(archived.terminal_phase, CadBuildSessionPhase::Done);
        assert!(
            archived
                .latest_tool_result
                .as_deref()
                .is_some_and(|value| value.contains("OA-CAD-INTENT-OK"))
        );
        assert!(
            archived
                .latest_rebuild_result
                .as_deref()
                .is_some_and(|value| value.contains("ai-intent:setmaterial"))
        );
        assert!(!archived.events.is_empty());
    }

    #[test]
    fn cad_build_session_rejects_invalid_phase_transition() {
        let mut state = CadDemoPaneState::default();
        let error = state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Applying,
                "cad.build.applying.invalid",
                "invalid jump".to_string(),
            )
            .expect_err("idle -> applying must be rejected");
        assert!(error.contains("invalid CAD build phase transition"));
        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        assert!(state.build_session.events.is_empty());
    }

    #[test]
    fn cad_build_session_failure_archives_reason_and_remediation() {
        let mut state = CadDemoPaneState::default();
        state
            .begin_agent_build_session("thread-2", "turn-2")
            .expect("build session should start");
        state
            .transition_agent_build_phase(
                CadBuildSessionPhase::Applying,
                "cad.build.applying.start",
                "tool executing".to_string(),
            )
            .expect("planning -> applying should be valid");
        state.set_agent_build_failure_context(CadBuildFailureClass::IntentParseValidation, 1, 1);
        state
            .fail_agent_build_session(
                "cad.build.tool.failed",
                "tool returned parse error".to_string(),
                Some("retry with explicit rack dimensions".to_string()),
            )
            .expect("applying -> failed should be valid");

        assert_eq!(state.build_session.phase, CadBuildSessionPhase::Idle);
        let archived = state
            .last_build_session
            .as_ref()
            .expect("failed session should be archived");
        assert_eq!(archived.terminal_phase, CadBuildSessionPhase::Failed);
        assert_eq!(
            archived.failure_class,
            Some(CadBuildFailureClass::IntentParseValidation)
        );
        assert_eq!(archived.retry_attempts, 1);
        assert_eq!(archived.retry_limit, 1);
        assert_eq!(
            archived.failure_reason.as_deref(),
            Some("tool returned parse error")
        );
        assert_eq!(
            archived.remediation_hint.as_deref(),
            Some("retry with explicit rack dimensions")
        );
        assert_eq!(state.build_failure_metrics.terminal_failures, 1);
    }

    #[test]
    fn cad_camera_methods_are_deterministic_and_clamped() {
        let mut first = CadDemoPaneState::default();
        first.orbit_camera_by_drag(480.0, -1200.0);
        first.pan_camera_by_drag(1200.0, -1600.0);
        first.zoom_camera_by_scroll(-10_000.0);

        let mut second = CadDemoPaneState::default();
        second.orbit_camera_by_drag(480.0, -1200.0);
        second.pan_camera_by_drag(1200.0, -1600.0);
        second.zoom_camera_by_scroll(-10_000.0);

        assert_eq!(first.camera_orbit_yaw_deg, second.camera_orbit_yaw_deg);
        assert_eq!(first.camera_orbit_pitch_deg, second.camera_orbit_pitch_deg);
        assert_eq!(first.camera_pan_x, second.camera_pan_x);
        assert_eq!(first.camera_pan_y, second.camera_pan_y);
        assert_eq!(first.camera_zoom, second.camera_zoom);
        assert!(first.camera_orbit_pitch_deg < 180.0);
        assert!(first.camera_orbit_pitch_deg >= -180.0);
        assert!((first.camera_orbit_pitch_deg + 6.0).abs() <= 0.001);
        assert!(first.camera_pan_x <= 800.0);
        assert!(first.camera_pan_y >= -800.0);
        assert!(first.camera_zoom <= 1.0);
        assert!(first.camera_zoom >= 0.35);
    }

    #[test]
    fn cad_camera_view_snaps_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        first.snap_camera_to_view(CadCameraViewSnap::Top);
        first.snap_camera_to_view(CadCameraViewSnap::Right);
        first.snap_camera_to_view(CadCameraViewSnap::Isometric);

        let mut second = CadDemoPaneState::default();
        second.snap_camera_to_view(CadCameraViewSnap::Top);
        second.snap_camera_to_view(CadCameraViewSnap::Right);
        second.snap_camera_to_view(CadCameraViewSnap::Isometric);

        assert_eq!(first.camera_orbit_yaw_deg, second.camera_orbit_yaw_deg);
        assert_eq!(first.camera_orbit_pitch_deg, second.camera_orbit_pitch_deg);
        assert_eq!(first.camera_pan_x, second.camera_pan_x);
        assert_eq!(first.camera_pan_y, second.camera_pan_y);
        assert_eq!(first.active_view_snap(), Some(CadCameraViewSnap::Isometric));
    }

    #[test]
    fn cad_projection_mode_cycles_deterministically() {
        let mut first = CadDemoPaneState::default();
        first.cycle_projection_mode();
        first.cycle_projection_mode();

        let mut second = CadDemoPaneState::default();
        second.cycle_projection_mode();
        second.cycle_projection_mode();

        assert_eq!(first.projection_mode, second.projection_mode);
        assert_eq!(first.projection_mode, CadProjectionMode::Orthographic);
        second.cycle_projection_mode();
        assert_eq!(second.projection_mode, CadProjectionMode::Perspective);
    }

    #[test]
    fn cad_drawing_mode_state_transitions_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        let mut second = CadDemoPaneState::default();

        for state in [&mut first, &mut second] {
            assert_eq!(state.toggle_drawing_view_mode(), CadDrawingViewMode::TwoD);
            assert_eq!(
                state.cycle_drawing_view_direction(),
                CadDrawingViewDirection::Back
            );
            assert!(!state.toggle_drawing_hidden_lines());
            assert!(!state.toggle_drawing_dimensions());
            state.pan_drawing_view_by_drag(22.0, -14.0);
            state.zoom_drawing_view_by_scroll(-280.0);
            let detail = state.add_drawing_detail_view();
            assert_eq!(detail.detail_id, "detail-1");
            assert_eq!(detail.label, "A");
            assert_eq!(state.drawing_detail_views.len(), 1);
            let cleared = state.clear_drawing_detail_views();
            assert_eq!(cleared, 1);
            state.reset_drawing_view();
        }

        assert_eq!(first.drawing_view_mode, second.drawing_view_mode);
        assert_eq!(first.drawing_view_direction, second.drawing_view_direction);
        assert_eq!(
            first.drawing_show_hidden_lines,
            second.drawing_show_hidden_lines
        );
        assert_eq!(
            first.drawing_show_dimensions,
            second.drawing_show_dimensions
        );
        assert_eq!(first.drawing_zoom, second.drawing_zoom);
        assert_eq!(first.drawing_pan_x, second.drawing_pan_x);
        assert_eq!(first.drawing_pan_y, second.drawing_pan_y);
        assert!(first.drawing_detail_views.is_empty());
        assert_eq!(first.drawing_next_detail_id, 2);
    }

    #[test]
    fn cad_section_mode_and_offset_cycle_deterministically() {
        let mut state = CadDemoPaneState::default();
        assert!(state.section_axis.is_none());

        assert_eq!(state.cycle_section_axis(), Some(CadSectionAxis::X));
        assert_eq!(state.section_summary(), "x/0");
        assert_eq!(state.step_section_offset(), 0.2);
        assert_eq!(state.step_section_offset(), 0.4);
        assert_eq!(state.step_section_offset(), -0.4);

        assert_eq!(state.cycle_section_axis(), Some(CadSectionAxis::Y));
        assert_eq!(state.cycle_section_axis(), Some(CadSectionAxis::Z));
        assert_eq!(state.cycle_section_axis(), None);
        assert_eq!(state.section_offset_normalized, 0.0);
        assert!(state.section_plane().is_none());
    }

    #[test]
    fn cad_material_cycle_is_deterministic() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-6061-t6")
        );
        assert_eq!(state.cycle_material_preset(), "al-5052-h32");
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-5052-h32")
        );
        assert_eq!(state.cycle_material_preset(), "steel-1018");
        assert_eq!(state.cycle_material_preset(), "ti-6al-4v");
        assert_eq!(state.cycle_material_preset(), "al-6061-t6");
    }

    #[test]
    fn cad_dimension_editing_is_typed_and_bounded() {
        let mut state = CadDemoPaneState::default();
        assert!(state.begin_dimension_edit(0));
        assert!(state.dimension_edit.is_some());

        // Replace default draft with a typed value.
        for _ in 0..16 {
            let _ = state.backspace_dimension_edit();
        }
        assert!(state.append_dimension_edit_char('4'));
        assert!(state.append_dimension_edit_char('2'));
        assert!(state.append_dimension_edit_char('0'));
        assert!(state.append_dimension_edit_char('.'));
        assert!(state.append_dimension_edit_char('5'));

        let (dimension_id, before, after) = state
            .commit_dimension_edit()
            .expect("dimension commit should succeed");
        assert_eq!(dimension_id, "width_mm");
        assert_eq!(before, 390.0);
        assert_eq!(after, 420.5);
        assert!(state.dimension_edit.is_none());
        assert_eq!(state.dimension_value_mm("width_mm"), Some(420.5));

        assert!(state.begin_dimension_edit(3));
        for _ in 0..16 {
            let _ = state.backspace_dimension_edit();
        }
        assert!(state.append_dimension_edit_char('9'));
        assert!(state.append_dimension_edit_char('9'));
        let error = state
            .commit_dimension_edit()
            .expect_err("out-of-range wall edit must fail");
        assert!(error.contains("Wall"));
        assert_eq!(state.dimension_value_mm("wall_mm"), Some(6.0));
    }

    #[test]
    fn cad_assembly_selection_and_editing_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        let mut second = CadDemoPaneState::default();

        for state in [&mut first, &mut second] {
            state
                .select_assembly_instance("arm-1")
                .expect("instance selection should succeed");
            state
                .rename_selected_assembly_instance("Arm Segment".to_string())
                .expect("instance rename should succeed");
            state
                .select_assembly_joint("joint.hinge")
                .expect("joint selection should succeed");
            let semantics = state
                .set_selected_assembly_joint_state(120.0)
                .expect("joint state edit should succeed");
            assert!(semantics.was_clamped);
            assert_eq!(semantics.effective_state, 90.0);
        }

        let first_name = first
            .assembly_schema
            .instances
            .iter()
            .find(|instance| instance.id == "arm-1")
            .and_then(|instance| instance.name.clone());
        let second_name = second
            .assembly_schema
            .instances
            .iter()
            .find(|instance| instance.id == "arm-1")
            .and_then(|instance| instance.name.clone());
        assert_eq!(first_name.as_deref(), Some("Arm Segment"));
        assert_eq!(first_name, second_name);
        assert_eq!(first.assembly_ui_state, second.assembly_ui_state);
    }

    #[test]
    fn cad_snap_modes_and_point_snapping_are_deterministic() {
        let mut first = CadDemoPaneState::default();
        let mut second = CadDemoPaneState::default();
        let viewport = wgpui::Bounds::new(40.0, 30.0, 240.0, 160.0);
        let point = wgpui::Point::new(149.2, 113.7);

        first.toggle_snap_mode(CadSnapMode::Endpoint);
        first.toggle_snap_mode(CadSnapMode::Midpoint);
        second.toggle_snap_mode(CadSnapMode::Endpoint);
        second.toggle_snap_mode(CadSnapMode::Midpoint);

        let snapped_first = first.apply_snap_to_viewport_point(point, viewport);
        let snapped_second = second.apply_snap_to_viewport_point(point, viewport);
        assert_eq!(snapped_first, snapped_second);
        assert_eq!(first.snap_summary(), second.snap_summary());
    }

    #[test]
    fn cad_measurement_points_produce_deterministic_distance_and_angle() {
        let mut state = CadDemoPaneState::default();
        assert!(state.record_measurement_snap_point(0, wgpui::Point::new(100.0, 80.0)));
        assert!(state.record_measurement_snap_point(0, wgpui::Point::new(160.0, 80.0)));

        assert_eq!(state.measurement_tile_index, Some(0));
        assert_eq!(state.measurement_points.len(), 2);
        assert_eq!(state.measurement_distance_px, Some(60.0));
        assert_eq!(state.measurement_angle_deg, Some(0.0));

        assert!(state.record_measurement_snap_point(1, wgpui::Point::new(20.0, 20.0)));
        assert_eq!(state.measurement_tile_index, Some(1));
        assert_eq!(state.measurement_points.len(), 1);
        assert!(state.measurement_distance_px.is_none());
        assert!(state.measurement_angle_deg.is_none());
    }

    #[test]
    fn cad_hotkey_profiles_and_conflict_checks_are_deterministic() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(state.hotkey_profile, "default");
        assert!(state.cycle_hotkey_profile().is_ok());
        assert_eq!(state.hotkey_profile, "compact");
        assert_eq!(state.hotkeys.snap_top, "7");
        assert!(state.hotkey_matches(CadHotkeyAction::SnapTop, "7"));

        let conflict = state.remap_hotkey(CadHotkeyAction::SnapFront, "7");
        assert!(conflict.is_err());

        assert!(state.remap_hotkey(CadHotkeyAction::SnapFront, "2").is_ok());
        assert!(state.hotkey_matches(CadHotkeyAction::SnapFront, "2"));
    }

    #[test]
    fn cad_three_d_mouse_mapping_modes_profiles_and_locks_are_deterministic() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(state.three_d_mouse_status(), "absent");
        assert!(state.apply_three_d_mouse_motion(0, 0.25));
        assert!(state.camera_pan_x > 0.0);

        state.toggle_three_d_mouse_mode();
        assert_eq!(state.three_d_mouse_mode, CadThreeDMouseMode::Rotate);
        let yaw_before = state.camera_orbit_yaw_deg;
        assert!(state.apply_three_d_mouse_motion(3, 0.2));
        assert!(state.camera_orbit_yaw_deg > yaw_before);

        state.cycle_three_d_mouse_profile();
        assert_eq!(state.three_d_mouse_profile, CadThreeDMouseProfile::Fast);
        state.camera_zoom = 0.7;
        let zoom_before = state.camera_zoom;
        assert!(state.apply_three_d_mouse_motion(2, -0.35));
        assert!(state.camera_zoom > zoom_before);

        assert!(state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::Rx));
        let yaw_locked = state.camera_orbit_yaw_deg;
        assert!(!state.apply_three_d_mouse_motion(3, 0.4));
        assert_eq!(state.camera_orbit_yaw_deg, yaw_locked);
    }

    #[test]
    fn cad_context_menu_targeting_and_actions_are_deterministic() {
        let mut state = CadDemoPaneState::default();
        let viewport = wgpui::Bounds::new(40.0, 40.0, 360.0, 220.0);
        let left = wgpui::Point::new(80.0, 120.0);
        let middle = wgpui::Point::new(210.0, 120.0);
        let right = wgpui::Point::new(360.0, 120.0);

        let (kind_left, _) = state.infer_context_menu_target_for_viewport_point(left, viewport);
        let (kind_mid, _) = state.infer_context_menu_target_for_viewport_point(middle, viewport);
        let (kind_right, edge_ref) =
            state.infer_context_menu_target_for_viewport_point(right, viewport);
        assert_eq!(kind_left, CadContextMenuTargetKind::Body);
        assert_eq!(kind_mid, CadContextMenuTargetKind::Face);
        assert_eq!(kind_right, CadContextMenuTargetKind::Edge);

        state.open_context_menu(right, kind_right, edge_ref.clone());
        assert!(state.context_menu.is_open);
        assert_eq!(
            state.context_menu.target_kind,
            CadContextMenuTargetKind::Edge
        );
        assert_eq!(state.context_menu.items.len(), 3);

        let action = state
            .run_context_menu_item(0)
            .expect("edge context menu should include at least one action");
        assert!(action.contains("edge.inspect"));
        assert!(action.contains(edge_ref.as_str()));

        state.close_context_menu();
        assert!(!state.context_menu.is_open);
        assert!(state.context_menu.items.is_empty());
    }

    #[test]
    fn cad_variant_tiles_keep_independent_camera_and_selection_state() {
        let mut state = CadDemoPaneState::default();
        assert!(state.set_active_variant_tile(0));
        state.orbit_camera_by_drag(12.0, -4.0);
        state.set_focused_geometry_for_active_variant(Some("face.front".to_string()));
        state.set_hovered_geometry_for_active_variant(Some("face.front.hover".to_string()));

        assert!(state.set_active_variant_tile(1));
        let tile1_before = state.camera_orbit_yaw_deg;
        state.orbit_camera_by_drag(-18.0, 6.0);
        state.set_focused_geometry_for_active_variant(Some("edge.rim".to_string()));
        state.set_hovered_geometry_for_active_variant(Some("edge.rim.hover".to_string()));
        assert_ne!(state.camera_orbit_yaw_deg, tile1_before);

        assert!(state.set_active_variant_tile(0));
        assert_eq!(
            state.focused_geometry_ref.as_deref(),
            Some("face.front"),
            "tile 0 selection should persist"
        );
        assert_eq!(
            state.hovered_geometry_ref.as_deref(),
            Some("face.front.hover"),
            "tile 0 hover should persist"
        );
        let yaw_tile0 = state.camera_orbit_yaw_deg;

        assert!(state.set_active_variant_tile(1));
        assert_eq!(
            state.focused_geometry_ref.as_deref(),
            Some("edge.rim"),
            "tile 1 selection should persist"
        );
        assert_eq!(
            state.hovered_geometry_ref.as_deref(),
            Some("edge.rim.hover"),
            "tile 1 hover should persist"
        );
        assert_ne!(state.camera_orbit_yaw_deg, yaw_tile0);
    }

    #[test]
    fn cad_variant_tiles_keep_independent_analysis_and_warning_state() {
        let mut state = CadDemoPaneState::default();

        assert!(state.set_active_variant_tile(0));
        let mut tile0_analysis = state.analysis_snapshot.clone();
        tile0_analysis.variant_id = "variant.baseline".to_string();
        tile0_analysis.mass_kg = Some(2.7);
        state.set_variant_analysis_snapshot("variant.baseline", tile0_analysis.clone());
        state.set_variant_warning_set(
            "variant.baseline",
            vec![CadDemoWarningState {
                warning_id: "w0".to_string(),
                code: "CAD-WARN-SLIVER-FACE".to_string(),
                severity: "warning".to_string(),
                message: "sliver".to_string(),
                remediation_hint: "adjust vent spacing".to_string(),
                semantic_refs: vec!["vent_face_set".to_string()],
                deep_link: Some("cad://feature/feature.rack.vent_face_set".to_string()),
                feature_id: "feature.rack.vent_face_set".to_string(),
                entity_id: "face.0".to_string(),
            }],
        );

        assert!(state.set_active_variant_tile(1));
        let mut tile1_analysis = state.analysis_snapshot.clone();
        tile1_analysis.variant_id = "variant.lightweight".to_string();
        tile1_analysis.mass_kg = Some(2.2);
        state.set_variant_analysis_snapshot("variant.lightweight", tile1_analysis.clone());
        state.set_variant_warning_set("variant.lightweight", Vec::new());

        assert_eq!(state.analysis_snapshot.variant_id, "variant.lightweight");
        assert_eq!(state.analysis_snapshot.mass_kg, Some(2.2));
        assert!(state.warnings.is_empty());

        assert!(state.set_active_variant_tile(0));
        assert_eq!(state.analysis_snapshot.variant_id, "variant.baseline");
        assert_eq!(state.analysis_snapshot.mass_kg, Some(2.7));
        assert_eq!(state.warnings.len(), 1);
        assert_eq!(state.warnings[0].warning_id, "w0");
    }

    #[test]
    fn cad_chat_session_binding_is_deterministic_per_thread() {
        let mut state = CadDemoPaneState::default();
        let first = state.ensure_chat_session_for_thread("thread-alpha");
        let second = state.ensure_chat_session_for_thread("thread-alpha");
        let third = state.ensure_chat_session_for_thread("thread-beta");

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(
            state.active_chat_session_id.as_deref(),
            Some(third.as_str())
        );
        assert_eq!(
            state
                .chat_thread_session_bindings
                .get("thread-alpha")
                .map(String::as_str),
            Some(first.as_str())
        );
    }

    #[test]
    fn cad_chat_followup_intents_reuse_session_dispatch_state() {
        let mut state = CadDemoPaneState::default();
        let thread = "thread-followup";
        let intent_a = openagents_cad::intent::CadIntent::SetMaterial(
            openagents_cad::intent::SetMaterialIntent {
                material_id: "al-6061-t6".to_string(),
            },
        );
        let intent_b = openagents_cad::intent::CadIntent::SetObjective(
            openagents_cad::intent::SetObjectiveIntent {
                objective: "stiffness".to_string(),
            },
        );

        let first = state
            .apply_chat_intent_for_thread(thread, &intent_a)
            .expect("first intent should apply");
        let session = state
            .active_chat_session_id
            .clone()
            .expect("session should exist");
        let second = state
            .apply_chat_intent_for_thread(thread, &intent_b)
            .expect("second intent should apply");

        assert_eq!(first.state_revision, 1);
        assert_eq!(second.state_revision, 2);
        assert_eq!(state.session_id, session);
        assert_eq!(
            state
                .dispatch_sessions
                .get(&session)
                .map(|dispatch| dispatch.revision),
            Some(2)
        );
        assert_eq!(state.last_chat_intent_name.as_deref(), Some("SetObjective"));
    }
}
